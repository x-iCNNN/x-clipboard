# X Clipboard

> **Clipboard history that stays on your device.** Capture, search, preview, and paste again with a native desktop app built in Rust.
>
> **让剪贴板历史留在本机。** 使用 Rust 原生桌面应用采集、搜索、预览并快速回贴。

<p align="center">
  <img src="assets/branding/app-icon-128.png" alt="X Clipboard icon" width="96" height="96">
</p>

<p align="center">
  <strong>Windows preview available · Windows 预览版可用</strong><br>
  macOS and Linux builds are in progress and have not completed native release validation.<br>
  macOS 与 Linux 构建仍在进行，尚未完成原生发布验收。
</p>

<p align="center">
  <a href="#features">Features</a> · <a href="#screenshots">Screenshots</a> · <a href="#getting-started">Get started</a> · <a href="#roadmap">Roadmap</a> · <a href="docs/README.md">Documentation</a>
</p>

---

## Why X Clipboard / 为什么做 X Clipboard

The clipboard is a tiny part of the desktop workflow, but losing a copied link, code snippet, image, or file reference interrupts the whole task. X Clipboard keeps a searchable local history and makes older items easy to preview and paste again.

剪贴板只占桌面工作流的一小部分，但丢失链接、代码片段、图片或文件引用，就可能打断整个任务。X Clipboard 在本机保存可搜索的历史记录，让旧内容可以重新预览和回贴。

This project focuses on a few practical principles:

本项目聚焦于几个实际原则：

- **Local-first / 本机优先** — history is stored locally in an encrypted database; there is no account or cloud service in the current build.
- **Native desktop workflow / 原生桌面体验** — a keyboard shortcut opens a compact panel, while a separate window handles search and organization.
- **Keep formats useful / 保留格式价值** — capture supported clipboard formats together, including text, HTML/RTF, images, and file references.
- **Clear boundaries / 明确能力边界** — file entries keep references rather than copying the original files; sync is planned, not implemented.

## Features / 功能

| | English | 中文 |
|---|---|---|
| **Capture** | Text, HTML/RTF, images, and file references; Windows file history is replayed with copy semantics. | 采集文本、HTML/RTF、图片和文件引用；Windows 文件历史按复制语义回放。 |
| **Find** | Full-text search, exact-format deduplication, and recently copied ordering. | 全文搜索、按格式精确去重、最近复制排序。 |
| **Organize** | Pin, favorite, multi-select, delete, and clear unprotected history. | 置顶、收藏、多选、删除和清理未受保护的历史记录。 |
| **Paste again** | Re-paste a selected history item; the quick panel attempts to restore the previous target control first. | 重新回贴选中的历史内容；快捷面板会先尝试恢复原目标控件焦点。 |
| **Preview** | Text/code, Markdown, HTML, images, PDF pages, and direct folder children. | 预览文本/代码、Markdown、HTML、图片、PDF 页面和文件夹直接子项。 |
| **Personalize** | Light/dark/system themes, compact density, shortcut settings, and offline emoji/symbol search. | 明暗/跟随系统主题、紧凑密度、快捷键设置及离线表情/符号搜索。 |
| **Protect** | Local encrypted storage, OS-protected key material, bounded background work, and no plaintext preview files. | 本地加密存储、操作系统保护密钥、有界后台任务，预览不写明文文件。 |

> **Current limitations / 当前限制**
>
> WebDAV sync is not implemented. File records store references, so the original file cannot be restored if it is moved or deleted. Application-source exclusion is currently Windows-only. The Windows `Win+V` system-panel coexistence, broad application compatibility, high-DPI/multi-monitor behavior, and macOS/Linux native operation still need release-level validation.
>
> WebDAV 同步尚未实现。文件记录只保存路径引用，原文件移动或删除后无法恢复。应用来源排除目前仅支持 Windows。Windows `Win+V` 与系统面板共存、更多应用兼容性、高 DPI/多屏行为，以及 macOS/Linux 原生运行仍需发布级验收。

## Screenshots / 截图

The repository includes native Windows captures and a browsable design gallery. The HTML prototype is illustrative; it does not access the real clipboard or replace native validation.

仓库包含 Windows 原生截图和可浏览的设计图库。HTML 原型用于展示交互，不会访问真实剪贴板，也不能替代原生验收。

- [Native UI polish gallery / 原生界面精修图库](docs/native-screenshots/polish/index.html)
- [Window and tray gallery / 窗口与托盘图库](docs/native-screenshots/window-fixes/index.html)
- [Design gallery / 设计图库](design/index.html)
- [Clickable prototype / 可交互原型](.planning/sketches/001-x-clipboard/index.html)

## Getting started / 开始使用

### Requirements / 环境要求

- Rust **1.92 or newer** and the platform's native build toolchain.
- Windows: MSVC toolchain and a graphics driver supported by GPUI.
- Python 3 for downloading the pinned PDFium runtime.
- macOS/Linux: source adapters and CI configuration exist, but native builds are not yet release-validated.

- Rust **1.92 或更高版本**及对应平台的原生构建工具链。
- Windows：MSVC 工具链和 GPUI 支持的图形驱动。
- Python 3：用于获取固定版本的 PDFium 运行库。
- macOS/Linux：已有源码适配和 CI 配置，但尚未完成原生发布验收。

### Build and run / 构建并运行

```powershell
cargo build --locked
python scripts/fetch_pdfium.py --platform win-x64 --output target/debug
cargo run --locked
```

The first launch shows a short setup guide. The app normally opens its management window on startup; closing content windows leaves the tray, shortcut, and capture service running. Use the tray menu to quit.

首次启动会显示简短引导。应用通常在启动时打开管理窗口；关闭内容窗口后，托盘、快捷键和采集服务仍继续运行。请从托盘菜单退出。

To use an isolated development data directory:

如需使用独立的开发数据目录：

```powershell
$env:X_CLIPBOARD_DATA_DIR = "$PWD/test-data/manual-review"
cargo run --locked
```

The download script pins PDFium chromium/7881 and verifies its SHA-256. You may provide a previously downloaded archive with `--archive`.

下载脚本固定使用 PDFium chromium/7881 并校验 SHA-256。已有压缩包可通过 `--archive` 参数提供。

### Windows preview package / Windows 预览包

The repository may include a local preview package at `dist/windows-x64-preview/`. Keep `pdfium.dll` and its `licenses/` directory alongside the executable. Do not put a real user database or key in a distributable archive. Preview packages are development builds, not signed installers.

仓库工作副本可能包含 `dist/windows-x64-preview/` 本地预览包。运行时请将 `pdfium.dll` 和 `licenses/` 与 EXE 放在同一目录。请勿把真实用户数据库或密钥放入分发压缩包。预览包是开发构建，不是签名安装程序。

## Data, privacy, and recovery / 数据、隐私与恢复

- The database is stored locally and encrypted with the project's pinned Turso version. Its encryption feature is documented upstream as experimental; see [third-party resources and dependencies](docs/THIRD-PARTY.md).
- Windows key material is protected with DPAPI and bound to the current Windows account. macOS uses Keychain; Linux uses Secret Service.
- Keep the database and its corresponding key together. If the key is missing or cannot be unlocked, the app will not silently replace it or fall back to plaintext.
- The app does not currently provide data-directory migration or cross-device key recovery. Back up the database and key as a matched pair before moving the data directory.
- Clipboard history can contain sensitive information. Pause capture or configure supported source exclusions when appropriate; avoid sharing database files, keys, or diagnostic data containing personal content.

- 数据库存放在本机，并使用项目锁定版本的 Turso 加密。上游仍将该加密能力标记为实验性，详见[第三方资源与依赖](docs/THIRD-PARTY.md)。
- Windows 密钥材料由 DPAPI 保护并绑定当前 Windows 账户；macOS 使用 Keychain，Linux 使用 Secret Service。
- 数据库必须与对应密钥一起保留。密钥缺失或无法解锁时，应用不会静默替换密钥，也不会降级为明文。
- 当前尚未提供数据目录迁移或跨设备密钥恢复。迁移数据目录前，请成对备份数据库和密钥。
- 剪贴板历史可能包含敏感信息。可按需暂停采集或配置当前平台支持的来源排除；不要分享数据库、密钥或含个人内容的诊断数据。

## Validation status / 验证状态

The latest documented Windows validation includes core storage/service tests, native clipboard probes, PDFium page rendering, abnormal-process-exit recovery checks, native UI capture review, and paste-focus probes. These checks are feature-scoped and do not establish compatibility with every application or production readiness on every OS.

最新记录的 Windows 验证包括核心存储/服务测试、原生剪贴板探针、PDFium 页面渲染、异常进程退出恢复检查、原生界面截图审查和回贴焦点探针。这些验证只覆盖特定功能，不代表兼容所有应用，也不代表已达到所有操作系统的生产发布要求。

Useful records:

相关记录：

- [Native validation log / 原生验证记录](docs/NATIVE-VALIDATION.md)
- [Memory and packaging measurements / 内存与打包测量](docs/MEMORY-AND-PACKAGING.md)
- [Background lifecycle measurements / 后台生命周期测量](docs/BACKGROUND-MEMORY.md)
- [Paste behavior and diagnosis / 回贴行为与诊断](docs/DINGTALK-PASTE.md)

## Roadmap / 路线图

- [ ] Complete Windows `Win+V` coexistence and release-level compatibility checks across common apps, DPI settings, multi-monitor setups, and sleep/resume.
- [ ] Build and run native acceptance on macOS and Linux (X11 and Wayland where applicable).
- [ ] Finish data-directory selection/migration and key-recovery guidance.
- [ ] Audit the full third-party license inventory; prepare signed, platform-appropriate packages.
- [ ] Evaluate sync only after a concrete design for encryption, key ownership, and conflict handling is approved.

- [ ] 完成 Windows `Win+V` 共存及常见应用、DPI、多屏、休眠/恢复场景的发布级验证。
- [ ] 在 macOS 和 Linux（适用时覆盖 X11 与 Wayland）完成原生验收。
- [ ] 完成数据目录选择/迁移与密钥恢复指引。
- [ ] 审核完整第三方许可清单，准备符合平台习惯的签名安装包。
- [ ] 在明确加密、密钥归属和冲突处理方案后，再评估同步功能。

See [ROADMAP.md](ROADMAP.md) for status and scope notes.

## Contributing / 参与贡献

Issues and pull requests are welcome. Before proposing a change, please read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Bug reports should include the OS, app build/commit, reproduction steps, and expected behavior. **Never attach clipboard databases, keys, passwords, or private clipboard contents.**

欢迎提交 Issue 和 Pull Request。开始前请阅读[贡献指南](CONTRIBUTING.md)和[行为准则](CODE_OF_CONDUCT.md)。问题报告请包含操作系统、应用构建版本/提交、复现步骤和预期行为。**请勿附上剪贴板数据库、密钥、密码或私人剪贴板内容。**

## Buy me a coffee / 请我喝杯咖啡

If X Clipboard has been useful to you, you can support its continued development by buying me a cup of coffee. Thank you for your encouragement!

如果这个项目对你有帮助，欢迎请我喝杯咖啡，支持项目继续改进。每一份鼓励都很感谢！

<p align="center">
  <strong>Alipay / 支付宝</strong><br>
  <img src="assets/donate/alipay.png" alt="Alipay donation QR code / 支付宝收款码" width="280">
</p>

<p align="center">
  <strong>WeChat Pay / 微信支付</strong><br>
  <img src="assets/donate/wechat-pay.png" alt="WeChat Pay donation QR code / 微信支付收款码" width="280">
</p>

## License and third-party notices / 许可与第三方声明

Third-party resource and dependency notes are available in [docs/THIRD-PARTY.md](docs/THIRD-PARTY.md). A project-wide license for the application source has not yet been declared in this source tree; clarify the rights holder's intended license before redistribution. Bundled assets and runtime dependencies may have separate terms.

第三方资源与依赖说明见 [docs/THIRD-PARTY.md](docs/THIRD-PARTY.md)。当前源码树尚未声明适用于整个应用源码的项目许可；重新分发前请先确认权利人拟采用的许可。内置资源和运行时依赖可能另有许可条件。

---

<p align="center">Built for the moments when you need that thing you copied earlier.<br>为那些“刚才复制过的内容”而做。</p>
