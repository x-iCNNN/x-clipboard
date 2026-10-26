# Terminal paste validation / 终端回贴验证

2026-10-26 (Asia/Shanghai)

## Report and cause / 反馈与原因

User screenshot displayed `^V`; CMD pasted successfully. User identified DBx's Shell plugin and confirmed manual Ctrl+V fails while Shift+Insert pastes. Production previously always injected Ctrl+V after restoring focus. That does not invoke paste in terminal plugins with a different binding.

The fix adds an explicit, locally saved Windows paste chord choice. It does not detect embedded plugins by executable name and does not try Ctrl+V then retry blindly, which could leave stray control input. The setting applies globally to automatic paste and defaults to Ctrl+V.

## Red/green loop / 复现与回归

- `cargo run --locked --no-default-features --example paste_probe -- --terminal --immediate-focus`: failed before the fix with actual `^V`, reproducing the visible symptom through the real backend paste path.
- The terminal fixture deliberately maps Ctrl+V to a visible control sequence and Shift+Insert to its paste action, matching the reported key semantics. It is not DBx itself.
- `--terminal --immediate-focus --shift-insert --activate-owned`: passed; actual synthetic clipboard text appeared in the owned input control.
- During implementation, the Shift+Insert probe was still red without KEYEVENTF_EXTENDEDKEY: the input stream released Shift before Insert and repressed it afterward. Adding the extended flag to Insert down/up and failure cleanup produced the correct chord and green result.
- `--immediate-focus --activate-owned`: default Ctrl+V native paste passed.
- `--terminal --preserve-focus --shift-insert --activate-owned`: passed with no original-input focus loss.
- `--terminal --shift-insert --interrupt-focus --activate-owned`: cancelled without typing into either target and did not steal foreground back.
- `--terminal --shift-insert --destroy-control --activate-owned`: cancelled without typing.
- The interrupt fixture now accepts either target-window or target-control change errors, since both are valid cancellation points; its no-input and foreground safety assertions remain unchanged.
- `--activate-owned` only clicks a verified window owned by the probe if Windows denies foreground activation; cursor position is restored. The clipboard fixture backs up/restores original supported content without printing it. No user shell commands are submitted.

## Core and UI / 核心与界面

- 40 core tests passed, 2 opt-in probes ignored. Tests cover old-setting defaults, accepted/invalid paste choices, JSON serialization and encrypted reopen.
- Native UI review saved Shift+Insert via the production settings command in a marked synthetic fixture, then captured Chinese/English light/dark settings. Captures visually inspected under `native-screenshots/terminal-v033-zh-CN` and `terminal-v033-en`.
- History writes and text/symbol writes both use the configured chord. Default backend behavior remains Ctrl+V for other platforms, where the new selector is hidden.
- Actual DBx plugin acceptance, elevated targets, and physical keyboard hardware remain outside this native synthetic fixture validation.

## Release package / 发布包

- Rustfmt, Clippy (all targets, ui-capture, deny warnings), and optimized Release build passed.
- Packaged installer passed payload checksum, actual custom-path installation, overwrite upgrade, binary integrity, user-data preservation, and uninstall. Shortcut creation was disabled for the isolated probe to preserve the existing user shortcut. Test registration was removed.
- ZIP SHA-256: `f2f46065de560b8c68a59a3764a4ca535ddabf13a03467eb2f21bd77c1e415af`.
- Setup SHA-256: `92e492029c121a4033bac6b5d2cef5804c05d3618caed172862e4f33fab53da9`.
