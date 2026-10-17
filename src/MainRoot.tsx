import { ConnectionTestButton } from "@/components/ConnectionTestButton";
import { DictationPill } from "@/components/DictationPill";
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
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useCallback, useEffect, useRef, useState } from "react";

type Conn = "idle" | "loading" | "ok" | "fail";

export default function MainRoot() {
  const [baseUrl, setBaseUrl] = useState(loadBaseUrl);
  const [apiKey, setApiKey] = useState(loadApiKey);
  const [keyHint, setKeyHint] = useState<string | null>(
    () => (loadApiKey() ? null : "请粘贴 API Key（开发用见 gateway/data/client-api-key.txt）")
  );
  const [hotkey, setHotkey] = useState(loadHotkey);
  const [capturingHotkey, setCapturingHotkey] = useState(false);
  const lastBoundHotkey = useRef(hotkey);
  const [connected, setConnected] = useState<Conn>("idle");
  const [connHint, setConnHint] = useState<string | null>(null);
  const [statusHint, setStatusHint] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);

  const onDictationError = useCallback((message: string) => {
    setStatusHint(message);
  }, []);

  const dictation = useDictation({ baseUrl, apiKey, onError: onDictationError });

  useEffect(() => {
    saveBaseUrl(baseUrl);
  }, [baseUrl]);

  useEffect(() => {
    saveApiKey(apiKey);
  }, [apiKey]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    void listen<{ phase: string; error?: string | null; mic_peak?: number }>(
      "dictation-event",
      (e) => {
        const { error, mic_peak, phase } = e.payload;
        if (error) setStatusHint(error);
        else if (phase === "idle" && typeof mic_peak === "number" && mic_peak >= 500) {
          setStatusHint(`本次录音电平 peak=${mic_peak}（正常范围约 500–8000）`);
        }
      }
    ).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!isTauri() || !apiKey.trim() || !baseUrl.trim()) return;
    void prewarmDictationUpstream(baseUrl, apiKey);
  }, [apiKey, baseUrl]);

  useEffect(() => {
    saveHotkey(hotkey);
  }, [hotkey]);

  useEffect(() => {
    if (!isTauri()) return;
    void isEnabled().then(setAutostart);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke("save_runtime_settings", {
      baseUrl,
      apiKey,
      locale: "zh-CN",
      hotkey,
    });
  }, [apiKey, baseUrl, hotkey]);

  useEffect(() => {
    if (!isTauri()) return;
    const base = loadBaseUrl();
    const key = loadApiKey();
    if (base.trim() && key.trim()) {
      void prewarmDictationUpstream(base, key);
    }
    void invoke("save_runtime_settings", {
      baseUrl: base,
      apiKey: key,
      locale: "zh-CN",
      hotkey: loadHotkey(),
    });
    void invoke<{ base_url: string; api_key: string; hotkey?: string }>(
      "get_runtime_settings"
    ).then((s) => {
      if (s.base_url && !base) setBaseUrl(s.base_url);
      if (s.api_key && !key) setApiKey(s.api_key);
      if (s.hotkey && s.hotkey !== loadHotkey()) {
        setHotkey(s.hotkey);
        saveHotkey(s.hotkey);
      }
    });
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
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
  }, [hotkey, capturingHotkey]);



  const testConnection = async () => {
    if (connected === "loading") return;
    setConnected("loading");
    setConnHint(null);
    const result = await testGatewayConnection(baseUrl, apiKey);
    setConnected(result.ok ? "ok" : "fail");
    if (!result.ok) {
      setConnHint(result.message);
    } else {
      const mode = result.health.dictation_mode ?? "unknown";
      const n = result.health.account_pool_size;
      const pool =
        typeof n === "number" ? ` · 账号池 ${n} 个` : "";
      setConnHint(`听写模式 ${mode}${pool}`);
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
              setApiKey(e.target.value);
              if (connected !== "idle") setConnected("idle");
              setConnHint(null);
              setKeyHint(
                e.target.value.trim()
                  ? null
                  : "请粘贴 API Key（开发用见 gateway/data/client-api-key.txt）"
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
              onClick={() => void invoke("dictation_toggle")}
              className="w-full rounded-lg border border-foreground/12 py-2 text-sm text-foreground transition-colors hover:border-foreground/30"
            >
              {dictation.recording ? "结束听写" : "开始听写"}
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

      {!isTauri() ? (
        <DictationPill
          visible={dictation.recording || dictation.processing}
          processing={dictation.processing}
          onCancel={dictation.cancel}
          onComplete={() => void dictation.stopAndSubmit()}
        />
      ) : null}
    </div>
  );
}
