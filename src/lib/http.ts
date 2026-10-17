import { isTauri } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

/** Tauri WebView 跨域访问本机 Gateway 需走插件 HTTP，避免 CORS 拦截 */
export function gatewayFetch(
  input: string,
  init?: RequestInit
): Promise<Response> {
  if (isTauri()) {
    return tauriFetch(input, init);
  }
  return fetch(input, init);
}
