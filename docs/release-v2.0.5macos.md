# v2.0.5macos

macOS（Apple Silicon），应用版本 **2.0.5**。

## 本版内容

- 听写胶囊声纹（Mac）：按音量驱动合成起伏，低音量起伏大、高音量更稳，便于确认「正在收音」。
- Windows 声纹仍为跟随麦克风时间环；Mac 仅改前端显示，不改听写链路。
- Fn / Globe 单按听写等同 2.0.4；ad-hoc 签名（`signingIdentity: "-"`）。

## 安装

打开 DMG，将 `typeless-client.app` 拖入「应用程序」。若提示无法验证开发者，在系统设置 → 隐私与安全性中允许打开，或执行 `xattr -cr /Applications/typeless-client.app`。
