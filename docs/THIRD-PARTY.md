# 资源与依赖来源

Rust 依赖完整版本锁定于 `Cargo.lock`；本文件记录手工分发的资源。正式发布前仍需生成全部传递依赖许可清单。

| 资源 | 来源 | 本项目处理 |
|---|---|---|
| GPUI Kit 0.8.0 | https://gpui-kit.com/docs/installation/ | 原生 UI 框架，精确锁定 |
| Turso 0.8.3 | https://turso.tech/blog/encrypting-sqlite-databases-with-turso | 本地 AES-256-GCM；官方仍标记实验性 |
| PDFium chromium/7881 | https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/7881 | 匹配 pdfium-render 0.9.4 ABI；SHA-256 固定在下载脚本；保留包内许可 |
| Unicode 17 emoji-test | https://unicode.org/Public/17.0.0/emoji/emoji-test.txt | 完整限定表情序列，无在线 GIF |
| CLDR 中文 annotations | https://github.com/unicode-org/cldr-json/tree/main/cldr-json/cldr-annotations-full/annotations/zh | 2026-10-24 下载生成中文名称/关键字；快照 assets/emoji-zh.json |

表情目录含 3944 项，生成脚本为 `scripts/prepare_symbols.py`。Unicode 许可在 `assets/LICENSE-UNICODE.txt`。运行时只读取内嵌目录，不访问上述网络地址。

托盘菜单图标复用已内嵌的 GPUI Kit Assets 0.8.0 Lucide 图标（clipboard、layout-grid、pause、settings、power），通过现有GPUI SVG渲染器生成16px原生菜单位图，未增加图标依赖或外部资源。

主题资源：GPUI Kit v0.8.0/themes，21个原始JSON集合、36个变体，保留作者和来源URL，LICENSE-APACHE随assets/themes分发。图标为本项目原创；winresource 0.1.34仅用于Windows构建时嵌入应用图标。

Windows托盘菜单现由GPUI渲染并跟随应用主题，复用相同Lucide图标（含play）；其他平台保留原生图标菜单。
