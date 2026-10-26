# Quick panel dismissal validation / 快捷面板关闭验证

2026-10-26 (Asia/Shanghai)

## Change / 变更

The tray menu had activation-loss dismissal, while the Windows quick panel deliberately used WS_EX_NOACTIVATE to preserve the original editor focus. Its activation state therefore could not be used as an ordinary popup blur signal.

A low-level mouse hook is now attached only while the quick panel is shown. Outside button-down events set a dismissal flag without consuming the click. Panel descendants and owned popup windows remain inside its interaction scope. Foreground window changes are also checked by the existing UI poll. The original foreground target and quick panel/owned popup windows are allowed. A zero/unknown target does not cause immediate dismissal on startup. Hiding or destroying the panel releases the temporary hook. The existing middle-click activator is excluded from outside-click dismissal when enabled, so opening with middle click does not immediately close it. Non-Windows quick panels use activation-loss dismissal like the tray menu; native acceptance there remains unverified.

## Red / 复现

The feature-gated `X_CLIPBOARD_UI_REVIEW_DISMISS=1` test uses a marked synthetic fixture and creates a separate owned Win32 foreground target. It opens the actual GPUI quick panel, clicks inside its list, then clicks the original target outside the panel. Before the fix it failed with `outside click in original window did not dismiss quick panel`.

Clicks are sent only after WindowFromPoint confirms the intended owned root window; obscured targets abort. Cursor position is restored and the synthetic foreground target is destroyed on exit. No user application is clicked or exited by this harness.

## Green / 回归

The native dismissal test passed three consecutive runs, verifying:

- Inside body/list clicks keep the panel visible.
- The original input window remains foreground while interacting with the nonactivating panel.
- Clicking outside in that same original window hides the panel.
- Changing foreground to another owned application window hides the panel.
- The panel can reopen after dismissal.
- An explicitly activated quick panel also dismisses when focus returns to the original target without a mouse click. Native activation loss to an owned popup remains within the panel scope.

Additional regression:

- Native search input: focus preserved, Latin and Unicode committed text, backspace, and clear/edit passed. Physical IME composition remains outside this test.
- Initial panel size and preview expansion/collapse probe passed.
- Enabled middle-click activation and original paste-target preservation passed against the new dismissal hook.
- Full middle enable/disable passthrough probe was not counted as passed on this desktop: when the test instance disabled its hook, the owned target received zero middle events while another installed X Clipboard process remained running. That process was left untouched. An enabled-only test confirmed this build's middle activation/dismissal interaction; its result JSON explicitly marks toggle/disabled passthrough checks false (not validated).

Private test results: `test-data/dismiss-final/dismiss-result.json`, `dismiss-repeat1`, `dismiss-repeat2`, `dismiss-SEARCH/search-input-result.json`, `dismiss-INITIAL_SIZE/initial-size-result.json`, and `dismiss-middle-enabled/middle-click-result.json`.

Release notes contain only user-facing changes, in Chinese and English. Bugs use 🕷️, features ✨, and fixes 🔧; unused categories are omitted. CI requires the version-specific notes file instead of auto-generated notes.


## Final build and packaging / 最终构建与打包

- Rustfmt and all-target ui-capture Clippy with warnings denied passed. Final core tests: 40 passed, 2 opt-in probes ignored. Optimized Release build passed.
- Final packaged payload checksum, custom-path installation, overwrite upgrade, binary integrity, preserved synthetic user data, and uninstall passed. Shortcut creation was disabled to preserve existing user shortcuts.
- The final native dismissal test includes explicit activation followed by return to the original target: 	est-data/dismiss-active-confirmed/dismiss-result.json passed all six checks.
- ZIP SHA-256: `e8df4d2ebad9278ae0efdbded0e9eb27cbccd0aa6a3c6dddfaf016d8a708a5e4`.
- Setup SHA-256: `d55e4543ec1b48dfa74da4d317c54391eb170cb63173bdf8b941b8eaa4817e72`.
