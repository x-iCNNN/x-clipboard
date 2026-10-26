# v0.3.7 验证记录

## 快速连续复制

旧实现将剪贴板序号过期作为普通错误显示，再尝试读取当前内容。正常连续复制可能在后台尚未读取旧通知时更新系统剪贴板，导致不必要的错误提示。

使用类型化 `ClipboardSuperseded` 区分过期快照与真实读取失败。捕获线程遇到过期快照时静默重读当前内容，最多尝试三次；持续变化时丢弃不一致快照，继续处理后续通知。成功取得的快照仍通过独立保存队列写入加密库。占用、格式无效、容量限制等真实错误继续提示。

`cargo test --locked --no-default-features --lib rapid_copy`：先运行未修复的读取路径，两个回归用例失败、一个通过；修复后三个通过，分别覆盖过期后捕获最新内容、保留真实错误、有界重试。

边界：测试通过可控的序号过期回调驱动实际捕获读取辅助函数，未进行真实第三方应用连续 Ctrl+C 压力验收。系统已覆盖且程序尚未读取的旧内容无法恢复；不宣称任意速度下每条复制均能保留。

## 磨玻璃与外观

- Windows 外观设置新增开关，默认关闭；加密设置保存和重新打开测试通过。
- 原生窗口使用 GPUI 的 `WindowBackgroundAppearance::Blurred`，表面调整透明度；根组件改为透明背景，避免覆盖系统模糊。关闭时恢复不透明背景。
- 隔离的合成数据验收依次执行开→关→开→关→开，验证设置与窗口帧透明度同步；新建快捷面板继承设置，保持原窗口焦点。
- 本机原生合成状态查询不可用，结果明确记录为 null；不能将帧透明度测试等同于系统合成器状态查询成功。另有桌面目视检查观察到彩色测试背景经窗口模糊透出。未覆盖所有 Windows 构建与系统透明效果配置。
- 检查中文和英文的浅色、深色原生渲染：磨玻璃文案、默认外观和默认主题可见；版本显示 `v0.3.7`，水平边距 12 px，贴合更新卡片右上角。
- 原生验收只使用 `UI-REVIEW-FIXTURE.json` 标记的合成库，不操作正常用户历史。

证据：`docs/native-screenshots/glass-v037-final/frosted-result.json`、`docs/native-screenshots/ui-v037-final/`、`docs/native-screenshots/ui-v037-en/`。

## 检查

- `cargo test --locked --no-default-features --lib`：48 通过，3 项可选测试按设计跳过。
- `cargo clippy --locked --all-targets --features ui-capture -- -D warnings`：通过。
- 安装版和便携版由相同 Release 构建打包；发布前校验 ZIP、安装器载荷、哈希和远端资源。

## 正式发布

本机大文件上传多次断开后，使用 [Windows 原生验证任务](https://github.com/x-iCNNN/x-clipboard/actions/runs/40284730128) 中成功的 Windows 作业产物，在 GitHub 内打包发布。源提交与 v0.3.7 标签均为 `34c8d69e5e7be94cc29026264ea1fd229bdb9e90`。该任务的其他平台结果不作为本次 Windows 发布验收依据。

[发布流程](https://github.com/x-iCNNN/x-clipboard/actions/runs/40287210534) 成功核对产物来源、草稿目标、版本号、安装器载荷及四个远端附件的大小和 SHA-256 后发布为 Latest。正式包由 GitHub 构建产物生成，因此哈希与本机试打包文件不同。公开校验文件已下载并与 API 摘要核对。

- 安装版：35,529,888 字节；`5636082c0cbede55858031e9c4f3161142eb021e725b79a6be19de8449419f92`
- 便携版：17,248,360 字节；`3dc3cfa13b9215cd693b11129b99509b2f5ce8738aaa436ccf62ca10119ea04e`
- [正式发布 v0.3.7](https://github.com/x-iCNNN/x-clipboard/releases/tag/v0.3.7)，中英双语说明仅包含更新内容。
