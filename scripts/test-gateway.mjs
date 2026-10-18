import assert from "node:assert/strict";
import test from "node:test";
import { build } from "esbuild";

const bundle = await build({
  entryPoints: ["src/lib/gateway.ts"],
  bundle: true,
  format: "esm",
  platform: "node",
  write: false,
  plugins: [{
    name: "mock-http",
    setup(builder) {
      builder.onResolve({ filter: /^@\/lib\/http$/ }, () => ({ path: "http", namespace: "mock" }));
      builder.onLoad({ filter: /.*/, namespace: "mock" }, () => ({
        contents: "export const gatewayFetch = (...args) => globalThis.mockGatewayFetch(...args);",
        loader: "js",
      }));
    },
  }],
});
const source = Buffer.from(bundle.outputFiles[0].text).toString("base64");
const { prewarmDictationUpstream, testGatewayConnection } = await import(`data:text/javascript;base64,${source}`);

test("invalid API Key is rejected after a healthy Gateway response", async () => {
  globalThis.mockGatewayFetch = async (url) => url.endsWith("/health")
    ? new Response(JSON.stringify({ status: "ok" }), { status: 200 })
    : new Response("", { status: 401 });
  assert.deepEqual(await testGatewayConnection("http://gateway", "invalid"), {
    ok: false,
    message: "API Key 无效或已吊销",
  });
});

test("valid API Key succeeds when public health only returns status", async () => {
  globalThis.mockGatewayFetch = async (url) => url.endsWith("/health")
    ? new Response(JSON.stringify({ status: "ok" }), { status: 200 })
    : new Response(JSON.stringify({ detail: "use WebSocket" }), { status: 400 });
  assert.deepEqual(await testGatewayConnection("http://gateway", "valid"), { ok: true });
});

test("offline and timeout errors tell the user what to check", async () => {
  globalThis.mockGatewayFetch = async () => { throw new TypeError("fetch failed"); };
  const offline = await testGatewayConnection("http://offline", "key");
  assert.equal(offline.ok, false);
  assert.match(offline.message, /地址、网络和服务状态/);

  globalThis.mockGatewayFetch = async () => { throw new DOMException("timed out", "TimeoutError"); };
  const timeout = await testGatewayConnection("http://slow", "key");
  assert.equal(timeout.ok, false);
  assert.match(timeout.message, /超时/);
});

test("prewarm is single-flight, runs once on success, and can retry a failure", async () => {
  let calls = 0;
  globalThis.mockGatewayFetch = async () => {
    calls++;
    return new Response(null, { status: 204 });
  };
  await Promise.all([
    prewarmDictationUpstream("http://gateway", "key"),
    prewarmDictationUpstream("http://gateway", "key"),
  ]);
  await prewarmDictationUpstream("http://gateway", "key");
  assert.equal(calls, 1);

  globalThis.mockGatewayFetch = async () => {
    calls++;
    throw new TypeError("offline");
  };
  await prewarmDictationUpstream("http://retry", "key");
  await prewarmDictationUpstream("http://retry", "key");
  assert.equal(calls, 3);
});
