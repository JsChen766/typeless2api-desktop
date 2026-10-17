import { invoke } from "@tauri-apps/api/core";

export async function captureInputTarget(): Promise<void> {
  await invoke("capture_input_target");
}

export async function injectIntoForeground(text: string): Promise<boolean> {
  try {
    await invoke("inject_text", { text });
    return true;
  } catch {
    return false;
  }
}
