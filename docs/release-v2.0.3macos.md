# v2.0.3macos

macOS（Apple Silicon），应用版本 **2.0.3**。

## 本版内容

- 听写胶囊缩小与真实麦克风声纹（同 Windows v2.0.3）。
- `.app` 内 `Contents/Resources/LICENSE`（MIT）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。需麦克风与辅助功能权限。

若提示「已损坏」或无法打开（GitHub 下载的未公证包在 Apple Silicon 上常见），在终端执行：

```bash
xattr -cr /Applications/typeless-client.app
```

或在「系统设置 → 隐私与安全性」中允许该应用；也可对 `.app` **右键 → 打开** 首次启动。

自 **v2.0.3.1macos** 起 CI 构建已启用 ad-hoc 签名（`signingIdentity: "-"`），一般不再出现「已损坏」提示。
