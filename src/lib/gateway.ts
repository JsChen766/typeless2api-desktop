import { gatewayFetch } from "@/lib/http";

function trimSlash(url: string): string {
  return url.replace(/\/+$/, "");
}

export async function prewarmDictationUpstream(
  baseUrl: string,
  apiKey: string
): Promise<void> {
  const root = trimSlash(baseUrl.trim());
  const key = apiKey.trim();
  if (!root || !key) return;
  try {
    await gatewayFetch(`${root}/v1/dictation/prewarm`, {
      method: "POST",
      headers: { Authorization: `Bearer ${key}` },
    });
  } catch {
    /* 预热失败不阻断使用 */
  }
}

export type GatewayHealth = {
  dictation_mode?: string;
  account_pool_size?: number;
};

export async function testGatewayConnection(
  baseUrl: string,
  apiKey: string
): Promise<{ ok: true; health: GatewayHealth } | { ok: false; message: string }> {
  const root = trimSlash(baseUrl.trim());
  if (!root) {
    return { ok: false, message: "请填写 Gateway 地址" };
  }
  const key = apiKey.trim();
  if (!key) {
    return { ok: false, message: "请填写 API Key" };
  }

  try {
    const health = await gatewayFetch(`${root}/health`, { method: "GET" });
    if (!health.ok) {
      return { ok: false, message: `Gateway 不可用（${health.status}）` };
    }
    const body = (await health.json()) as {
      status?: string;
      dictation_mode?: string;
      account_pool_size?: number;
    };
    if (body.status !== "ok") {
      return { ok: false, message: "健康检查响应异常" };
    }
    const healthInfo: GatewayHealth = {
      dictation_mode: body.dictation_mode,
      account_pool_size: body.account_pool_size,
    };

    const probe = await gatewayFetch(`${root}/v1/transcribe`, {
      method: "POST",
      headers: { Authorization: `Bearer ${key}` },
    });
    if (probe.status === 401) {
      return { ok: false, message: "API Key 无效或已吊销" };
    }
    if (probe.status === 422 || probe.status === 400) {
      return { ok: true, health: healthInfo };
    }
    if (probe.ok) {
      return { ok: true, health: healthInfo };
    }
    return { ok: false, message: `验证失败（${probe.status}）` };
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err);
    return {
      ok: false,
      message: `无法连接 Gateway（${detail}）。请确认服务在 8787 运行，并重启 Gateway 以应用 CORS`,
    };
  }
}

export type TranscribeOptions = {
  locale?: string;
  contextHint?: {
    app_name: string;
    window_title: string;
    app_identifier: string;
  } | null;
};

export async function transcribeWav(
  baseUrl: string,
  apiKey: string,
  wav: Blob,
  options?: TranscribeOptions
): Promise<{ ok: true; text: string } | { ok: false; message: string }> {
  const root = trimSlash(baseUrl.trim());
  const key = apiKey.trim();
  if (!root || !key) {
    return { ok: false, message: "请先填写地址与 Key" };
  }
  try {
    const form = new FormData();
    form.append("file", wav, "recording.wav");
    const locale = options?.locale?.trim();
    if (locale) form.append("locale", locale);
    const hint = options?.contextHint;
    if (hint) {
      form.append(
        "context_hint",
        JSON.stringify({
          app_name: hint.app_name,
          window_title: hint.window_title,
          app_identifier: hint.app_identifier,
        })
      );
    }
    const res = await gatewayFetch(`${root}/v1/transcribe`, {
      method: "POST",
      headers: { Authorization: `Bearer ${key}` },
      body: form,
    });
    const body = (await res.json().catch(() => ({}))) as {
      detail?: unknown;
      data?: { refined_text?: string };
    };
    if (!res.ok) {
      let message = `转写失败（${res.status}）`;
      const d = body.detail;
      if (typeof d === "string") message = d;
      else if (Array.isArray(d)) {
        message = d
          .map((item) =>
            typeof item === "object" && item && "msg" in item
              ? String((item as { msg?: string }).msg)
              : ""
          )
          .filter(Boolean)
          .join("; ");
      }
      return { ok: false, message };
    }
    const text = body.data?.refined_text ?? "";
    return { ok: true, text };
  } catch {
    return { ok: false, message: "转写请求失败" };
  }
}
