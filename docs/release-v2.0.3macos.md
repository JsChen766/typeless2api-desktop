# v2.0.3macos

macOS（Apple Silicon）再发行版，应用版本 **2.0.3**。功能与当前 main 分支一致，本版主要补齐开源许可。

## 本版内容

- 仓库与 `.app` 内附带 [MIT](../LICENSE) 许可文本（`Contents/Resources/LICENSE`）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。麦克风与辅助功能权限与先前版本相同。

## 验证

- GitHub Actions 在 `macos-14` 上构建 DMG。
- 挂载 DMG 或打开 App 后确认 `Contents/Resources/LICENSE` 存在。
