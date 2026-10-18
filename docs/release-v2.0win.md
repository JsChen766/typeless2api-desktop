# v2.0win

Typeless Windows x64 安装版，应用版本 2.0.0。基于 v1.2 的客户端功能构建。

## 内容

- F9 快捷键启动听写，胶囊中的 ✓ 完成、× 取消。
- 在设置页配置 Gateway 地址与 API Key，并测试连接。
- 识别文本复制到剪贴板，尝试回填到听写开始时的输入窗口。
- 设置页支持开机自启与真正退出客户端。

## 安装

运行 `typeless-client_2.0.0_x64-setup.exe`。安装包未做 Windows 代码签名，系统可能显示发行者未知。首次使用时允许麦克风访问，并在目标输入框测试回填。

## 验证范围

Windows GitHub Actions 已完成前端与 Rust 构建、Gateway 自动测试及 NSIS 安装包检查。Windows 实机启动、录音及跨应用回填尚待验证。
