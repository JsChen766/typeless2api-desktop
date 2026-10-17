use crate::inject::{
    self, capture_foreground_target, hide_pill_and_inject, set_pill_visible, DictationContextHint,
};
#[cfg(any(windows, target_os = "macos"))]
use crate::opus::OpusSession;
use crate::settings::load_runtime_settings;

#[cfg(not(any(windows, target_os = "macos")))]
struct OpusSession;

#[cfg(not(any(windows, target_os = "macos")))]
impl OpusSession {
    fn new(_extra: &[std::path::PathBuf]) -> Result<Self, String> {
        Err("听写编码仅支持 Windows / macOS（需要 libopuschunk）".into())
    }

    fn encode_pcm(&self, _pcm: &[u8]) -> Result<(Vec<u8>, f64), String> {
        Err("听写编码仅支持 Windows / macOS".into())
    }
}
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleRate;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const TARGET_RATE: u32 = 16000;
/// 官方 AudioWorklet：SAMPLES_PER_MESSAGE = 1024
const CHUNK_SAMPLES: usize = 1024;
const SINC_HALF_TAPS: i32 = 12;

#[derive(Clone, Serialize)]
struct DictationEvent {
    phase: String,
    text: Option<String>,
    error: Option<String>,
    /// 本次会话 PCM 峰值（约 16k s16le），<500 多为电平/设备问题
    mic_peak: Option<u32>,
}

struct ActiveSession {
    id: u64,
    stop_tx: Option<oneshot::Sender<()>>,
    result_rx: oneshot::Receiver<Result<(String, u32), String>>,
    cancelled: Arc<AtomicBool>,
}

static SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
/// stop 已取走 SESSION 但后台仍在转写/注入
static DICTATION_BUSY: AtomicBool = AtomicBool::new(false);
/// 从按下结束到结果贴回输入框之前，不再开始新的一轮。
static STOPPING: AtomicBool = AtomicBool::new(false);
/// 结束后短暂不再接受新的开始。粘贴按键和快捷键松开都会再投递一次热键。
static COOL_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn cooling_down() -> bool {
    COOL_UNTIL
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .is_some_and(|until| Instant::now() < until)
}

fn arm_cooldown() {
    if let Ok(mut guard) = COOL_UNTIL.lock() {
        *guard = Some(Instant::now() + Duration::from_millis(350));
    }
}

fn emit_phase(
    app: &AppHandle,
    phase: &str,
    text: Option<String>,
    error: Option<String>,
    mic_peak: Option<u32>,
) {
    let _ = app.emit(
        "dictation-event",
        DictationEvent {
            phase: phase.into(),
            text: text.clone(),
            error: error.clone(),
            mic_peak,
        },
    );
    if phase == "idle" {
        STOPPING.store(false, Ordering::SeqCst);
        arm_cooldown();
    }
    let visible = phase == "recording" || phase == "processing";
    if visible {
        let _ = set_pill_visible(app.clone(), true, phase.to_string());
    } else {
        let _ = set_pill_visible(app.clone(), false, "idle".into());
    }
}

pub fn toggle_dictation(app: AppHandle) {
    let active = SESSION
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false);
    if active {
        if STOPPING.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = set_pill_visible(app.clone(), true, "processing".into());
        tauri::async_runtime::spawn(async move {
            let _ = dictation_stop(app).await;
        });
        return;
    }

    if STOPPING.load(Ordering::SeqCst)
        || DICTATION_BUSY.load(Ordering::SeqCst)
        || cooling_down()
    {
        return;
    }

    // 必须在快捷键/托盘的 UI 线程上立刻 show，后台线程 show WebView 经常没反应
    capture_foreground_target();
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let _ = set_pill_visible(app.clone(), true, "recording".into());

    tauri::async_runtime::spawn(async move {
        let settings = load_runtime_settings(&app);
        if settings.api_key.trim().is_empty() {
            emit_phase(
                &app,
                "idle",
                None,
                Some("请先在托盘打开「设置」填写 API Key".into()),
                None,
            );
            return;
        }
        let hint = inject::dictation_context_hint();
        if let Err(err) = dictation_start(
            app.clone(),
            settings.base_url,
            settings.api_key,
            Some("zh-CN".into()),
            Some(hint),
        )
        .await
        {
            emit_phase(&app, "idle", None, Some(err), None);
        }
    });
}

fn http_to_ws_url(base: &str, api_key: &str) -> Result<String, String> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Gateway 地址为空".into());
    }
    let ws_base = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        format!("ws://{}", trimmed)
    };
    let token = urlencoding::encode(api_key.trim());
    Ok(format!("{}/v1/dictation/stream?token={}", ws_base, token))
}

fn f32_to_i16_le(input: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() * 2);
    for &s in input {
        let clamped = s.clamp(-1.0, 1.0);
        let v = if clamped < 0.0 {
            (clamped * 32768.0) as i16
        } else {
            (clamped * 32767.0) as i16
        };
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let pi_x = std::f64::consts::PI * x;
        pi_x.sin() / pi_x
    }
}

/// 官方页面用 AudioContext(16 kHz) 重采样。线性插值会混叠，这里用短 sinc。
struct RateConverter {
    from_rate: u32,
    to_rate: u32,
    pending: Vec<f32>,
    phase: f64,
}

impl RateConverter {
    fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            from_rate,
            to_rate,
            pending: Vec::new(),
            phase: SINC_HALF_TAPS as f64,
        }
    }

    fn push(&mut self, input: &[f32]) -> Vec<u8> {
        if self.from_rate == 0 || self.to_rate == 0 {
            return Vec::new();
        }
        if self.from_rate == self.to_rate {
            return f32_to_i16_le(input);
        }
        self.pending.extend_from_slice(input);
        let step = self.from_rate as f64 / self.to_rate as f64;
        let cutoff = (self.to_rate as f64 / self.from_rate as f64).min(1.0);
        let half = SINC_HALF_TAPS as f64;
        let mut out_f = Vec::new();
        while self.phase + half + 1.0 < self.pending.len() as f64 {
            let center = self.phase;
            let mut acc = 0.0f64;
            let i0 = (center - half).ceil() as i32;
            let i1 = (center + half).floor() as i32;
            for i in i0..=i1 {
                if i < 0 {
                    continue;
                }
                let iu = i as usize;
                if iu >= self.pending.len() {
                    break;
                }
                let dist = i as f64 - center;
                let window = 0.5 + 0.5 * (std::f64::consts::PI * dist / half).cos();
                acc += self.pending[iu] as f64 * sinc(dist * cutoff) * window;
            }
            out_f.push(acc as f32);
            self.phase += step;
        }
        let drop = (self.phase as i32 - SINC_HALF_TAPS).max(0) as usize;
        let drop = drop.min(self.pending.len());
        if drop > 0 {
            self.pending.drain(..drop);
            self.phase -= drop as f64;
        }
        f32_to_i16_le(&out_f)
    }
}

struct PcmChunker {
    pending: Vec<u8>,
}

impl PcmChunker {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

    fn push(&mut self, pcm: Vec<u8>) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(&pcm);
        let chunk_bytes = CHUNK_SAMPLES * 2;
        let mut chunks = Vec::new();
        while self.pending.len() >= chunk_bytes {
            let piece = self.pending.drain(..chunk_bytes).collect();
            chunks.push(piece);
        }
        chunks
    }

    fn take_remainder(&mut self) -> Option<Vec<u8>> {
        if self.pending.is_empty() {
            None
        } else {
            Some(self.pending.drain(..).collect())
        }
    }
}

fn pick_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device.default_input_config().map_err(|e| e.to_string())?;
    let target = SampleRate(TARGET_RATE);
    if let Ok(ranges) = device.supported_input_configs() {
        let mut fallback = None;
        for range in ranges {
            if range.min_sample_rate().0 <= TARGET_RATE && range.max_sample_rate().0 >= TARGET_RATE {
                let cfg = range.with_sample_rate(target);
                if cfg.channels() == 1 {
                    return Ok(cfg);
                }
                if fallback.is_none() {
                    fallback = Some(cfg);
                }
            }
        }
        if let Some(cfg) = fallback {
            return Ok(cfg);
        }
    }
    Ok(default)
}

fn note_pcm_peak(peak: &mut u32, pcm: &[u8]) {
    for sample in pcm.chunks_exact(2) {
        let s = i16::from_le_bytes([sample[0], sample[1]]);
        let a = s.abs() as u32;
        if a > *peak {
            *peak = a;
        }
    }
}

#[derive(Serialize)]
struct AudioMessage {
    #[serde(rename = "type")]
    kind: &'static str,
    duration: f64,
}

async fn send_opus_chunks(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    encoder: &OpusSession,
    chunker: &mut PcmChunker,
    bytes: Vec<u8>,
    peak: &mut u32,
) -> Result<(), String> {
    note_pcm_peak(peak, &bytes);
    for chunk in chunker.push(bytes) {
        note_pcm_peak(peak, &chunk);
        send_opus_blob(write, encoder, &chunk).await?;
    }
    Ok(())
}

async fn send_opus_blob(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    encoder: &OpusSession,
    pcm: &[u8],
) -> Result<(), String> {
    let (blob, duration) = encoder.encode_pcm(pcm)?;
    if blob.is_empty() {
        return Ok(());
    }
    let header = serde_json::to_string(&AudioMessage {
        kind: "audio",
        duration,
    })
    .map_err(|e| e.to_string())?;
    write
        .send(Message::Text(header.into()))
        .await
        .map_err(|e| e.to_string())?;
    write
        .send(Message::Binary(blob.into()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
struct StartMessage {
    #[serde(rename = "type")]
    kind: String,
    locale: String,
    context_hint: Option<DictationContextHint>,
    record_pressed_at_ms: u64,
}

#[derive(Deserialize)]
struct WsEnvelope {
    #[serde(rename = "type")]
    kind: String,
    message: Option<String>,
    #[serde(default)]
    code: Option<String>,
    data: Option<WsResultData>,
}

fn ws_error_text(env: &WsEnvelope) -> String {
    let msg = env
        .message
        .clone()
        .unwrap_or_else(|| "服务端错误".into());
    match env.code.as_deref() {
        Some(code) if !code.is_empty() => format!("{code}: {msg}"),
        _ => msg,
    }
}

#[derive(Deserialize)]
struct WsResultData {
    refined_text: Option<String>,
}

struct LiveMic {
    flag: Arc<AtomicBool>,
    rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl Drop for LiveMic {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn open_microphone(stop_mic: Arc<AtomicBool>) -> Result<LiveMic, String> {
    let (pcm_tx, pcm_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let mic_stop_flag = stop_mic.clone();
    let mic_thread = std::thread::spawn(move || {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "没有默认麦克风".to_string())?;
        let supported = pick_input_config(&device)?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let stream_config = supported.config();
        let err_fn = |e| eprintln!("mic stream error: {e}");
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                let tx = pcm_tx.clone();
                let flag = mic_stop_flag.clone();
                let mut converter = RateConverter::new(sample_rate, TARGET_RATE);
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        if flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let mono: Vec<f32> = if channels == 1 {
                            data.to_vec()
                        } else {
                            data.chunks(channels)
                                .map(|c| c.iter().sum::<f32>() / channels as f32)
                                .collect()
                        };
                        let bytes = converter.push(&mono);
                        if !bytes.is_empty() {
                            let _ = tx.send(bytes);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let tx = pcm_tx.clone();
                let flag = mic_stop_flag.clone();
                let mut converter = RateConverter::new(sample_rate, TARGET_RATE);
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        if flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let mono: Vec<f32> = if channels == 1 {
                            data.iter().map(|&s| s as f32 / 32768.0).collect()
                        } else {
                            data.chunks(channels)
                                .map(|c| {
                                    c.iter().map(|&s| s as f32).sum::<f32>()
                                        / (channels as f32 * 32768.0)
                                })
                                .collect()
                        };
                        let bytes = converter.push(&mono);
                        if !bytes.is_empty() {
                            let _ = tx.send(bytes);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => return Err::<(), String>("不支持的麦克风采样格式".into()),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        while !mic_stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    });
    Ok(LiveMic {
        flag: stop_mic,
        rx: pcm_rx,
        thread: Some(mic_thread),
    })
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn run_stream(
    _app: AppHandle,
    base_url: String,
    api_key: String,
    locale: String,
    context_hint: Option<DictationContextHint>,
    record_pressed_at_ms: u64,
    stop_mic: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    mut stop_signal: oneshot::Receiver<()>,
) -> Result<(String, u32), String> {
    // 官方客户端是连接已经在，人一开始说话就有采样。这里先开麦，握手期间的声音先攒着，不丢掉开头。
    let mut opus_search = Vec::new();
    if let Ok(dir) = _app.path().resource_dir() {
        opus_search.push(dir.join("libopuschunk.dll"));
        opus_search.push(dir.join("libopuschunk.dylib"));
    }
    let encoder = OpusSession::new(&opus_search)?;
    let mut mic = open_microphone(stop_mic)?;
    let mut pcm_peak: u32 = 0;
    let url = http_to_ws_url(&base_url, &api_key)?;
    let (ws, _) = connect_async(&url)
        .await
        .map_err(|e| format!("WebSocket 连接失败: {e}"))?;
    let (mut write, mut read) = ws.split();

    let start = StartMessage {
        kind: "start".into(),
        locale,
        context_hint,
        record_pressed_at_ms,
    };
    let start_json = serde_json::to_string(&start).map_err(|e| e.to_string())?;
    write
        .send(Message::Text(start_json.into()))
        .await
        .map_err(|e| e.to_string())?;

    let ready = read
        .next()
        .await
        .ok_or_else(|| "连接已关闭".to_string())?
        .map_err(|e| e.to_string())?;
    if let Message::Text(t) = ready {
        let env: WsEnvelope = serde_json::from_str(&t).map_err(|e| e.to_string())?;
        if env.kind != "ready" {
            if env.kind == "error" {
                return Err(ws_error_text(&env));
            }
            return Err(env.message.unwrap_or_else(|| "握手失败".into()));
        }
    } else {
        return Err("无效的握手响应".into());
    }

    let mut chunker = PcmChunker::new();
    while let Ok(bytes) = mic.rx.try_recv() {
        send_opus_chunks(&mut write, &encoder, &mut chunker, bytes, &mut pcm_peak).await?;
    }

    let mut recording = true;
    while recording {
        tokio::select! {
            _ = &mut stop_signal => {
                recording = false;
            }
            pcm = mic.rx.recv() => {
                match pcm {
                    Some(bytes) => send_opus_chunks(&mut write, &encoder, &mut chunker, bytes, &mut pcm_peak).await?,
                    None => recording = false,
                }
            }
            msg = read.next() => {
                if let Some(Ok(Message::Text(t))) = msg {
                    if let Ok(env) = serde_json::from_str::<WsEnvelope>(&t) {
                        if env.kind == "error" {
                            return Err(ws_error_text(&env));
                        }
                    }
                }
            }
        }
    }

    // 官方松开后只 flush 已缓冲采样，不再多录一段。
    mic.flag.store(true, Ordering::Relaxed);
    let mic_error = match mic.thread.take() {
        Some(thread) => thread.join().unwrap_or(Ok(())).err(),
        None => None,
    };
    while let Ok(bytes) = mic.rx.try_recv() {
        send_opus_chunks(&mut write, &encoder, &mut chunker, bytes, &mut pcm_peak).await?;
    }
    drop(mic);
    if let Some(err) = mic_error {
        return Err(err);
    }

    if cancelled.load(Ordering::Relaxed) {
        let _ = write
            .send(Message::Text(r#"{"type":"cancel"}"#.into()))
            .await;
        return Ok((String::new(), pcm_peak));
    }

    if let Some(tail) = chunker.take_remainder() {
        note_pcm_peak(&mut pcm_peak, &tail);
        send_opus_blob(&mut write, &encoder, &tail).await?;
    }

    let end = serde_json::json!({ "type": "end", "mic_peak": pcm_peak });
    write
        .send(Message::Text(end.to_string().into()))
        .await
        .map_err(|e| e.to_string())?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while tokio::time::Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_secs(5), read.next())
            .await
            .map_err(|_| "等待转写结果超时".to_string())?;
        match msg {
            Some(Ok(Message::Text(t))) => {
                let env: WsEnvelope = serde_json::from_str(&t).map_err(|e| e.to_string())?;
                match env.kind.as_str() {
                    "result" => {
                        let text = env
                            .data
                            .and_then(|d| d.refined_text)
                            .unwrap_or_default();
                        return Ok((text, pcm_peak));
                    }
                    "error" => {
                        return Err(ws_error_text(&env));
                    }
                    _ => {}
                }
            }
            Some(Ok(Message::Close(_))) => break,
            None => break,
            Some(Err(e)) => return Err(e.to_string()),
            _ => {}
        }
    }
    Err("未收到转写结果".into())
}

#[tauri::command]
pub fn dictation_toggle(app: AppHandle) {
    toggle_dictation(app);
}

#[tauri::command]
pub async fn dictation_start(
    app: AppHandle,
    base_url: String,
    api_key: String,
    locale: Option<String>,
    context_hint: Option<DictationContextHint>,
) -> Result<(), String> {
    {
        let guard = SESSION.lock().map_err(|_| "lock")?;
        if guard.is_some() {
            return Err("听写已在进行".into());
        }
    }

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    let stop_mic = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let (stop_tx, stop_rx) = oneshot::channel();
    let (result_tx, result_rx) = oneshot::channel();
    let locale = locale.unwrap_or_else(|| "zh-CN".into());
    let session_id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);

    {
        let mut guard = SESSION.lock().map_err(|_| "lock")?;
        *guard = Some(ActiveSession {
            id: session_id,
            stop_tx: Some(stop_tx),
            result_rx,
            cancelled: cancelled.clone(),
        });
    }

    emit_phase(&app, "recording", None, None, None);
    DICTATION_BUSY.store(true, Ordering::SeqCst);

    let record_pressed_at_ms = epoch_ms();
    let app2 = app.clone();
    let cancelled_task = cancelled.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_stream(
            app2.clone(),
            base_url,
            api_key,
            locale,
            context_hint,
            record_pressed_at_ms,
            stop_mic,
            cancelled_task.clone(),
            stop_rx,
        )
        .await;
        {
            let mut guard = SESSION.lock().unwrap_or_else(|e| e.into_inner());
            if guard.as_ref().is_none_or(|session| session.id == session_id) {
                *guard = None;
            }
        }
        if cancelled_task.load(Ordering::Relaxed) {
            emit_phase(&app2, "idle", None, None, None);
            DICTATION_BUSY.store(false, Ordering::SeqCst);
            let _ = result_tx.send(Ok((String::new(), 0)));
            return;
        }
        match &result {
            Ok((text, peak)) => {
                let peak = *peak;
                let peak_note = mic_peak_hint(peak);
                if !text.is_empty() {
                    match hide_pill_and_inject(app2.clone(), text.clone()).await {
                        Ok(()) => emit_phase(
                            &app2,
                            "idle",
                            Some(text.clone()),
                            peak_note,
                            Some(peak),
                        ),
                        Err(err) => emit_phase(
                            &app2,
                            "idle",
                            Some(text.clone()),
                            Some(format!(
                                "已复制到剪贴板，请到输入框 {}（{err}）",
                                inject::paste_shortcut()
                            )),
                            Some(peak),
                        ),
                    }
                } else {
                    emit_phase(
                        &app2,
                        "idle",
                        None,
                        Some(
                            peak_note.unwrap_or_else(|| "识别结果为空，请再说一次".into()),
                        ),
                        Some(peak),
                    );
                }
            }
            Err(err) => emit_phase(&app2, "idle", None, Some(err.clone()), None),
        }
        DICTATION_BUSY.store(false, Ordering::SeqCst);
        let _ = result_tx.send(result);
    });

    Ok(())
}

#[tauri::command]
pub async fn dictation_stop(app: AppHandle) -> Result<String, String> {
    STOPPING.store(true, Ordering::SeqCst);
    let session = {
        let mut guard = SESSION.lock().map_err(|_| "lock")?;
        guard.take()
    };
    let session = match session {
        Some(session) => session,
        None => {
            STOPPING.store(false, Ordering::SeqCst);
            return Err("当前没有在听写".into());
        }
    };
    if let Some(tx) = session.stop_tx {
        let _ = tx.send(());
    }
    emit_phase(&app, "processing", None, None, None);
    session
        .result_rx
        .await
        .map_err(|_| "听写任务异常结束".to_string())?
        .map(|(text, _peak)| text)
}

#[tauri::command]
pub async fn dictation_cancel(app: AppHandle) -> Result<(), String> {
    STOPPING.store(true, Ordering::SeqCst);
    let _ = set_pill_visible(app.clone(), false, "idle".into());
    let session = {
        let mut guard = SESSION.lock().map_err(|_| "lock")?;
        guard.take()
    };
    if let Some(s) = session {
        s.cancelled.store(true, Ordering::Relaxed);
        if let Some(tx) = s.stop_tx {
            let _ = tx.send(());
        }
        let _ = tokio::time::timeout(Duration::from_secs(3), s.result_rx).await;
    }
    emit_phase(&app, "idle", None, None, None);
    Ok(())
}

fn mic_peak_hint(peak: u32) -> Option<String> {
    if peak < 500 {
        Some(format!(
            "麦克风电平偏低（peak={peak}）：Windows 设置→声音→输入音量调高，或靠近麦克风；误识别不一定是 Gateway 故障"
        ))
    } else if peak > 12000 {
        Some(format!(
            "麦克风电平偏高（peak={peak}）：可能削波，略降输入音量或离麦远一点"
        ))
    } else {
        None
    }
}
