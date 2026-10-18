# v1.2macos

macOS 版 Typeless 客户端。安装包版本号为 1.2.0，适用于 Apple Silicon Mac。

## 本版内容

- 重排设置页，明确显示 Gateway 连接、API Key 验证结果和听写快捷键。
- 修正公开健康检查不提供听写模式时显示 `unknown` 的问题。
- 点击程序坞图标可显示设置页，再次点击可隐藏；报错时设置页沿用程序坞图标上方的位置。
- 将听写胶囊上移，避开程序坞。
- “退出客户端”仍会真正退出应用。

## 安装与权限

打开 DMG，将 `typeless-client.app` 拖入“应用程序”。首次听写需要允许麦克风与辅助功能权限。本地构建采用临时签名，未进行 Apple 公证；替换 App 后 macOS 可能要求重新授予辅助功能权限。

## 验证

- `npm run build`
- `npm run test:gateway`（4 项通过）
- `cargo test --locked --offline window_position::tests --lib`
- DMG 镜像校验通过，镜像内 App 版本为 1.2.0，临时签名校验通过。
- 手动确认：程序坞图标第一次点击显示设置页，第二次点击隐藏；窗口位于图标上方，底边贴近程序坞上沿。

DMG SHA-256：`97750d2fd0184e585b030aee453f3519e8abaf90b3d78c61cbc2ece32d81e236`
