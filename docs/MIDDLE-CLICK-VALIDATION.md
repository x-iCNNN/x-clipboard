# Middle-click activation validation / 鼠标中键激活验证

Date / 日期：2026-10-26 (Asia/Shanghai)

## Scope / 范围

Windows-only, opt-in `middle_click_activation` setting, default false for new and older settings. The global mouse hook is installed only while enabled, on the existing clipboard event thread. The hook consumes middle-button down/up and dispatches the existing `ShowQuick` event on release. The original foreground/focus target is captured on press. Other mouse messages continue through the hook chain. Disabling and event-thread shutdown release the hook. No clipboard payloads are inspected by the hook.

仅 Windows 提供此选项；默认关闭，即时应用并持久保存。沿用已有面板及目标窗口回贴逻辑；中键按下时保存原窗口，松开时打开面板。禁用时卸载监听；鼠标滚轮滚动及其他按键不在拦截范围内。

## Checks / 检查

- `cargo test --locked --no-default-features --lib`: 39 passed, 2 opt-in probes ignored.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets --features ui-capture -- -D warnings`: passed.
- Missing-setting compatibility and JSON serialization tested. Enabled middle-click and hidden-tray preferences survive encrypted database reopen.
- Feature-gated `X_CLIPBOARD_UI_REVIEW_MIDDLE_CLICK=1` runs only with a marked, paused synthetic fixture. It applies settings through the same handler as the switch, then sends native `SendInput` middle down/up to a verified foreground window owned by the test process.
- Initial disabled setting: target receives both mouse messages; no quick panel opens.
- Enabled: target receives neither middle mouse message, quick panel opens, and Owner.target equals the original owned window.
- Disabled: target receives both messages again; quick panel stays hidden. Repeated true/false toggles passed.
- A second process reopened the same synthetic encrypted fixture with the setting enabled and successfully activated the hook at startup before the first settings change.
- Native Chinese and English light/dark captures were inspected: `native-screenshots/middle-v032` and `native-screenshots/middle-v032-en`. Result JSON records native input, repeated toggles, consumption/restoration, and target preservation.
- This is native synthetic input verification, not physical mouse hardware, elevated-target, multi-monitor, or macOS/Linux acceptance. Actual content paste injection and keyboard shortcut re-injection were not rerun for this change.

## Release package / 发布包

- Release build passed. Packaging verified the ZIP application hash against the staged Release EXE.
- Final packaged installer passed actual custom-path installation, overwrite upgrade, binary integrity, user-data preservation, and uninstall. No test uninstall registration remained. Shortcut creation was disabled for this isolated probe to preserve the existing user shortcut.
- Application SHA-256: `92f8a3a77e038ad249c503365e9e8ce39ff226959b7d920833c703746b780dbf`.
- ZIP SHA-256: `ce11a54c08ce80d7775a22f3beb03ee75bb7fbd28b55b209101ff2a912ccff9a`.
- Setup SHA-256: `ea3bc952566c5fc8ff7911de7a73e47d30cc0c9141f80ec93355de5b6aa8272a`.

## Public release / 公开发布

- Published https://github.com/x-iCNNN/x-clipboard/releases/tag/v0.3.2 on 2026-10-26 (Asia/Shanghai), with bilingual release notes and four assets.
- GitHub `/releases/latest` returned `v0.3.2`, `draft=false`; all four remote asset sizes and SHA-256 digests matched local files.
- Remote tag: `854358f0c8a693343e0721f40e28cee96ff145ef`. Tracked build inputs matched the packaged workspace after line-ending normalization.
- Explicit public `updater::tests::github_live_check` passed: actual latest portable download, GitHub SHA-256 verification, ZIP extraction, and same-version no-update behavior.
