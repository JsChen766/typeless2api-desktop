import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { DictationPill } from "@/components/DictationPill";
import { emitPillAction, listenPillPhase, type PillState } from "@/lib/pillWindow";
import "./index.css";

function PillApp() {
  const [state, setState] = useState<PillState>({ phase: "idle" });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenPillPhase((next) => {
      if (next.phase === "recording" || next.phase === "processing" || next.phase === "idle" || next.phase === "error") {
        setState(next);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  return (
    <DictationPill
      visible={state.phase !== "idle"}
      processing={state.phase === "processing"}
      error={state.phase === "error" ? state.error ?? "听写失败，请重试" : null}
      onCancel={() => void emitPillAction("cancel")}
      onComplete={() => void emitPillAction("complete")}
      className="!bottom-auto !left-auto !translate-x-0 static"
    />
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PillApp />
  </StrictMode>
);
