# v2.0.4macos

macOS（Apple Silicon），应用版本 **2.0.4**。

## 本版内容

- **Fn / Globe 单按**切换听写（需辅助功能权限）；**Fn + 其它键**组合行为不变。
- 建议：系统设置 → 键盘 → 将「按 🌐/Fn 键以」设为 **无操作**。
- CI 构建含 ad-hoc 签名（`signingIdentity: "-"`）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。若提示已损坏，执行 `xattr -cr /Applications/typeless-client.app`。
