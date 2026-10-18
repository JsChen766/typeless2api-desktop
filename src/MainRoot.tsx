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
        const message = err instanceof Error ? err.message : "快捷键注册失败";
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
      const mode = result.health.dictation_mode ?? "unknown";
      const n = result.health.account_pool_size;
      const pool =
        typeof n === "number" ? ` · 账号池 ${n} 个` : "";
      setConnHint(`听写模式 ${mode}${pool}`);
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

  return (
    <div className="flex h-full flex-col bg-background">
      <main className="flex flex-1 flex-col gap-2.5 px-3 py-3">
        <section className="space-y-2 rounded-xl bg-card p-3">
          <input
            className="w-full rounded-lg border border-foreground/12 bg-transparent px-3 py-2.5 text-sm text-foreground outline-none focus:border-foreground/35 focus:ring-2 focus:ring-foreground/10"
            value={baseUrl}
            placeholder="http://127.0.0.1:8787"
            onChange={(e) => {
              connectionAttempt.current++;
              setBaseUrl(e.target.value);
              if (connected !== "idle") setConnected("idle");
              setConnHint(null);
            }}
          />
          <input
            type="password"
            className="w-full rounded-lg border border-foreground/12 bg-transparent px-3 py-2.5 text-sm text-foreground outline-none focus:border-foreground/35 focus:ring-2 focus:ring-foreground/10"
            value={apiKey}
            placeholder="sk_live_…"
            onChange={(e) => {
              connectionAttempt.current++;
              setApiKey(e.target.value);
              if (connected !== "idle") setConnected("idle");
              setConnHint(null);
              setKeyHint(
                e.target.value.trim()
                  ? null
                  : "请向管理员索取 API Key"
              );
            }}
          />
          {keyHint && !apiKey.trim() ? (
            <p className="text-xs text-muted-foreground">{keyHint}</p>
          ) : null}
          <ConnectionTestButton status={connected} onClick={testConnection} />
          {connHint ? <p className="text-xs text-muted-foreground">{connHint}</p> : null}
        </section>

        <section className="space-y-2 rounded-xl bg-card p-3">
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
              className="w-full rounded-lg border border-foreground/12 py-2 text-sm text-foreground transition-colors hover:border-foreground/30 disabled:opacity-50"
            >
              {dictation.processing ? "处理中…" : dictation.recording ? "结束听写" : "开始听写"}
            </button>
          ) : null}
        </section>

        {isTauri() ? (
          <section className="flex items-center justify-between gap-3 rounded-xl bg-card px-3 py-2.5">
            <div>
              <p className="text-sm text-foreground">开机自启</p>
              <p className="text-[11px] text-muted-foreground">登录后停在任务栏，不打开窗口</p>
            </div>
            <Toggle
              checked={autostart}
              onCheckedChange={(v) => void onAutostartChange(v)}
              aria-label="开机自启"
            />
          </section>
        ) : null}

        {statusHint ? (
          <p className="px-1 text-xs text-muted-foreground">{statusHint}</p>
        ) : null}

        {isTauri() ? (
          <button
            type="button"
            onClick={() => void invoke("quit_app")}
            className="mt-auto rounded-lg px-2 py-1.5 text-left text-xs text-muted-foreground hover:text-foreground"
          >
            退出
          </button>
        ) : null}
      </main>
    </div>
  );
}
