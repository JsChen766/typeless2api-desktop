# Typeless2api 桌面端（Tauri 2）

本仓库源码采用 [MIT](LICENSE) 许可。随附的 `libopuschunk` 等原生库另有其使用条件，再分发时请自行确认权利。

本机听写客户端：麦克风 → 本机 `libopuschunk` 编码 → 连 Gateway（只配地址 + API Key）→ 把识别文本贴回开始听写时的前台窗口。

**不登录 Typeless**，不保存上游 token。

## 每天怎么用

1. 启动后只在托盘。
2. 左键托盘打开设置：填 Gateway 地址和 `sk_live_` API Key，点测试连接。
3. 任意输入框按 **F9** 开始听写；底部胶囊 × 取消 / ✓ 结束。
4. Windows 用 Ctrl+V 注入；Mac 用 Cmd+V（需麦克风 + 辅助功能）。

公网试用地址（明文 HTTP，仅内部）：`http://122.152.251.247`  
不要加 `:8787`。API Key 私下要，不要提交进仓库。

## 开发

- Windows 10+ 或 macOS 11+（仓库里的 `.dylib` 是 Apple Silicon arm64）
- Node.js 20+
- [Rust / Tauri 前置](https://v2.tauri.app/start/prerequisites/)

```powershell
npm install
npm run tauri:dev
```

`src-tauri/resources` 已带 `libopuschunk.dll` 与 `libopuschunk.dylib`，不必再装 Typeless。上述二进制不属于 MIT 授权范围。
