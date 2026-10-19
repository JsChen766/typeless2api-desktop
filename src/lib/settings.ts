const PREFIX = "rt.";

/** macOS 默认 Fn；Windows 默认右 Alt；其它平台 F9 */
export const DEFAULT_HOTKEY =
  typeof navigator !== "undefined" && /Mac/i.test(navigator.platform)
    ? "Fn"
    : typeof navigator !== "undefined" && /Win/i.test(navigator.platform)
      ? "AltRight"
      : "F9";
export const DEFAULT_BASE_URL = "http://localhost:8787";

function key(name: string) {
  return `${PREFIX}${name}`;
}

export function loadBaseUrl(): string {
  return localStorage.getItem(key("baseUrl")) ?? DEFAULT_BASE_URL;
}

export function saveBaseUrl(value: string) {
  localStorage.setItem(key("baseUrl"), value);
}

export function loadApiKey(): string {
  return localStorage.getItem(key("apiKey")) ?? "";
}

export function saveApiKey(value: string) {
  localStorage.setItem(key("apiKey"), value);
}

export function loadHotkey(): string {
  const stored = localStorage.getItem(key("hotkey"));
  if (!stored || stored === "Ctrl+Shift+Space") return DEFAULT_HOTKEY;
  return stored;
}

export function saveHotkey(value: string) {
  localStorage.setItem(key("hotkey"), value);
}
