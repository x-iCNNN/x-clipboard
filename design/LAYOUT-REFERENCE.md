# 布局参考与第三轮修订

用户追加要求：参考火绒布局，以及 `D:\code\r\x-bls-workspace` 的 titlebar 和布局。

## 本次核对的来源

- 火绒官方 6.0 界面图：[官方介绍](https://www.huorong.cn/document/info/productions/1655)、[界面截图](https://cdn-www.huorong.cn/d/file/2024-06-25/48a33eca7f6edc958cfb855ffae6a63e.png)。用于确认窄竖向图标栏、内容区独立标题栏、右上主题及窗口菜单的结构。
- BLS 当前源码 `src/ui.rs`：`NAV_WIDTH = 64`、`STATUS_HEIGHT = 52`；`title()` 为 46px；主题和菜单为 30×30；窗口控制为 32×30，主内容区独立承载 titlebar。
- BLS 当前源码 `src/settings_ui.rs` 和 `src/AGENTS.md`：180px 设置侧栏、标题和说明位于分组面板外、圆角面板、相邻项单层分隔线。
- BLS 原生截图 `target/ui-smoke/profile-scrollbar-main-light.png`、`profile-scrollbar-quick-light.png`、`settings-cache.png`。未将该项目早期 `design/renders/accounts.png` 的宽导航当作最新实现依据。

## 应用到 X Clipboard

| 参考结构 | X Clipboard 对应设计 |
|---|---|
| 顶部品牌与导航同宽 | 第三轮改为 200px 宽导航，应用图标和名称并排；历史、置顶、收藏、表情、符号显示完整名称，记录分组显示数量 |
| 内容区 titlebar | 当前区域名称、明暗主题、窗口菜单、原生习惯的窗口控制 |
| 主内容左右布局 | 记录列表 + 详情预览，内容类型放在上方标签栏 |
| 底部状态栏 | 记录状态及暂停、已保存条数、本地加密与唤起快捷键 |
| 独立设置侧栏 | 180px 设置导航，右侧按通用、外观、快捷键、存储、隐私等分组 |
| 紧凑辅助窗口 | 快捷面板保持 440×600，采用相同 titlebar 控件排列 |

保留已确定的青绿色、浅色/深色、macOS 左侧窗口按钮，以及原有剪贴板功能范围。不复制品牌标识或安全软件的业务功能。

本轮没有修改 BLS Workspace 仓库。

## 第三轮浏览器批注

- 主窗口采用完整宽导航，覆盖此前的 64px 图标栏选择。
- 菜单使用按钮容器作为定位锚点，取消固定 92px 右侧偏移，macOS、Windows、Linux 均与按钮右边缘对齐。
- 取消标题栏菜单中的“退出 X Clipboard”；托盘菜单仍按原产品约定提供退出入口。

## 第四轮首次使用引导结构

以用户提供的欢迎页截图为准：只将 titlebar 与正文一起纳入右侧内容区，左栏贯穿整个窗口高度；右侧 titlebar 与正文之间不设分割线。保持 740×550 窗口、220px 左栏、原有四步内容、文案和按钮逻辑。上轮误加的独立使用指南页面已撤回，恢复原来的使用帮助弹窗。
