# Version corner and README gallery / 版本角标与 README 展示图

2026-10-26 (Asia/Shanghai)

- The version badge is an absolute child of the update card with top=0 and right=0, outside its padded content. Only the bottom-left badge corner is rounded; the outer card clips its top-right corner to the existing border radius.
- Native light/dark About captures were visually inspected: `native-screenshots/readme-v036/settings-about-light.png` and `settings-about-dark.png`. The badge touches the card border; 37% background progress remains within the same update row.
- README embeds four native screenshots in a two-column clickable gallery: History, Quick panel, the corrected update card, and encrypted WebDAV. Demo-data/progress captions are explicit. Existing WebDAV fixture screenshot was reused without editing.
- History light, Quick panel dark, About light/dark and the reused WebDAV image were inspected. README local image paths were checked with Python's HTML parser.
- Rustfmt, all-target ui-capture Clippy with warnings denied, and Release binaries passed. This is a layout/docs-only change; updater transport and installation logic are unchanged, so full network/core regression was not repeated.

Final packaging independently verified the setup footer length/magic/SHA-256, exact embedded ZIP identity, all ZIP CRCs, and the packaged application bytes against the Release EXE.

- ZIP SHA-256: `41aa33e1dcef857d3d35599c0c3e0fe4074465361b94c7768c7265ccad116707`.
- Setup SHA-256: `2ccfa98b44039f971714085114e2cf9e56999bab8b0964f94502c52b766af9ed`.
