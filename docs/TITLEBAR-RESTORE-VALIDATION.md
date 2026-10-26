# v0.3.8 标题栏最大化与还原

## 与 BLS Workspace 的实现对比

参考 `D:/code/r/x-bls-workspace/src/ui.rs`：最大化按钮是带 `WindowControlArea::Max` 的自定义元素，没有手动调用 `zoom_window()` 的点击回调。

X Clipboard 旧实现通过普通 Button 的点击回调调用 `window.zoom_window()`。当前锁定的 GPUI Windows 0.3.9 实现中，`zoom()` 对可见窗口只调用 `ShowWindowAsync(SW_MAXIMIZE)`，所以第二次点击仍然最大化。

修复采用与 BLS 一致的 Windows 路径：

1. 自定义按钮声明 `WindowControlArea::Max`。
2. `WM_NCHITTEST` 返回 `HTMAXBUTTON`。
3. GPUI 在 `WM_NCLBUTTONDOWN` 记录按钮，在 `WM_NCLBUTTONUP` 根据原生窗口状态执行 `SW_MAXIMIZE` 或 `SW_NORMAL`。
4. 不额外注册普通点击回调，避免重复处理。保留无障碍按钮标签、尺寸、图标和主题悬停高亮。
5. 非客户区鼠标移动继续进入系统默认处理，供 Windows 11 显示原生 Snap Layouts；没有自绘替代分屏菜单。非 Windows 的原有点击路径保持原样。

## 复现与验证

隔离合成库 `test-data/glass-v037-fixture`，原生验收开关 `X_CLIPBOARD_UI_REVIEW_TITLEBAR=1`，通过 `--ui-review` 执行。

- 修复前：同一验收第一次点击最大化成功，第二次点击报 `titlebar step 1: expected maximized=false; hit=1`，复现无法还原。
- 修复后：两轮最大化→还原通过；每次命中均为 `HTMAXBUTTON`；Win32 `IsZoomed` 与 GPUI 状态一致；还原后的窗口矩形与初始矩形完全相同。
- Computer Use 在隔离的「X Clipboard · 标题栏验收」窗口实际点击自定义按钮，观察到窗口扩大为最大化，再点击还原回原尺寸。
- 用户在同一原生验收窗口实测悬停，明确确认“会弹出系统分屏布局”。自动化工具无法可靠保持悬停，因此该项记录为用户实测确认。
- `cargo test --locked --no-default-features --lib`：48 通过，3 项可选测试按设计跳过。
- `cargo clippy --locked --all-targets --features ui-capture -- -D warnings`：通过。

自动验收结果与还原窗口帧位于 `docs/native-screenshots/titlebar-v038/`。该帧不用于声称捕获了系统分屏弹层。

## 发布核对

- [Windows 原生验证任务](https://github.com/x-iCNNN/x-clipboard/actions/runs/40368416707) 的 Windows 测试、正式构建和产物上传均成功。本次仅发布 Windows x64，其他平台任务不作为本次验收依据。
- [发布任务](https://github.com/x-iCNNN/x-clipboard/actions/runs/40370082027) 成功验证产物来源、安装器内嵌 ZIP、四个远端附件的大小及 SHA-256，随后发布为 Latest。
- [v0.3.8](https://github.com/x-iCNNN/x-clipboard/releases/tag/v0.3.8) 的标签和草稿目标均为 `420be8ee2c4b2941da53e1306ab17c57d663eae5`，中英双语说明仅列本次修复。
- 安装版：35,530,577 字节；`19d2a69be0eb3e28fdaea61bcd6e2a391efcd07a3f6cfe438f6673eedc4d5ba3`。
- 便携版：17,249,049 字节；`f3219fd29f5cf57be4e36d9b2ed442087f58aacd958da85c977527e08c1c9463`。
- 已下载公开 SHA-256 校验文件，核对其值与 GitHub API 资源摘要一致。
