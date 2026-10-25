# 原生预览与关键状态视觉审查

## 审查范围与证据

2026-10-25，针对首次成功完整运行的 `docs/native-screenshots/polish/` 原生截图，通过 `view_image` 逐张查看下列 **32 个场景的浅色和深色版本，共64张完整图片**。没有用文件存在、manifest状态或拼图代替逐页查看。

| 分组 | 已查看的场景文件名前缀（各含 `-light.png` 与 `-dark.png`） |
|---|---|
| 预览 | `preview-text`、`preview-rich`、`preview-image`、`preview-code`、`preview-markdown`、`preview-pdf`、`preview-file`、`preview-folder`、`preview-missing`、`preview-preview-error`、`preview-inline`、`preview-loading` |
| 快捷面板及状态 | `quick`、`quick-compact`、`quick-empty`、`quick-no-results`、`quick-paused`、`save-pending`、`storage-error`、`key-unavailable`、`permission`、`protected-capacity` |
| 弹窗与菜单 | `delete`、`clear`、`clear-protected`、`record-menu`、`window-menu`、`quick-menu` |
| 管理页空状态 | `history-empty`、`no-results`、`pinned-empty`、`favorites-empty` |

完整采集共106张，其他42张由主代理及页面代理负责。首轮manifest中的4个预览错误都来自两种预期错误场景（失效文件与损坏图片，各明暗一张）；PDF成功产生两页预览。但PDF内容能渲染不等于适应窗口行为正确，见下表。

下列发现保留首轮图片的观察记录。第三轮复核已重新打开最新PNG，结果见下一节；没有把源代码改动视为视觉通过。

## 第三轮复核结果

2026-10-25，第三轮106张图片重新生成后，使用 `view_image` 再次逐张查看以下15种场景的明暗完整图片，共30张：`preview-image`、`preview-pdf`、`preview-markdown`、`key-unavailable`、`permission`、`storage-error`、`save-pending`、`protected-capacity`、`no-results`、`window-menu`、`quick-menu`、`record-menu`、`preview-code`、`preview-file`、`preview-folder`。

| 编号 | 第三轮状态 | 新PNG实际观察 |
|---|---|---|
| V01 | 视觉复核通过 | 图片完整显示太阳和山丘底部，四周留白且宽高比正确；PDF第一页的白色纸张四边、绿色标题与浅绿正文块均完整可见，明暗一致。只证明适应模式静态效果，未验证1:1及缩放交互。 |
| V02 | 视觉复核通过 | 密钥、权限、存储错误均在420px内换成两行；重试和关闭位于提示框内。权限错误仅保留关闭，避免提供没有实际意义的存储重试。底部快捷键提示仍可见。 |
| V03 | 视觉复核通过 | 保存中显示“保存中…”，超限显示琥珀色“保护内容超限”，密钥不可用显示“记录不可用”，其他错误显示“需要处理”。管理入口未被长状态挤出。 |
| V04 | 视觉复核通过 | Markdown深色表头“项目 / 状态”已变为清晰浅色文字；浅色表头同样清晰。表格边界、正文和背景仍可区分。 |
| V05 | 视觉复核通过 | 三种菜单的图标和文字形成统一左列；窗口菜单与快捷面板菜单显示右侧Win+V键帽。菜单右缘锚点与触发按钮一致，无退出项。 |
| V06 | 部分通过，剩文件夹名称 | 代码、PDF、Markdown、txt分别显示对应类型；代码具有等宽字体和独立底框。`preview-folder-light/dark` 仍显示“文件预览”，应明确为“文件夹预览”；当前文件夹图标正确。 |
| V07 | 视觉复核通过 | 无结果时右侧变为“选择一条记录查看详情”，旧富文本和复制操作消失，明暗一致。只证明该合成状态显示，未替代真实键入搜索的交互测试。 |

本次30张复核没有发现新的内容裁切、错误提示溢出或菜单对齐问题。**V06还有文件夹标题这一项待补齐**；其余6项达到本报告定义的静态视觉验收条件。真实鼠标/键盘、系统回贴和macOS/Linux验收边界不变。

## 首轮发现（保留修复前证据）

| 编号 | 严重性 | 图片证据与具体表现 | 对应源码与修复验收条件 |
|---|---|---|---|
| V01 | P1 | `preview-image-light/dark` 中“适应”模式把960×640图片按约428px宽绘制，却只给约202px可视高度，底部山丘被裁掉。`preview-pdf-light/dark` 同样只显示第一页上部。 | `src/ui/detail.rs::detail_image` 的 `img.size_full()` 位于 flex/scroll 容器；应根据实际内容框宽高确定 contain 比例。验收：两类内容均完整可见，图片比例不变，PDF整页可见；1:1仍可滚动。原型 `.preview-canvas img` 明确 `object-fit:contain`。 |
| V02 | P1 | `key-unavailable-light/dark`、`permission-light/dark` 的底部错误横条不换行，重试/关闭按钮被挤到420px窗口右侧之外；保存失败短文案暂时容得下，不能据此认为长错误已兼容。 | `src/ui.rs` 错误提示容器。正文需要 `flex_1/min_w_0` 和多行，按钮固定不收缩。验收：上述长文案完整换行，重试/关闭都在视口内且不遮住底栏。 |
| V03 | P1 | `save-pending-light/dark`、`protected-capacity-light/dark` 与普通快捷面板几乎完全相同，头部仍为绿色“正在记录”，缺少“保存中”与保护内容超限说明。`key-unavailable` 在 `ready=false` 时也继续显示绿色记录状态。 | `src/ui/history.rs` 调用的 `src/ui.rs::status_badge` 首轮只读取 paused 布尔值。验收：保存中、暂停、未解锁、保存失败和容量超限具有准确可见的状态文字，长状态不会挤掉管理入口。 |
| V04 | P2 | `preview-markdown-dark` 的表格表头“项目 / 状态”接近黑色，在深色底上几乎不可读；同页正文与表格首行是可读的浅色文字。 | `src/ui/detail.rs::detail_rich_text` 使用 `TextView::html`；由应用主题的 `table_head_foreground/table_head` 正确覆盖。验收：表头与正文在深色下均清晰可读，表格分隔保留。 |
| V05 | P2 | `window-menu`、`quick-menu`、`record-menu` 明暗均把图标和文字整体居中，每行图标的起始位置不同；窗口菜单未展示快捷键。菜单右缘与触发按钮对齐正常，无退出项。 | `src/ui.rs::titlebar` 与 `src/ui/history.rs::row_menu`。原型 `.menu button` 是 flex 左对齐，固定图标列，窗口菜单 `kbd` 靠右。验收：图标列、文字列一致左对齐，快捷面板项显示当前快捷键。 |
| V06 | P2 | 代码、Markdown、PDF、普通文本文件、文件夹均以“文件预览”和文件夹图标显示。`preview-code` 内容使用普通正文样式，缺少代码的等宽视觉层次。 | `src/ui/detail.rs::detail_heading/detail_plain_text`。根据扩展名和预览内容选类型名称/图标，代码使用等宽字体与独立背景。原型 `.code-block` 为11px等宽、背景/边框、关键字颜色；不应把普通文件与PDF/代码全部合并成同一个视觉标题。 |
| V07 | P2 | `no-results-light/dark` 左栏明确“没有找到匹配内容”，右侧却继续显示此前富文本及可用复制按钮，容易误认为该内容匹配当前搜索。 | `src/ui.rs::sync/visible`：首轮只在选中ID从全量entries消失时清空选中。验收：搜索/类型/分区过滤排除选中项后，详情改为空状态或明确保留上下文提示，不把旧内容呈现为匹配结果。 |

## 已观察到的正确视觉表现

- 420×540快捷面板的常规与紧凑密度有明显区别，紧凑模式保留操作按钮并显示更多记录；浅深主题没有标题、来源和按钮重叠。
- 文本、富文本、小屏内联预览及加载骨架的正文可读；内联预览具有返回入口，底部粘贴/纯文本/收藏操作可见。
- 失效文件与坏图分别显示“无法访问文件引用”与“图片解码失败”，没有伪装成已恢复文件；长路径在详情错误框内能换行。路径中的 `\\?\` 来自此次fixture规范化后的路径，不将它单独算作用户文件显示缺陷。
- 删除对话框显示选中数量及保护内容提醒；清空默认保留3条保护记录，仅删除8条；勾选包含保护项后明确将删除11条。明暗下危险按钮、警告色和取消操作可辨认。
- 快捷面板和管理页的无历史、无置顶、无收藏、无搜索结果都有对应图标和具体文案，搜索无结果提供清除入口。
- Windows菜单右侧锚点位置正确且没有窗口菜单退出项；系统托盘退出不属于本次菜单截图范围。

这些是静态视觉观察，不证明按钮点击、键盘焦点、删除事务、自动回贴或跨平台行为通过。

## 采集及验收边界

- 本轮所有文件与记录都来自独立加密fixture目录，只有失效文件故意不存在。未向其他应用发送输入。
- 部分文件预览对应的记录在快捷列表首屏之外，首轮未自动滚动到选中项。应追加“打开下方记录的预览后选中项可见”的真实交互检查；原型导航使用 `scrollIntoView({block:'nearest'})`。
- 首轮 `pinned-empty/favorites-empty` 用全量空历史构造，没有覆盖“历史非空但当前保护分区为空”的组合；可补独立场景。
- 错误状态通过合成snapshot配置，仅用于检查文案和布局。密钥不可用场景仍保留fixture历史，是为了检测现有UI面对状态变化的表现，不代表实际解密失败能读取这些记录。
- `capture-manifest.json` 保持 `NOT_PROVEN_BY_CAPTURE_EXISTENCE`；它记录渲染成功，不记录人工/代理视觉验收。本报告的第三轮逐图复核结果独立记录于上表，当前剩余V06文件夹标题待补齐。

## 第四轮复核补充（主代理）

主代理逐张查看最新 `preview-folder-light.png`、`preview-folder-dark.png`，标题已为“文件夹预览”。目录类别由后台预览服务的元数据传递，渲染路径未加入文件系统查询。V06剩余视觉项通过，V01–V07已全部复核修复。

主代理另逐张核对 `shortcut-recording` 与 `shortcut-conflict` 的浅深版本：录入状态、当前按键、保存状态、冲突提示和关闭提示按钮均可见，冲突文案完整。
