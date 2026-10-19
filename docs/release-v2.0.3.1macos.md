# v2.0.3.1macos

与 v2.0.3macos 功能相同；修复 **Apple Silicon** 从 GitHub 安装时提示「已损坏」的问题（CI 构建启用 ad-hoc 代码签名）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。

若仍被拦截，执行 `xattr -cr /Applications/typeless-client.app` 或右键应用 → **打开**。
