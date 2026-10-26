# v0.3.1 — Project link and optional tray icon / 项目链接与可隐藏托盘

## Changes / 变更

- Settings → About → Open source and help now displays the project URL and a GitHub button: https://github.com/x-iCNNN/x-clipboard.
- 设置 → 关于 → 开源与帮助中新增项目地址及 GitHub 按钮。
- Settings → General → Startup and windows now includes **Show tray icon**. Changes apply immediately and are saved in the encrypted settings database. Existing installations keep the icon enabled by default.
- 设置 → 通用 → 启动与窗口中新增**显示托盘图标**，即时生效并持久保存；旧版本升级后默认显示。
- Hiding the icon keeps the application running in the background. Use your configured global shortcut to open the quick panel, then its Settings button to restore the icon. Window-close behavior is now described independently of tray visibility.
- 隐藏图标后仍在后台运行。使用已配置的全局快捷键打开快捷面板，再通过设置按钮恢复图标；关闭窗口的说明不再暗示图标始终可见。
- New labels are included in all nine bundled UI languages. No clipboard data or WebDAV protocol changes.
- 新增文案已覆盖内置的九种界面语言；剪贴板数据及 WebDAV 协议没有变化。

## Validation / 验证

- 38 core tests passed; 2 opt-in probes intentionally ignored. Rustfmt and Clippy (`--all-targets --features ui-capture -D warnings`) passed.
- 核心测试 38 项通过，2 项可选探针按设计跳过；格式检查与 Clippy 通过。
- Native Windows QA exercised show → hide → show → hide, checking actual Shell icon rectangles each time. The tray is created only after saved settings load; encrypted database reopen preserves the hidden setting.
- 原生 Windows 验收完成显示 → 隐藏 → 显示 → 隐藏，并逐次查询 Shell 图标位置；配置加载后才创建图标，隐藏状态通过加密数据库重新打开验证。
- Chinese/English light/dark settings and About pages were captured and visually inspected. Global shortcut behavior itself was not re-injected as part of this change.
- 已采集并检查中英文、明暗主题的通用与关于界面；本次未重新注入测试全局快捷键。

## Downloads / 下载

- `*-setup.exe`: custom installer with selectable installation directory. / 支持选择安装目录的自定义安装版。
- `*.zip`: portable application and PDF preview runtime. / 便携版，包含 PDF 预览运行库。
- `*.sha256`: SHA-256 checksums for both packages. / 两种安装包的 SHA-256 校验文件。
