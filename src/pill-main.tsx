import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { DictationPill } from "@/components/DictationPill";
import type { DictationPhase } from "@/hooks/useDictation";
import { emitPillAction, listenPillPhase } from "@/lib/pillWindow";
import "./index.css";

function PillApp() {
  const [phase, setPhase] = useState<DictationPhase>("idle");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenPillPhase((p) => {
      if (p === "recording" || p === "processing" || p === "idle") {
        setPhase(p);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  return (
    <DictationPill
      visible={phase !== "idle"}
      processing={phase === "processing"}
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
