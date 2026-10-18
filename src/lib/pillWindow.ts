import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { DictationPhase } from "@/hooks/useDictation";

export type PillAction = "cancel" | "complete";
export type PillState = { phase: DictationPhase; error?: string | null };

export async function syncPillWithPhase(phase: DictationPhase): Promise<void> {
  const visible = phase !== "idle";
  await invoke("set_pill_visible", { visible, phase });
}

export function listenPillPhase(
  handler: (state: PillState) => void
): Promise<() => void> {
  return listen<PillState>("pill-phase", (e) => {
    handler(e.payload);
  });
}

export async function emitPillAction(action: PillAction): Promise<void> {
  await emit("pill-action", { action });
}

export function listenPillAction(
  handler: (action: PillAction) => void
): Promise<() => void> {
  return listen<{ action: PillAction }>("pill-action", (e) => {
    handler(e.payload.action);
  });
}

export function isPillWindow(): boolean {
  try {
    return getCurrentWebviewWindow().label === "pill";
  } catch {
    return false;
  }
}
