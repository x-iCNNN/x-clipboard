# Roadmap / 路线图

This roadmap records the project's current direction, not a promise of dates. Status reflects repository evidence and should be updated when new validation is completed.

本路线图记录当前方向，不承诺交付日期。状态依据仓库中的验证证据；完成新验证后应及时更新。

## Available as a Windows preview / Windows 预览版已有

- [x] Native clipboard history, search, deduplication, pin/favorite, and cleanup.
- [x] Tray-resident workflow, keyboard shortcut panel, management/settings windows.
- [x] Local encrypted storage and OS-protected key material.
- [x] Text, rich-format, image, PDF, and file-reference previews within documented limits.
- [x] Windows native probes, UI review captures, and memory/package measurements.

- [x] 原生剪贴板历史、搜索、去重、置顶/收藏和清理。
- [x] 托盘驻留、快捷键面板、管理窗口和设置窗口。
- [x] 本地加密存储和操作系统保护的密钥材料。
- [x] 在已记录边界内支持文本、富文本、图片、PDF 和文件引用预览。
- [x] Windows 原生探针、界面审查截图及内存/打包测量。

“Available” here means the feature exists in the Windows development preview. It does not imply a signed installer or universal application compatibility.

这里的“已有”表示功能存在于 Windows 开发预览版，不代表已有签名安装包或兼容所有应用。

## Next validation / 下一步验证

- [ ] Verify system `Win+V` coexistence, shortcut release on exit, and common application behavior.
- [ ] Exercise high-DPI, multi-monitor, sleep/resume, long-running capture, and elevated-window cases.
- [ ] Complete native build and runtime acceptance on macOS and Linux; document X11/Wayland capability differences.
- [ ] Validate data-directory selection/migration and document safe backup/key recovery behavior.
- [ ] Finish dependency license inventory, packaging, signing, and release/update process.

- [ ] 验证系统 `Win+V` 共存、退出时释放快捷键，以及常见应用行为。
- [ ] 验证高 DPI、多屏、休眠恢复、长时间采集和管理员窗口场景。
- [ ] 完成 macOS 和 Linux 原生构建/运行验收，并记录 X11/Wayland 能力差异。
- [ ] 验证数据目录选择/迁移，并说明安全备份和密钥恢复行为。
- [ ] 完成依赖许可清单、打包、签名和发布/更新流程。

## Later exploration / 后续探索

- [ ] Design synchronization only after encryption, key ownership, conflict resolution, and server trust are specified.
- [ ] Reassess file-reference behavior and optional richer format previews based on user feedback.

- [ ] 明确加密、密钥归属、冲突解决和服务器信任模型后，再设计同步。
- [ ] 根据用户反馈重新评估文件引用体验及更多格式预览。

## Explicitly out of current scope / 当前明确不在范围内

Online GIF search, OCR, AI processing, Office/media embedded preview, and an operational WebDAV sync service are not implemented. Do not describe planned items as shipped functionality.

在线 GIF 搜索、OCR、AI 处理、Office/音视频内置预览及可用的 WebDAV 同步服务均未实现。请勿将规划项写成已交付功能。
