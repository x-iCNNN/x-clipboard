# 主题、Kbd 与应用图标

日期：2026-10-25。实现基于锁定的 GPUI Kit 0.8.0。

## 使用

- 外观设置中的“内置主题”，或窗口右上角主题按钮，均可选择主题。菜单支持滚动；选择后立即生效并保存。
- 默认配色为火绒风格暖黄，以 #F5BC21 为按钮/品牌基准；浅色文字强调使用较深金色保持可读性，深色使用亮金色。选择第三方主题时完整应用其配色，不以默认黄色覆盖原始主题。
- 内置21个官方JSON主题集合、36个可选明暗变体，资源编译进程序，运行时无需网络。来源与署名见 assets/themes/README.md 及各原始JSON。
- 快捷键使用 GPUI Kit 的 Kbd，覆盖面板底栏、菜单、引导、设置及管理底栏；根据平台格式化。保留原有快捷键行为。
- 标识为原创“守护剪贴板”：圆形黄色背景与白色剪贴板，参考火绒的简洁黄色视觉。SVG、多尺寸PNG、ICO与ICNS见 assets/branding/README.md。
- Windows EXE通过资源ID1内嵌7尺寸图标，与GPUI窗口图标加载路径匹配；应用内品牌和托盘使用同一母版。macOS/Linux图标文件已导出，但目标系统打包/实机验证另计。

## 验证

- 13项核心测试通过：新增21套/36变体目录一致性、所有主题设置合法性、加密数据库实际保存读取及重开保持。
- Kbd别名测试通过：Win+V、Super+Shift+V、⌘⇧V、Shift+Enter、Esc保持修饰键含义。
- 全目标Clippy（含ui-capture）及Windows Debug构建通过。
- 实际读取最终Windows图标组资源，ID1包含7个尺寸。
- 原生截图142张：默认53场景×明暗，共106张；36个内置变体各有完整快捷面板图。捕获时逐个核对Theme::theme_name与选择项一致，避免仅记录名称但未实际应用。
- 主代理查看默认快捷面板、外观、快捷键设置，以及Ayu Light、Catppuccin Mocha、Tokyo Night、Solarized Light等样图：图标、Kbd和配色均生效，360px面板没有底部绿色装饰。截图文件存在不等于所有交互及对比度矩阵都已验收。

图库：native-screenshots/branding/index.html。原有polish图库的默认106张也已更新。

## 参考

- [GPUI Kit Theme](https://gpui-kit.com/zh-CN/component/theme/)
- [GPUI Kit Kbd](https://gpui-kit.com/zh-CN/component/kbd/)
- [GPUI Kit v0.8.0 themes](https://github.com/longbridge/gpui-kit/tree/v0.8.0/themes)
- [火绒官网视觉参考](https://www.huorong.cn/)

后续修正：应用图标外轮廓改为圆形黄色背景；首次快捷面板尺寸与开关状态对比修复见 WINDOW-FIXES.md。
