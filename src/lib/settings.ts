const PREFIX = "rt.";

/** F9 较少与输入法/系统冲突；旧版 Ctrl+Shift+Space 在中文 Windows 上常被占用 */
export const DEFAULT_HOTKEY = "F9";
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
