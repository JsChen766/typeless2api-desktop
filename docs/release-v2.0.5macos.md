# v2.0.5macos

macOS（Apple Silicon），应用版本 **2.0.5**。

## 本版内容

- 听写胶囊声纹：中间更突出、两侧更细，随说话不规则起伏。
- 降低麦克风电平显示增益，避免声纹长期满格（内置麦电平偏高时更明显）。
- Fn / Globe 单按听写等同 2.0.4；ad-hoc 签名（`signingIdentity: "-"`）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。若提示无法验证开发者，在系统设置 → 隐私与安全性中允许打开，或执行 `xattr -cr /Applications/typeless-client.app`。
