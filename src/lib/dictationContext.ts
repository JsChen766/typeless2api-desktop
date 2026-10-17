import { invoke, isTauri } from "@tauri-apps/api/core";

export type DictationCursorState = {
  cursor_position: number;
  has_text_selected: boolean;
  selected_text: string;
  text_before_cursor: string;
  text_after_cursor: string;
  full_field_content: string;
};

export type DictationContextHint = {
  app_name: string;
  window_title: string;
  app_identifier: string;
  app_type?: string;
  process_id?: number;
  window_id?: number;
  cursor_state?: DictationCursorState;
};

export async function getDictationContextHint(): Promise<DictationContextHint | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<DictationContextHint>("dictation_context_hint");
  } catch {
    return null;
  }
}
