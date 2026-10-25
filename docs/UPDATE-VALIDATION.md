# v0.2.0 安装与更新验证

验证平台：Windows x64。本记录只覆盖本次功能，不扩大已有 macOS / Linux 原生验收范围。

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- `cargo test --locked --no-default-features --lib`：28 项通过；2 项显式忽略（合成数据库性能探针、发布后 GitHub 包下载探针）。覆盖正式版本排序、预发布/降级排除、GitHub 错误与损坏 JSON、大小/哈希校验、更新包路径白名单、安装目录和回滚。
- `scripts/test-updater.ps1`：实际 Windows PowerShell 5.1 安装、备份、占用文件回滚、越界路径拒绝、等待父进程退出、用户文件保留均通过。
- `cargo run --locked --no-default-features --example updater_probe`：实际 GitHub 手动异步检查、自动更新关闭、启动等待后自动异步检查均通过。6 小时重复调度通过源码检查，未实际等待 6 小时。
- `cargo build --locked --release --bins` 和 `scripts/package-windows.ps1 -SkipBuild -NoUpx`：通过。应用、PDFium、许可和说明文件形成同一个根目录 ZIP，安装器嵌入该 ZIP，并记录 SHA-256 和长度。
- `installer_probe` 对最终单文件安装器执行真实安装和覆盖升级：通过。目录含中文和空格，EXE 与载荷一致，保留合成用户文件。
- 实际卸载：EXE、卸载器、桌面/开始菜单快捷方式、当前用户卸载注册项均移除；合成用户文件保留。
- 原生界面：检查了最终安装器布局，系统目录选择器打开/取消；关于页面明暗主题原生截图位于 `native-screenshots/updates/`。

最终发布包：

| 文件 | SHA-256 |
| --- | --- |
| `x-clipboard-v0.2.0-windows-x64.zip` | `804f2c2630d706cac25e32bbcad2310ddbe66bc9cf0c2ed9637dda2229c371c8` |
| `x-clipboard-v0.2.0-windows-x64-setup.exe` | `93e00d79a600036b772dcf98e0500327637c4b9a2b8e11e31637d0ac0586994c` |

已验证更新检查、包完整性和安装机制；跨两个真实发布版本的在线升级仍需下一次正式版本发布时复验。v0.1.0 不含更新器，首次需手动安装 v0.2.0。安装器和应用未做代码签名。

发布后复验：四个远端资产均为 `uploaded`，大小和 GitHub SHA-256 与本地一致。公开 `releases/latest` 返回 `v0.2.0`；标签指向源码提交 `6d9aa71bc570fda69f46728a643c7853c8f6408d`。显式执行 `cargo test --locked --no-default-features --lib updater::tests::github_live_check -- --ignored --nocapture` 通过，覆盖公开版本发现、实际更新包下载、大小与 SHA-256 校验、解压和当前版本无须重复更新。
