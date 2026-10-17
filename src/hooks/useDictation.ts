import { copyToClipboard } from "@/lib/clipboard";
import { getDictationContextHint } from "@/lib/dictationContext";
import { transcribeWav } from "@/lib/gateway";
import { captureInputTarget, injectIntoForeground } from "@/lib/inject";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MicRecorder } from "@/lib/recorder";
import { useCallback, useEffect, useRef, useState } from "react";

export type DictationPhase = "idle" | "recording" | "processing";

type Config = {
  baseUrl: string;
  apiKey: string;
  onError?: (message: string) => void;
};

type DictationEventPayload = {
  phase: string;
  error?: string | null;
};

export function useDictation({ baseUrl, apiKey, onError }: Config) {
  const [phase, setPhase] = useState<DictationPhase>("idle");
  const recorderRef = useRef<MicRecorder | null>(null);
  const contextHintRef = useRef<Awaited<ReturnType<typeof getDictationContextHint>>>(null);
  const streamActiveRef = useRef(false);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    void listen<DictationEventPayload>("dictation-event", (e) => {
      const { phase: p, error } = e.payload;
      if (p === "recording") setPhase("recording");
      if (p === "processing") setPhase("processing");
      if (p === "idle") {
        streamActiveRef.current = false;
        setPhase("idle");
        if (error) onError?.(error);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [onError]);

  const finishWithBlob = useCallback(
    async (wav: Blob) => {
      setPhase("processing");
      const result = await transcribeWav(baseUrl, apiKey, wav, {
        locale: typeof navigator !== "undefined" ? navigator.language : "zh-CN",
        contextHint: contextHintRef.current,
      });
      contextHintRef.current = null;
      if (!result.ok) {
        onError?.(result.message);
        setPhase("idle");
        return;
      }
      if (result.text) {
        if (isTauri()) {
          const ok = await injectIntoForeground(result.text);
          if (!ok) await copyToClipboard(result.text);
        } else {
          await copyToClipboard(result.text);
        }
      }
      setPhase("idle");
    },
    [apiKey, baseUrl, onError]
  );

  const start = useCallback(async () => {
    if (phase !== "idle") return;
    try {
      if (isTauri()) {
        await captureInputTarget();
        contextHintRef.current = await getDictationContextHint();
        const key = apiKey.trim();
        const root = baseUrl.trim();
        if (key && root) {
          await invoke("dictation_start", {
            baseUrl: root,
            apiKey: key,
            locale: navigator.language || "zh-CN",
            contextHint: contextHintRef.current,
          });
          streamActiveRef.current = true;
          setPhase("recording");
          return;
        }
      } else {
        contextHintRef.current = null;
      }
      const rec = new MicRecorder();
      await rec.start();
      recorderRef.current = rec;
      setPhase("recording");
    } catch (e) {
      streamActiveRef.current = false;
      onError?.(e instanceof Error ? e.message : "无法开始听写");
      setPhase("idle");
    }
  }, [apiKey, baseUrl, onError, phase]);

  const stopAndSubmit = useCallback(async () => {
    if (phase !== "recording") return;
    if (streamActiveRef.current && isTauri()) {
      setPhase("processing");
      try {
        await invoke("dictation_stop");
        contextHintRef.current = null;
      } catch (e) {
        streamActiveRef.current = false;
        onError?.(e instanceof Error ? e.message : "听写结束失败");
        setPhase("idle");
      }
      return;
    }
    const rec = recorderRef.current;
    if (!rec) return;
    recorderRef.current = null;
    try {
      const wav = await rec.stop();
      if (wav.size < 44) {
        onError?.("录音太短");
        setPhase("idle");
        return;
      }
      await finishWithBlob(wav);
    } catch {
      onError?.("录音结束失败");
      setPhase("idle");
    }
  }, [finishWithBlob, onError, phase]);

  const cancel = useCallback(async () => {
    if (streamActiveRef.current && isTauri()) {
      try {
        await invoke("dictation_cancel");
      } catch {
        /* ignore */
      }
      streamActiveRef.current = false;
      contextHintRef.current = null;
      setPhase("idle");
      return;
    }
    recorderRef.current?.cancel();
    recorderRef.current = null;
    setPhase("idle");
  }, []);

  const toggle = useCallback(() => {
    if (phase === "recording") void stopAndSubmit();
    else if (phase === "idle") void start();
  }, [phase, start, stopAndSubmit]);

  return {
    phase,
    recording: phase === "recording",
    processing: phase === "processing",
    start,
    stopAndSubmit,
    cancel,
    toggle,
  };
}
