import { gatewayFetch } from "@/lib/http";

function trimSlash(url: string): string {
  return url.replace(/\/+$/, "");
}

const prewarmed = new Set<string>();
const prewarming = new Map<string, Promise<void>>();

export async function prewarmDictationUpstream(
  baseUrl: string,
  apiKey: string
): Promise<void> {
  const root = trimSlash(baseUrl.trim());
  const key = apiKey.trim();
  if (!root || !key) return;
  const id = `${root}\n${key}`;
  if (prewarmed.has(id)) return;
  const pending = prewarming.get(id);
  if (pending) return pending;
  const request = (async () => {
    try {
      const result = await gatewayFetch(`${root}/v1/dictation/prewarm`, {
        method: "POST",
        headers: { Authorization: `Bearer ${key}` },
      });
      if (result.ok) prewarmed.add(id);
    } catch {
      /* 预热失败不阻断使用；下次可重试 */
    } finally {
      prewarming.delete(id);
    }
  })();
  prewarming.set(id, request);
  return request;
}

export async function testGatewayConnection(
  baseUrl: string,
  apiKey: string
): Promise<{ ok: true } | { ok: false; message: string }> {
  const root = trimSlash(baseUrl.trim());
  if (!root) {
    return { ok: false, message: "请填写 Gateway 地址" };
  }
  const key = apiKey.trim();
  if (!key) {
    return { ok: false, message: "请填写 API Key" };
  }

  try {
    const health = await gatewayFetch(`${root}/health`, {
      method: "GET",
      signal: AbortSignal.timeout(8000),
    });
    if (!health.ok) {
      return { ok: false, message: `Gateway 不可用（${health.status}）` };
    }
    const body = (await health.json()) as {
      status?: string;
    };
    if (body.status !== "ok") {
      return { ok: false, message: "健康检查响应异常" };
    }

    const probe = await gatewayFetch(`${root}/v1/transcribe`, {
      method: "POST",
      headers: { Authorization: `Bearer ${key}` },
      signal: AbortSignal.timeout(8000),
    });
    if (probe.status === 401) {
      return { ok: false, message: "API Key 无效或已吊销" };
    }
    if (probe.status === 422 || probe.status === 400) {
      return { ok: true };
    }
    if (probe.ok) {
      return { ok: true };
    }
    return { ok: false, message: `验证失败（${probe.status}）` };
  } catch (err) {
    const timedOut = err instanceof Error && (err.name === "TimeoutError" || err.name === "AbortError");
    return {
      ok: false,
      message: timedOut
        ? "连接 Gateway 超时，请检查地址和网络"
        : "无法连接 Gateway，请检查地址、网络和服务状态",
    };
  }
}
