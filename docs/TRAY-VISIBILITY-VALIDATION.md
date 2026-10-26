# Tray visibility validation / 托盘显示验证

Date / 日期：2026-10-26

- `cargo test --locked --no-default-features --lib`: 38 passed, 2 ignored.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets --features ui-capture -- -D warnings`: passed.
- Settings compatibility: missing `show_tray_icon` defaults to true; false survives JSON serialization and encrypted database reopen.
- Feature-gated `X_CLIPBOARD_UI_REVIEW_TRAY=1` uses only the marked synthetic fixture. It drives the same settings handler as the switch, then the production `poll` and `TrayIcon::set_visible` path, checking `Shell_NotifyIconGetRect` through `TrayIcon::rect()` for true/false/true/false.
- No tray object is created before ready settings are loaded, so a saved hidden preference does not create a startup flash.
- Chinese and English, light and dark native captures are in `native-screenshots/tray-v031` and `native-screenshots/tray-v031-en`; screenshots visually inspected.
- UI fixture captures are isolated from real clipboard history. Global hotkey injection and macOS/Linux native tray behavior were not revalidated.

隐藏托盘图标后，应用继续后台运行；可用已配置的快捷键打开面板，再进入设置恢复显示。旧配置默认显示，隐藏设置保存在本机加密数据库中。本次未变更全局快捷键注册、记录、同步和更新逻辑。

## Windows package / Windows 发布包

- `cargo build --locked --release --bins`: passed.
- Staged packaging copied the release binary without modifying it and verified the ZIP's embedded EXE hash. Packaged application SHA-256: `2884595d702cc2262ef56052577b10de28092e78e8cd6af786efd9512de70627`.
- Final installer payload checksum, actual custom-path install, overwrite upgrade, and asynchronous uninstall passed; the synthetic user-data marker remained and the test uninstall registration was removed.
- This machine already had a user desktop shortcut. The disposable package probe disabled shortcut creation and left that existing shortcut intact; it did not rerun shortcut creation acceptance.
- ZIP SHA-256: `9f133ea6280182191470b0ee09bd098ff21e1322ddea4ba7d77e4eb00e39eae5`.
- Setup SHA-256: `5de083542ac84a17f2572be2c8f9fbcf89976e17db4e91ea592132774295fb1c`.

## Public release / 公开发布

- Published: https://github.com/x-iCNNN/x-clipboard/releases/tag/v0.3.1 (2026-10-26, Asia/Shanghai).
- `/releases/latest` returned `v0.3.1`, `draft=false`, four assets; all four GitHub SHA-256 digests and sizes matched local files before publication.
- Remote tag points to `77b54f3a25fa76b1581e118d05a563216295de5f`; tracked build inputs match the packaged workspace after line-ending normalization.
- `cargo test --locked --no-default-features --lib updater::tests::github_live_check -- --ignored --nocapture`: passed against the public release, including actual portable download, GitHub SHA-256 verification, ZIP extraction, and no update offered for the same version.
