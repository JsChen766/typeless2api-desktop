import { isTauri } from "@tauri-apps/api/core";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";

const MODIFIER_ONLY_KEYS = new Set([
  "Fn",
  "AltRight",
  "AltLeft",
  "ControlRight",
  "ControlLeft",
  "ShiftRight",
  "ShiftLeft",
  "MetaRight",
  "MetaLeft",
]);

const MODIFIER_LABELS: Record<string, string> = {
  Fn: "Fn",
  AltRight: "右 Alt",
  AltLeft: "左 Alt",
  ControlRight: "右 Ctrl",
  ControlLeft: "左 Ctrl",
  ShiftRight: "右 Shift",
  ShiftLeft: "左 Shift",
  MetaRight: "右 Win",
  MetaLeft: "左 Win",
};

export function isExclusiveModifierHotkey(shortcut: string): boolean {
  const key = shortcut.trim();
  if (key === "Fn" || /^fn$/i.test(key)) return true;
  return MODIFIER_ONLY_KEYS.has(key) || /^(Alt|Control|Shift|Meta)(Left|Right)$/i.test(key);
}

export function formatHotkeyLabel(shortcut: string): string {
  const trimmed = shortcut.trim();
  if (MODIFIER_LABELS[trimmed]) {
    return MODIFIER_LABELS[trimmed];
  }
  const normalized = shortcut.replace(/CommandOrControl/g, "Ctrl");
  if (typeof navigator === "undefined" || !/Mac/i.test(navigator.platform)) {
    return normalized.replace(/\+/g, " + ");
  }
  const symbols: Record<string, string> = {
    Ctrl: "⌘",
    Meta: "⌘",
    Alt: "⌥",
    Shift: "⇧",
  };
  return normalized.split("+").map((part) => symbols[part] ?? part).join("");
}

/** Tauri global-shortcut 需要 CommandOrControl，而不是 Ctrl */
export function toTauriShortcut(shortcut: string): string {
  return shortcut
    .replace(/(^|\+)Ctrl(\+|$)/g, "$1CommandOrControl$2")
    .replace(/(^|\+)Meta(\+|$)/g, "$1CommandOrControl$2");
}

function primaryKeyFromEvent(e: KeyboardEvent): string | null {
  const code = e.code;
  if (code.startsWith("Key") && code.length === 4) return code.slice(3).toUpperCase();
  if (code.startsWith("Digit") && code.length === 6) return code.slice(5);
  if (/^F\d+$/.test(code)) return code;
  const named: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    AltRight: "AltRight",
    AltLeft: "AltLeft",
    ControlRight: "ControlRight",
    ControlLeft: "ControlLeft",
    ShiftRight: "ShiftRight",
    ShiftLeft: "ShiftLeft",
    MetaRight: "MetaRight",
    MetaLeft: "MetaLeft",
  };
  if (e.key === "Fn" || e.code === "Fn") return "Fn";
  if (named[code]) return named[code];
  if (e.key === " " || e.key === "Spacebar") return "Space";
  if (e.key.length === 1) return e.key.toUpperCase();
  if (/^F\d+$/i.test(e.key)) return e.key.toUpperCase();
  return null;
}

export function keyboardEventToShortcut(e: KeyboardEvent): string | null {
  if (e.key === "Escape") return null;
  const primary = primaryKeyFromEvent(e);
  if (!primary) return null;
  if (MODIFIER_ONLY_KEYS.has(primary)) {
    return primary;
  }
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(primary);
  return parts.join("+");
}

let hotkeyHeld = false;

export async function bindGlobalHotkey(
  shortcut: string,
  onPressed: () => void
): Promise<void> {
  if (!isTauri()) return;
  const normalized = toTauriShortcut(shortcut.trim());
  if (!normalized) {
    throw new Error("快捷键无效");
  }
  try {
    await unregisterAll();
  } catch {
    /* ignore */
  }
  hotkeyHeld = false;
  await register(normalized, (event) => {
    if (event.state === "Released") {
      hotkeyHeld = false;
      return;
    }
    if (event.state !== "Pressed" || hotkeyHeld) return;
    hotkeyHeld = true;
    onPressed();
  });
}

export async function unbindGlobalHotkey(): Promise<void> {
  if (!isTauri()) return;
  try {
    await unregisterAll();
  } finally {
    hotkeyHeld = false;
  }
}
