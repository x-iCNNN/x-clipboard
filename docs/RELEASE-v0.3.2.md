# X Clipboard v0.3.2 — 鼠标中键唤起 / Middle-click activation

## 中文

- 新增 Windows **设置 → 快捷键 → 鼠标中键打开快捷面板**，默认关闭，开启后点击滚轮并松开即可唤起。
- 设置立即生效并保存在本机加密数据库中，重启后自动恢复。
- 开启后会接管中键点击，避免原应用的自动滚屏、打开或关闭标签页等动作同时触发；关闭选项后卸载鼠标监听，恢复中键原有行为。
- 复用现有快捷面板与原窗口回贴目标。键盘快捷键仍可使用，滚轮滚动不在拦截范围内。
- 新增文案覆盖内置九种界面语言。

验证：39 项核心测试通过，2 项可选探针按设计跳过；格式检查与 Clippy 通过。Windows 原生输入验收覆盖默认关闭、反复开关、启用时拦截点击、禁用时恢复点击、原窗口目标保持，以及重启后恢复。已检查中英文、明暗主题原生截图。

## English

- Added an opt-in Windows setting: **Settings → Shortcuts → Open quick panel with middle mouse button**. Press and release the mouse wheel to open the panel.
- Changes apply immediately, persist in encrypted local settings, and are restored on restart. Existing installations keep this option disabled by default.
- Enabling the option takes over middle clicks, preventing the original application from also starting auto-scroll or opening/closing tabs. Disabling it removes the mouse hook and restores normal middle-click behavior.
- Reuses the existing quick panel and preserves the original paste target. Keyboard shortcuts remain available; wheel scrolling is not intercepted.
- New labels are translated into all nine bundled UI languages.

Validation: 39 core tests passed; 2 opt-in probes intentionally ignored. Rustfmt and Clippy passed. Native Windows input QA covered the disabled default, repeated toggles, click consumption/restoration, original-target preservation, and restart restoration. Chinese/English light/dark native screenshots were inspected.

## 下载 / Downloads

- `*-setup.exe`：可选择安装目录的自定义安装版。 / Custom installer with selectable installation directory.
- `*.zip`：便携版，含 PDF 预览运行库。 / Portable build including the PDF preview runtime.
- `*.sha256`：安装版及便携版的 SHA-256 校验文件。 / SHA-256 checksums for both packages.
