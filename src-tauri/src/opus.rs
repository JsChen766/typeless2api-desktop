use libloading::{Library, Symbol};
use std::ffi::c_void;
use std::path::PathBuf;

const TARGET_RATE: u32 = 16000;
const BITRATE: i32 = 32000;
const COMPLEXITY: i32 = 9;

#[cfg(windows)]
type CreateFn = unsafe extern "system" fn(i32, i32, i32, i32) -> *mut c_void;
#[cfg(windows)]
type SetComplexityFn = unsafe extern "system" fn(*mut c_void, i32) -> i32;
#[cfg(windows)]
type EncodeFn =
    unsafe extern "system" fn(*mut c_void, *const u8, u64, *mut u8, u64, *mut u64) -> i32;
#[cfg(windows)]
type DestroyFn = unsafe extern "system" fn(*mut c_void);

#[cfg(not(windows))]
type CreateFn = unsafe extern "C" fn(i32, i32, i32, i32) -> *mut c_void;
#[cfg(not(windows))]
type SetComplexityFn = unsafe extern "C" fn(*mut c_void, i32) -> i32;
#[cfg(not(windows))]
type EncodeFn = unsafe extern "C" fn(*mut c_void, *const u8, u64, *mut u8, u64, *mut u64) -> i32;
#[cfg(not(windows))]
type DestroyFn = unsafe extern "C" fn(*mut c_void);

/// Typeless `libopuschunk`：16 kHz / mono / 32 kbps / complexity 9，每块产出 Ogg Opus。
/// 每次听写重新 dlopen，替换仓库里的 dll/dylib 后下一轮就会用新文件。
pub struct OpusSession {
    _lib: Library,
    session: *mut c_void,
    encode: EncodeFn,
    destroy: DestroyFn,
}

// 仅在单一 tokio 任务里调用；DLL 会话不是线程安全的。
unsafe impl Send for OpusSession {}
unsafe impl Sync for OpusSession {}

impl OpusSession {
    pub fn new(extra: &[PathBuf]) -> Result<Self, String> {
        let path = resolve_dll(extra)?;
        let lib = unsafe { Library::new(&path) }.map_err(|e| format!("加载 Opus 库失败: {e}"))?;
        let create = unsafe {
            let symbol: Symbol<CreateFn> = lib
                .get(b"libopuschunk_session_create")
                .map_err(|e| format!("缺少 libopuschunk_session_create: {e}"))?;
            *symbol
        };
        let set_complexity = unsafe {
            let symbol: Symbol<SetComplexityFn> = lib
                .get(b"libopuschunk_session_set_complexity")
                .map_err(|e| format!("缺少 libopuschunk_session_set_complexity: {e}"))?;
            *symbol
        };
        let encode = unsafe {
            let symbol: Symbol<EncodeFn> = lib
                .get(b"libopuschunk_session_encode_wav_chunk")
                .map_err(|e| format!("缺少 libopuschunk_session_encode_wav_chunk: {e}"))?;
            *symbol
        };
        let destroy = unsafe {
            let symbol: Symbol<DestroyFn> = lib
                .get(b"libopuschunk_session_destroy")
                .map_err(|e| format!("缺少 libopuschunk_session_destroy: {e}"))?;
            *symbol
        };
        let session = unsafe { create(TARGET_RATE as i32, 1, BITRATE, 1) };
        if session.is_null() {
            return Err("Opus 会话没有建立".into());
        }
        let code = unsafe { set_complexity(session, COMPLEXITY) };
        if code != 0 {
            unsafe { destroy(session) };
            return Err("Opus 复杂度没有设上".into());
        }
        Ok(Self {
            _lib: lib,
            session,
            encode,
            destroy,
        })
    }

    pub fn encode_pcm(&self, pcm: &[u8]) -> Result<(Vec<u8>, f64), String> {
        if pcm.is_empty() {
            return Ok((Vec::new(), 0.0));
        }
        let wav = wav_bytes(pcm, TARGET_RATE);
        let duration = pcm.len() as f64 / 2.0 / f64::from(TARGET_RATE);
        let cap = 8192usize.max(wav.len());
        let mut out = vec![0u8; cap];
        let mut size: u64 = 0;
        let code = unsafe {
            (self.encode)(
                self.session,
                wav.as_ptr(),
                wav.len() as u64,
                out.as_mut_ptr(),
                cap as u64,
                &mut size,
            )
        };
        if code != 0 {
            return Err(format!("连续 Opus 编码失败 code={code}"));
        }
        out.truncate(size as usize);
        Ok((out, duration))
    }
}

impl Drop for OpusSession {
    fn drop(&mut self) {
        if !self.session.is_null() {
            unsafe { (self.destroy)(self.session) };
            self.session = std::ptr::null_mut();
        }
    }
}

fn lib_filename() -> &'static str {
    if cfg!(target_os = "macos") {
        "libopuschunk.dylib"
    } else {
        "libopuschunk.dll"
    }
}

fn resolve_dll(extra: &[PathBuf]) -> Result<PathBuf, String> {
    let name = lib_filename();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for key in ["TYPELESS_OPUS_DLL", "TYPELESS_OPUS_LIB"] {
        if let Ok(raw) = std::env::var(key) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                candidates.push(PathBuf::from(trimmed));
            }
        }
    }
    candidates.extend(extra.iter().cloned());
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
            candidates.push(dir.join("resources").join(name));
            candidates.push(dir.join("../Resources").join(name));
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join(name);
    candidates.push(bundled);
    #[cfg(windows)]
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Typeless")
                .join("resources")
                .join("lib")
                .join("libopuschunk")
                .join("build")
                .join("windows")
                .join("x64")
                .join("libopuschunk.dll"),
        );
    }
    #[cfg(target_os = "macos")]
    {
        let apps = [
            PathBuf::from("/Applications/Typeless.app"),
            dirs_home().map(|h| h.join("Applications/Typeless.app")).unwrap_or_default(),
        ];
        for app in apps {
            if app.as_os_str().is_empty() {
                continue;
            }
            let resources = app.join("Contents/Resources/lib/libopuschunk/build");
            for os_dir in ["darwin", "macos"] {
                for arch in ["arm64", "aarch64", "x64", "x86_64"] {
                    candidates.push(resources.join(os_dir).join(arch).join(name));
                }
            }
            candidates.push(
                app.join("Contents/Resources/lib/libopuschunk").join(name),
            );
        }
    }
    if let Some(path) = candidates.into_iter().find(|p| p.is_file()) {
        return Ok(path);
    }
    Err(format!(
        "找不到 {name}。仓库应包含 desktop/src-tauri/resources/{name}，也可设置 TYPELESS_OPUS_DLL"
    ))
}

#[cfg(target_os = "macos")]
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn wav_bytes(pcm: &[u8], rate: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate.saturating_mul(2)).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}
