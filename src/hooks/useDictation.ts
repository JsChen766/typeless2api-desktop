import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type DictationPhase = "idle" | "recording" | "processing" | "error";

export type DictationEvent = {
  phase: DictationPhase;
  text?: string | null;
  error?: string | null;
  mic_peak?: number | null;
};

export function useDictation(onEvent: (event: DictationEvent) => void) {
  const [phase, setPhase] = useState<DictationPhase>("idle");

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<DictationEvent>("dictation-event", (e) => {
      if (e.payload.phase === "idle" || e.payload.phase === "recording" || e.payload.phase === "processing") {
        setPhase(e.payload.phase);
        onEvent(e.payload);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [onEvent]);

  return {
    phase,
    recording: phase === "recording",
    processing: phase === "processing",
  };
}
