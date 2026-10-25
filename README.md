# X Clipboard

Rust + GPUI Kit 0.8.0 原生剪贴板，使用 Turso 0.8.3 本地加密数据库。

**2026-10-24 原型已确认，已提供 Windows x64 原生测试版。跨平台发布验收尚未完成。**

## 运行原生测试版

本地测试包：`dist/windows-x64-preview/x-clipboard.exe`。保持同目录的 `pdfium.dll` 和 `licenses/` 完整；不要把数据库和密钥放进分发包。

首次启动显示四步引导。默认 Win+V 唤起，可以修改快捷键。关闭窗口后继续驻留托盘，退出入口仅在托盘菜单。管理窗口的“复制”只写入剪贴板；快捷面板粘贴时恢复原目标窗口。

```powershell
cargo build --locked
python scripts/fetch_pdfium.py --platform win-x64 --output target/debug
cargo run --locked
```

需要 Rust 1.92+、Windows MSVC 构建环境和 GPUI 支持的图形驱动。下载脚本固定 PDFium chromium/7881 并校验 SHA-256；已有压缩包可传入 `--archive`。

默认数据目录由 `directories::ProjectDirs` 选择，设置页面显示实际位置。开发验收可用独立目录：

```powershell
$env:X_CLIPBOARD_DATA_DIR = "$PWD/test-data/manual-review"
cargo run --locked
```

数据库与 Windows `clipboard-key.dat` 必须一起保留；密钥还绑定当前 Windows 账户。macOS 使用 Keychain，Linux 使用 Secret Service。缺失或不可解锁的密钥不会被自动替换，不会降级为明文。当前尚未提供目录迁移或跨设备密钥恢复界面。

## 原生功能与验证

- 多格式采集、精确去重、重复复制排序、全文搜索、置顶、收藏、批量删除和保护记录清理。
- 原生快捷面板、完整左导航管理窗口、设置、首次引导、托盘菜单；明暗主题和跟随系统。
- 文本、HTML/RTF 格式保存与写回，图片、文件引用；Windows 历史文件按复制语义回放。
- 文本/代码、Markdown、HTML、图片、PDF 翻页、文件夹直接子项预览；图片缩放。RTF 原始格式保留，预览使用同次复制的纯文本。
- 中文离线表情/符号搜索和最近使用，快捷键录入、暂停、应用排除（目前仅 Windows）、开机启动和容量设置。
- 后台加密读写、有界队列、占用重试和失败反馈；图片载荷在加密库内，预览不写明文缓存。

WebDAV 仅展示“计划中”。文件仅保存引用，原文件失效后不能恢复。

本机已通过 12 项核心测试、Windows 系统剪贴板探针、PDFium 连续翻页、进程异常退出恢复及记事本真实回贴。详见 [原生验收记录](docs/NATIVE-VALIDATION.md) 和 [资源来源](docs/THIRD-PARTY.md)。

Win+V 与系统面板互斥、Office/浏览器/资源管理器、高 DPI、多屏、休眠和压力场景尚需实机验收。macOS、Linux 有适配源码和 CI 配置，尚未在目标系统编译运行。非 Windows 应用来源排除尚未实现，窗口关闭暂采用最小化。数据目录选择迁移、完整许可清单、安装包和签名仍待完成。

## 本轮界面精修与回贴修复

2026-10-25：快捷面板收紧为 360×540，提供常规/紧凑密度；管理窗口、设置、引导、表情和预览统一间距、字号、图标与明暗主题。已生成并逐页审查 53 个页面/状态的双主题完整原生图。

- [Windows 原生界面图库](docs/native-screenshots/polish/index.html)
- [页面验收记录](docs/UI-POLISH-CHECKLIST.md)
- [钉钉回贴诊断与回归](docs/DINGTALK-PASTE.md)

回贴现在恢复唤起前的输入控件焦点，再发送粘贴；目标切换或控件销毁时取消，内容保留在剪贴板。八组真实 Win32 控件探针通过；用户已确认当前钉钉输入框无需再次点击即可自动填入；临时诊断已移除。

## 查看设计

- [完整渲染图库](design/index.html)：浅色、深色及系统适配图，可放大查看。
- [可点击原型](.planning/sketches/001-x-clipboard/index.html)：搜索、预览、收藏、置顶、清理、快捷键录入等交互演示。
- [交付说明](design/README.md)：页面清单、运行和验收说明。
- [已确定的产品与实现约定](design/DECISIONS.md)：下一阶段实现边界。

使用浏览器直接打开 HTML 即可，无网络依赖。所有内容均为虚构示例；原型不访问真实剪贴板、不执行系统快捷键接管、不保存真实用户内容。

## 当前目录

```text
.planning/sketches/001-x-clipboard/  可交互 HTML 原型及示例插图
.planning/sketches/themes/          共用颜色、字体与主题变量
design/index.html                  设计评审图库
design/renders/                    独立完整 PNG 渲染图
design/tools/                      渲染与交互验收脚本
```

用户确认见 [APPROVAL.md](design/APPROVAL.md)。HTML 仅为设计原型，正式窗口由 GPUI 原生渲染。

## 主题与应用标识

默认使用火绒风格暖黄；外观设置或标题栏主题按钮可选21套、36款内置明暗配色，离线使用并保存。快捷键统一使用GPUI Kit Kbd。

- [主题与图标原生预览](docs/native-screenshots/branding/index.html)
- [实现与验证记录](docs/THEMES-AND-BRANDING.md)
- [原创应用图标源文件与尺寸](assets/branding/README.md)

- [窗口、托盘与响应修复](docs/WINDOW-FIXES.md) · [最新原生截图](docs/native-screenshots/window-fixes/index.html)

- [内存与UPX打包记录](docs/MEMORY-AND-PACKAGING.md) · [最新优化原生预览](docs/native-screenshots/final-optimizations/index.html)
- [后台按需卸载与内存实测](docs/BACKGROUND-MEMORY.md)
- [加密数据库缩略图](docs/PERSISTED-THUMBNAILS.md)
