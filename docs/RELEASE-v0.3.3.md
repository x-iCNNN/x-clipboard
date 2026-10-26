# X Clipboard v0.3.3 — SSH 终端回贴兼容 / SSH terminal paste compatibility

## 中文

- Windows 新增 **设置 → 快捷键 → 回贴按键**，可选择 `Ctrl+V` 或 `Shift+Insert`。
- DBx Shell 插件等不支持 `Ctrl+V` 粘贴的终端，请选择 **Shift+Insert**。用户已确认其 DBx 插件中 `Ctrl+V` 不生效、`Shift+Insert` 可以粘贴。
- 默认仍为 `Ctrl+V`。选择立即保存，重启后恢复，并应用于所有自动回贴；不会仅凭进程名称猜测当前是否为终端插件。
- 修正 Insert 键注入的扩展键标志，确保 Shift 修饰键在 Insert 按下时保持有效。焦点恢复、目标校验及部分输入失败时的按键释放逻辑继续保留。
- 历史正文、纯文本回贴、表情和符号使用同一已配置按键；原有 Shift+Enter 仍表示以纯文本回贴。

验证：40 项核心测试通过，2 项可选探针按设计跳过。原生模拟终端在旧路径下复现 `^V`，选择 Shift+Insert 后实际插入测试正文；默认 Ctrl+V、焦点保留、目标中断和控件关闭回归均通过。已检查中英文、明暗主题界面。**DBx 插件本身的端到端效果仍需更新后实机确认。**

## English

- Added **Settings → Shortcuts → Paste shortcut** on Windows, with `Ctrl+V` and `Shift+Insert` choices.
- Select **Shift+Insert** for terminal plugins that do not use Ctrl+V for pasting. The user confirmed that DBx's Shell plugin accepts Shift+Insert but not Ctrl+V.
- Ctrl+V remains the default. The choice is saved immediately, survives restart, and applies to all automatic pastes. No process-name guesses are used to identify an embedded terminal.
- Mark Insert as an extended key during input injection so Shift remains held when Insert is pressed. Existing focus restoration, target validation, and partial-input cleanup are retained.
- History, plain-text paste, emoji, and symbols share the configured chord. Shift+Enter continues to request plain-text content.

Validation: 40 core tests passed; 2 opt-in probes intentionally ignored. A native terminal fixture reproduced `^V` with the old path and received actual clipboard text using Shift+Insert. Default Ctrl+V, focus preservation, interrupted targets, and destroyed controls passed regression checks. Chinese/English light/dark native views were inspected. **End-to-end verification inside DBx itself remains pending user retesting.**

## 下载 / Downloads

- `*-setup.exe`：自定义安装版，可选择安装目录。 / Custom installer with selectable directory.
- `*.zip`：便携版，含 PDF 预览运行库。 / Portable build including PDF preview runtime.
- `*.sha256`：两种发布包的校验文件。 / SHA-256 checksums for both packages.
