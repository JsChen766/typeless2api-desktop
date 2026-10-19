import { ConnectionTestButton } from "@/components/ConnectionTestButton";
import { HotkeyField } from "@/components/HotkeyField";
import { Toggle } from "@/components/Toggle";
import { useDictation } from "@/hooks/useDictation";
import { prewarmDictationUpstream, testGatewayConnection } from "@/lib/gateway";
import {
  loadApiKey,
  loadBaseUrl,
  loadHotkey,
  saveApiKey,
  saveBaseUrl,
  saveHotkey,
} from "@/lib/settings";
import { formatHotkeyLabel } from "@/lib/hotkey";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { AudioLines, Eye, EyeOff, Mic2, Power } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

type Conn = "idle" | "loading" | "ok" | "fail";

export default function MainRoot() {
  const [baseUrl, setBaseUrl] = useState(loadBaseUrl);
  const [apiKey, setApiKey] = useState(loadApiKey);
  const [keyHint, setKeyHint] = useState<string | null>(
    () => (loadApiKey() ? null : "请向管理员索取 API Key")
  );
  const [hotkey, setHotkey] = useState(loadHotkey);
  const [runtimeReady, setRuntimeReady] = useState(() => !isTauri());
  const [capturingHotkey, setCapturingHotkey] = useState(false);
  const lastBoundHotkey = useRef(hotkey);
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  const saveVersion = useRef(0);
  const startupPrewarmDone = useRef(false);
  const connectionAttempt = useRef(0);
  const [connected, setConnected] = useState<Conn>("idle");
  const [connHint, setConnHint] = useState<string | null>(null);
  const [statusHint, setStatusHint] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);

  const onDictationEvent = useCallback((event: {
    phase: string;
    text?: string | null;
    error?: string | null;
    mic_peak?: number | null;
  }) => {
    if (event.phase === "recording") setStatusHint(null);
    if (event.error) setStatusHint(event.error);
    else if (event.phase === "idle" && event.text) {
      setStatusHint("识别完成，文字已复制到剪贴板；可用 ⌘V 粘贴");
    }
    else if (event.phase === "idle" && typeof event.mic_peak === "number" && event.mic_peak >= 500) {
      setStatusHint(`本次录音电平 peak=${event.mic_peak}（正常范围约 500–8000）`);
    }
  }, []);

  const dictation = useDictation(onDictationEvent);

  useEffect(() => {
    if (!runtimeReady) return;
    saveBaseUrl(baseUrl);
  }, [baseUrl, runtimeReady]);

  useEffect(() => {
    if (!runtimeReady) return;
    saveApiKey(apiKey);
  }, [apiKey, runtimeReady]);

  useEffect(() => {
    if (!isTauri() || !runtimeReady || startupPrewarmDone.current) return;
    if (!apiKey.trim() || !baseUrl.trim()) {
      startupPrewarmDone.current = true;
      return;
    }
    const timer = window.setTimeout(() => {
      startupPrewarmDone.current = true;
      void prewarmDictationUpstream(baseUrl, apiKey);
    }, 600);
    return () => window.clearTimeout(timer);
  }, [apiKey, baseUrl, runtimeReady]);

  useEffect(() => {
    if (!runtimeReady) return;
    saveHotkey(hotkey);
  }, [hotkey, runtimeReady]);

  useEffect(() => {
    if (!isTauri()) return;
    void isEnabled().then(setAutostart);
  }, []);

  useEffect(() => {
    if (!isTauri() || !runtimeReady) return;
    const version = ++saveVersion.current;
    saveQueue.current = saveQueue.current.catch(() => {}).then(async () => {
      if (version !== saveVersion.current) return;
      await invoke("save_runtime_settings", {
        baseUrl,
        apiKey,
        locale: "zh-CN",
      });
    }).catch((err) => {
      if (version === saveVersion.current) {
        setStatusHint("保存设置失败，请检查应用数据目录的写入权限");
      }
      throw err;
    });
    void saveQueue.current.catch(() => {});
  }, [apiKey, baseUrl, runtimeReady]);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    void invoke<{ base_url: string; api_key: string; hotkey?: string }>(
      "get_runtime_settings"
    ).then((s) => {
      if (cancelled) return;
      setBaseUrl(s.base_url);
      setApiKey(s.api_key);
      setKeyHint(s.api_key.trim() ? null : "请向管理员索取 API Key");
      setHotkey(s.hotkey || loadHotkey());
      lastBoundHotkey.current = s.hotkey || loadHotkey();
      setRuntimeReady(true);
    }).catch(() => {
      if (!cancelled) {
        setStatusHint("无法读取本地设置，请检查应用数据目录后重启客户端");
      }
    });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!isTauri() || !runtimeReady) return;
    let cancelled = false;
    const binding = hotkey;
    const apply = async () => {
      try {
        if (capturingHotkey) {
          await invoke("clear_dictation_hotkey_cmd");
          return;
        }
        await invoke("set_dictation_hotkey", { hotkey: binding });
        if (!cancelled) lastBoundHotkey.current = binding;
      } catch (err: unknown) {
        if (cancelled) return;
        const raw =
          typeof err === "string"
            ? err
            : err instanceof Error
              ? err.message
              : String(err);
        const message = raw.trim() || "快捷键注册失败";
        setStatusHint(message);
        setHotkey(lastBoundHotkey.current);
      }
    };
    void apply();
    return () => {
      cancelled = true;
    };
  }, [hotkey, capturingHotkey, runtimeReady]);



  const testConnection = async () => {
    if (connected === "loading") return;
    const attempt = ++connectionAttempt.current;
    setConnected("loading");
    setConnHint(null);
    try {
      await saveQueue.current;
    } catch {
      setConnected("fail");
      setConnHint("保存设置失败，请检查应用数据目录的写入权限");
      return;
    }
    const result = await testGatewayConnection(baseUrl, apiKey);
    if (attempt !== connectionAttempt.current) return;
    setConnected(result.ok ? "ok" : "fail");
    if (!result.ok) {
      setConnHint(result.message);
    } else {
      setConnHint("Gateway 可用，API Key 已验证");
      void prewarmDictationUpstream(baseUrl, apiKey);
    }
  };

  const onAutostartChange = async (next: boolean) => {
    if (!isTauri()) return;
    try {
      if (next) await enable();
      else await disable();
      setAutostart(await isEnabled());
    } catch {
      setStatusHint("无法更改开机自启");
    }
  };

  const connectionLabel = {
    idle: "未测试",
    loading: "连接中",
    ok: "已连接",
    fail: "连接失败",
  }[connected];

  return (
    <div className="flex h-full min-h-0 flex-col bg-[#0d1217] text-[#f1f5f4]">
      <header className="flex shrink-0 items-center gap-2.5 px-4 pb-2 pt-3">
        <div className="flex size-8 items-center justify-center rounded-[10px] bg-[#203b38] text-[#7bd5be]">
          <AudioLines size={17} strokeWidth={2.1} aria-hidden="true" />
        </div>
        <div className="leading-tight">
          <h1 className="text-[14px] font-semibold tracking-[-0.01em]">Typeless</h1>
          <p className="text-[11px] text-[#94a4aa]">语音输入设置</p>
        </div>
      </header>

      <main className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-3 pb-2">
        <section className="rounded-[12px] border border-[#263038] bg-[#171d23] px-3 py-2.5">
          <div className="mb-2 flex items-center justify-between">
            <h2 className="text-[12px] font-semibold text-[#dfe9e6]">连接</h2>
            <span className="inline-flex items-center gap-1.5 text-[11px] text-[#a5b3b6]" role="status">
              <span className={`size-1.5 rounded-full ${connected === "ok" ? "bg-[#7bd5be]" : connected === "fail" ? "bg-[#e2a190]" : "bg-[#718087]"}`} />
              {connectionLabel}
            </span>
          </div>
          <label className="block text-[11px] font-medium text-[#aebbc0]">
            Gateway 地址
            <input
              className="mt-1 block h-9 w-full rounded-[8px] border border-[#34414a] bg-[#10161b] px-2.5 text-[13px] font-normal text-[#f1f5f4] outline-none placeholder:text-[#66767d] focus-visible:border-[#7bd5be] focus-visible:ring-2 focus-visible:ring-[#7bd5be]/20"
              value={baseUrl}
              placeholder="http://127.0.0.1:8787"
              spellCheck={false}
              onChange={(e) => {
                connectionAttempt.current++;
                setBaseUrl(e.target.value);
                if (connected !== "idle") setConnected("idle");
                setConnHint(null);
              }}
            />
          </label>
          <div className="mt-2">
            <label htmlFor="gateway-api-key" className="block text-[11px] font-medium text-[#aebbc0]">API Key</label>
            <div className="relative mt-1">
              <input
                id="gateway-api-key"
                type={showApiKey ? "text" : "password"}
                className="block h-9 w-full rounded-[8px] border border-[#34414a] bg-[#10161b] py-1.5 pl-2.5 pr-9 text-[13px] font-normal text-[#f1f5f4] outline-none placeholder:text-[#66767d] focus-visible:border-[#7bd5be] focus-visible:ring-2 focus-visible:ring-[#7bd5be]/20"
                value={apiKey}
                placeholder="输入 API Key"
                spellCheck={false}
                onChange={(e) => {
                  connectionAttempt.current++;
                  setApiKey(e.target.value);
                  if (connected !== "idle") setConnected("idle");
                  setConnHint(null);
                  setKeyHint(e.target.value.trim() ? null : "请向管理员索取 API Key");
                }}
              />
              <button
                type="button"
                aria-label={showApiKey ? "隐藏 API Key" : "显示 API Key"}
                onClick={() => setShowApiKey((shown) => !shown)}
                className="absolute inset-y-0 right-0 flex w-9 items-center justify-center rounded-r-[8px] text-[#90a0a6] hover:text-[#e2eeea] focus-visible:outline-2 focus-visible:outline-[#7bd5be]"
              >
                {showApiKey ? <EyeOff size={15} aria-hidden="true" /> : <Eye size={15} aria-hidden="true" />}
              </button>
            </div>
          </div>
          {keyHint && !apiKey.trim() ? (
            <p className="mt-1.5 text-[11px] text-[#aebbc0]">{keyHint}</p>
          ) : null}
          <div className="mt-2">
            <ConnectionTestButton status={connected} onClick={testConnection} />
            {connHint ? <p role="status" className="mt-1.5 break-words text-[11px] leading-[1.35] text-[#aebbc0]">{connHint}</p> : null}
          </div>
        </section>

        <section className="rounded-[12px] border border-[#263038] bg-[#171d23] px-3 py-2.5">
          <div className="mb-2 flex items-baseline justify-between gap-2">
            <h2 className="text-[12px] font-semibold text-[#dfe9e6]">听写快捷键</h2>
            <span className="text-[10px] text-[#82949a]">在任意应用中使用</span>
          </div>
          <HotkeyField
            value={hotkey}
            onChange={(next) => {
              setHotkey(next);
              saveHotkey(next);
              setStatusHint(`快捷键已设为 ${formatHotkeyLabel(next)}`);
            }}
            onCapturingChange={setCapturingHotkey}
          />
          {isTauri() ? (
            <button
              type="button"
              disabled={!runtimeReady || dictation.processing}
              onClick={() => void saveQueue.current.then(() => invoke("dictation_toggle")).catch((err: unknown) => {
                setStatusHint(err instanceof Error ? err.message : "无法切换听写状态");
              })}
              className="mt-2 flex h-10 w-full items-center justify-center gap-2 rounded-[9px] bg-[#7bd5be] text-[13px] font-semibold text-[#10221e] outline-none transition-colors hover:bg-[#a0e6d1] focus-visible:ring-2 focus-visible:ring-[#baf2df] focus-visible:ring-offset-2 focus-visible:ring-offset-[#171d23] disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Mic2 size={15} strokeWidth={2.2} aria-hidden="true" />
              {dictation.processing ? "处理中…" : dictation.recording ? "结束听写" : "开始听写"}
            </button>
          ) : null}
        </section>

        {isTauri() ? (
          <section className="flex items-center justify-between gap-3 rounded-[12px] border border-[#263038] bg-[#171d23] px-3 py-2.5">
            <div>
              <p className="text-[12px] font-medium text-[#e5ecea]">开机自启</p>
              <p className="mt-0.5 text-[10px] text-[#94a4aa]">登录后在后台运行</p>
            </div>
            <Toggle
              checked={autostart}
              onCheckedChange={(v) => void onAutostartChange(v)}
              aria-label="开机自启"
            />
          </section>
        ) : null}

        {statusHint ? (
          <div role="status" className="flex flex-col gap-2 rounded-[8px] bg-[#25332f] px-2.5 py-1.5 text-[11px] leading-relaxed text-[#d1ebe2]">
            <p className="break-words whitespace-pre-wrap">{statusHint}</p>
            {isTauri() &&
            /Mac/i.test(navigator.platform) &&
            (statusHint.includes("完全退出") || statusHint.includes("辅助功能") || statusHint.includes("输入监控")) ? (
              <button
                type="button"
                className="self-start rounded-[6px] bg-[#2d4a42] px-2 py-1 text-[10px] font-medium text-[#b8e8d8] hover:bg-[#35554b]"
                onClick={() => void invoke("restart_app")}
              >
                保存权限后重启应用
              </button>
            ) : null}
          </div>
        ) : null}

        {isTauri() ? (
          <button
            type="button"
            onClick={() => void invoke("quit_app")}
            className="inline-flex h-8 items-center gap-1.5 self-start rounded-[8px] px-2 text-[11px] text-[#8e9da3] outline-none hover:bg-[#20282f] hover:text-[#e5ecea] focus-visible:ring-2 focus-visible:ring-[#7bd5be]"
          >
            <Power size={12} aria-hidden="true" />
            退出客户端
          </button>
        ) : null}
      </main>
    </div>
  );
}
