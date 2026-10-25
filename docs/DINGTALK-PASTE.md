# 钉钉自动回贴诊断

日期：2026-10-25。

## 用户报告与验证边界

- 同一条文本通过快捷面板可回贴记事本，在钉钉没有插入。
- 用户补充：钉钉自动回贴失败后，手动 Ctrl+V 可以粘贴。这说明该场景的剪贴板写回可用，自动焦点/输入链路需要单独验证。
- 首轮修复后的真实钉钉复测仍失败：点击输入框、唤起面板并双击记录后没有写入；立即再点击输入框会填入，不点击则错误恢复流程重新显示面板。单纯等待原输入框自行恢复焦点不足以覆盖该场景。
- 当前桌面自动化工具对钉钉返回窗口归属校验错误：错误中的原应用与当前应用标识相同。重新枚举后仍失败。该工具故障不是钉钉不能粘贴的根因证据。
- 未读取或记录钉钉聊天正文，没有发送消息。

## 已复现的通用缺陷

`examples/paste_probe.rs` 创建两个仅属于测试进程的临时窗口：一个含原生编辑框的目标窗口、一个模拟快捷面板。通过真实 `ClipboardBackend::paste` 回贴，并断言编辑框最终确实包含固定测试文本。

修复前结果：

| 模式 | 行为 | 结果 |
|---|---|---|
| `--immediate-focus` | 激活目标后立即恢复编辑框焦点 | PASS，测试文本进入编辑框 |
| 默认 | 激活目标后 160ms 恢复编辑框焦点 | FAIL：paste 返回成功，但编辑框仍为空 |

旧实现只检查顶层前台窗口，固定等 40ms 后注入 Ctrl+V。顶层窗口已在前台，不代表原输入控件已经重新获焦。因此该路径会把按键发给未就绪的窗口，同时向用户显示成功。

## 首轮修复方式与局限

- 唤起快捷面板时记录原顶层窗口、原输入控件及其线程/进程身份；不读取编辑框文字。
- 回贴前确认原窗口仍有效，先等待异步前台激活完成，再等待原输入控件重新获焦，并保持稳定 40ms。焦点等待上限为 1.2 秒，而不是增加固定延时。
- 用户切换到其他窗口、目标窗口被替换、输入控件未恢复时停止自动输入，明确提示手动粘贴；已写入剪贴板的内容保留。
- 原输入控件已销毁时停止自动输入，不猜测同窗口的其他输入框。
- SendInput 完全/部分失败时明确反馈；部分注入只尝试释放本次已按下而未释放的合成键，不重试粘贴。
- 不按钉钉进程名分支、不模拟鼠标点击、不提权或修改系统权限。

## 复测命令

```powershell
cargo run --locked --no-default-features --example paste_probe -- --immediate-focus
cargo run --locked --no-default-features --example paste_probe
cargo run --locked --no-default-features --example paste_probe -- --focus-delay=400
cargo run --locked --no-default-features --example paste_probe -- --interrupt-focus
cargo run --locked --no-default-features --example paste_probe -- --destroy-control
```

首轮修复后本机已运行的结果：

| 验证 | 结果 |
|---|---|
| 立即恢复输入焦点 | PASS，实际编辑框收到测试文本 |
| 160ms 延迟恢复 | PASS，实际编辑框收到测试文本 |
| 400ms 延迟恢复 | PASS，实际编辑框收到测试文本 |
| `--interrupt-focus` | PASS，切到第三个自建窗口后明确取消，两个编辑框均无输入 |
| `--destroy-control` | PASS，原输入框销毁后明确取消，未插入到其他控件 |
| 核心单元测试 | 12 项全部通过，新增部分键盘注入的按键释放回归 |

临时诊断日志与 `X_CLIPBOARD_PASTE_TRACE` 开关已删除。上述原生控件通过结果没有覆盖“应用完全不自行恢复控件焦点”，用户后续钉钉复测仍失败，不能将其计为钉钉验收通过。

## 第二轮：精确恢复原输入控件

新增 `--manual-focus`：目标窗口重新激活后仅让顶层窗口获得焦点，不安排任何自动恢复编辑框的计时器。它调用同一个真实 `ClipboardBackend::paste`，仍断言原编辑框收到固定测试文本。

```powershell
cargo run --locked --no-default-features --example paste_probe -- --manual-focus
```

改后端前已实际运行，退出码 1：`原输入框尚未恢复焦点；内容已复制，请点击输入框后手动粘贴`。该红测试说明原生控件不自行恢复的缺口存在；它不证明钉钉内部编辑器与 Win32 Edit 控件实现相同。

本轮可证伪假设：

1. 顶层窗口激活后未恢复原 HWND 输入控件：精确恢复同一有效控件应使 `--manual-focus` 由红转绿。
2. 原控件已销毁或被替换：恢复前的身份检查应拒绝，销毁负向测试应继续取消。
3. 用户在恢复期间切换窗口：第三窗口中断测试应取消输入，并保留第三窗口前台状态。
4. 钉钉使用 HWND 内部的虚拟编辑焦点：即使原生测试通过，真实钉钉仍可能失败；必须以新版本同输入框复测判断。

实现只尝试一次恢复捕获到的原控件：

- 记录原顶层窗口及原控件各自的线程/进程身份；激活前、恢复前后校验句柄有效性、身份及父子关系。
- 必须先确认原顶层窗口已在前台，且原控件可见、启用。临时连接工作线程与原控件线程的输入队列，调用 `SetFocus` 恢复精确 HWND，并由 RAII 在返回或出错时立即解除连接。
- 连接期间不等待、不发送粘贴键；解除连接后重新确认原控件稳定获焦 40ms，再检查前台及物理修饰键状态并发送 Ctrl+V。
- 原控件消失、线程/进程不符、目标转至第三窗口、连接失败均取消。没有原控件身份时不猜测其他输入框，不模拟点击。
- `--interrupt-focus` 改为在编辑框真实收到 `WM_SETFOCUS` 时同步切至第三个自建窗口，不再使用 80ms 计时器与恢复速度竞争；断言两个编辑框无输入且第三窗口仍在前台。

API 依据：微软 [SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus) 要求目标窗口连接到调用线程的消息队列，并明确说明跨线程可配合 AttachThreadInput；[AttachThreadInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-attachthreadinput) 要求两线程存在消息队列、不能连接自身或跨桌面，并会重置键盘状态；[GetGUIThreadInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getguithreadinfo) 可读取其他进程 GUI 线程的焦点信息，但激活切换期间的句柄可能暂时无效。

第二轮本机已实际运行的回归结果：

| 验证 | 结果 |
|---|---|
| `--manual-focus`，原控件不自行恢复 | PASS，由修复前超时失败转为原编辑框实际收到文本 |
| `--interrupt-focus`，在 WM_SETFOCUS 同步切第三窗 | PASS，取消输入，两个编辑框为空，第三窗口仍在前台 |
| `--destroy-control` | PASS，原编辑框销毁后取消，未输入到其他控件 |
| `--immediate-focus` | PASS，原编辑框实际收到文本 |
| 默认 160ms 恢复 | PASS，原编辑框实际收到文本 |
| `--focus-delay=400` | PASS，原编辑框实际收到文本 |

这些结果不覆盖前述首轮真实钉钉失败证据。第二轮真实钉钉仍需用户复核，尚不能宣布通过。

静态审查边界：

- 回贴开始时复制一次已记录目标身份，避免等待期间重新读取全局目标导致目标混用；根窗口与控件身份在恢复前后复核。句柄、线程与进程匹配仍不能完全排除极短时间内同线程重用相同 HWND。
- 输入队列 guard 只在 AttachThreadInput 成功时创建，同线程不连接；正常返回、提前报错及 Rust panic 展开均会解除连接。解除调用由系统执行，线程终止等极端情况未单独注入验证。
- 发送键之前等待用户松开修饰键，解除输入队列连接后再次检查异步键状态，不回写一份可能陈旧的全局键状态。部分 SendInput 成功只释放本次遗留合成键，不重试文本。
- UIPI/应用权限差异仍由系统限制，SendInput 失败会报错并保留剪贴板内容；没有提权或绕过权限。6 个本机模式使用同权限自建窗口，不覆盖提权应用。
- SetFocus 会同步触发目标焦点消息，目标线程在调用期间挂起的极端情形仍可能阻塞该调用；本轮未挂起第三方应用进行验证。
- HWND 获焦不能代表 HWND 内部的虚拟编辑器已经恢复光标；真实钉钉验收仍是独立门槛。

若后台启动没有前台权限，探针会提示在 30 秒内激活其自身 `X Clipboard · own target probe` 窗口。它不会绕过 Windows 前台权限限制。测试会暂时写入合成剪贴板内容，并尝试恢复运行前支持的格式；不要与日常剪贴板历史验收混用数据目录。

此回归探针覆盖原生控件延迟恢复、完全不自行恢复，以及第三窗口中断和原控件销毁，不能单独证明钉钉所有版本/权限/输入框都已通过。钉钉仍需在同一输入框对比：手动 Ctrl+V、普通回贴、纯文本回贴，并验证没有误发送消息；未完成前不能宣布钉钉实机验收通过。

## 第三轮：取得真实应用的阶段证据

第二轮版本用户仍报告失败：有时钉钉输入框内仍显示光标，但没有填入内容；退出测试应用后，系统 Win+V 选取相同文本可以成功。可见光标并不等价于已核实 Win32 或虚拟编辑器的焦点状态，6 档原生 Edit 回归也不能代表钉钉链路已通过。

本轮只增加诊断，不变更焦点等待或 Ctrl+V 注入策略。根代理只读检查了当前 8 个钉钉进程，TokenIntegrityLevel 均为 Medium，启动构建 shell 也为 Medium；这不支持“当前钉钉比发送方提权”的常见解释，但新启动的实际 X Clipboard 进程完整性级别仍需单独核验，不能泛化其他会话。

以进程环境变量 `X_CLIPBOARD_PASTE_DIAGNOSTICS=1` 启动后，每次回贴结束把缓存在内存的阶段事件追加到：

```text
%TEMP%\x-clipboard-paste-<实际应用进程PID>.jsonl
```

诊断默认关闭；值不为 `1` 时不创建日志。单次回贴最多记录 64 条阶段事件及一条 `finished`，所有正常 Result 错误返回也写 `finished.success=false` 与最后阶段。日志只包含事件阶段、相对耗时、窗口/输入控件句柄和类名、保存与当前线程/进程身份、修饰键按下布尔值，以及 AttachThreadInput / SetFocus / SendInput 返回数值。未记录剪贴板正文、窗口标题、账号或密钥；不在低级键盘钩子中写日志，不在恢复焦点与发送输入之间写磁盘。

关键判读：

- `target-foreground`、`focus-observed`、`focus-ready` 区分顶层激活与原控件就绪；`restore-validate`、`attach-returned`、`set-focus-returned` 显示是否尝试了控件恢复。
- `before-input` 记录最终输入前的前台、焦点与物理修饰键状态。
- `send-input-returned` 中 `sent=requested` 只表示输入事件进入系统输入流，不能证明目标编辑器消费或执行了粘贴。
- SetFocus 成功但先前没有焦点时也可能返回空句柄；不能仅凭 `returned_previous_focus=false` 判定恢复失败，应结合随后 focus 快照。
- SendInput 失败前清零线程 LastError，随后立即记录计数与错误码；UIPI 原因不保证能从 LastError 识别。其他 API 的 LastError 只能作附带信息，不能把陈旧错误码当根因。

### 扫描码与批量按键的静态结论

当前发送四个虚拟键事件：Ctrl 按下、V 按下、V 释放、Ctrl 释放；`wScan=0`，未设置 `KEYEVENTF_SCANCODE`，一次调用 SendInput。微软 [SendInput 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput) 的官方示例同样零初始化 INPUT 并批量发送四个虚拟键事件；文档保证数组串行进入输入流，不与其他调用或用户输入交错。因此不能仅从“扫描码为零”或“四键批量发送”判定该调用不合法，也没有官方要求在这些事件之间插入固定延时。

微软 [KEYBDINPUT 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput) 规定启用 `KEYEVENTF_SCANCODE` 时由 wScan 标识键、wVk 被忽略；扫描码适用于模拟物理按键，但目标是否依赖扫描码必须另取证。[MapVirtualKey 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyw) 说明未区分左右的 VK_CONTROL 可映射为左侧扫描码，也支持按指定键盘布局映射。Qt/CEF 是否读取扫描码或异步按键状态、系统 Win+V 使用何种回贴路径，当前都没有由这三份 API 文档证明；本轮不据此切换注入方式或继续增加延时。

## 第四轮：激活覆盖首次焦点恢复

真实诊断进程31776共记录5次回贴。钉钉进程17280的3次中，2次在1204/1207ms等待原控件超时，完全没有调用SendInput；另1次在988ms恢复原控件，1032ms提交4/4事件。后者不证明用户未再点击，也不证明编辑器消费了粘贴。另两次属于Chrome类窗口，不计作钉钉通过。各次物理修饰键状态均为未按下。

新增 `paste_probe --redirect-first-focus`：自建编辑框第一次收到恢复焦点消息后同步将焦点退回自己的顶层窗口，不安排自动恢复计时器。原实现实际退出1，提示原输入框尚未恢复；修复后实际编辑框收到合成测试文本，退出0。它复现首次恢复被覆盖这一缺口，不代表已经证明钉钉内部机制。

修复将一次恢复改为最多两次：仅当前焦点为空或原顶层窗口、且状态稳定40ms后，恢复同一个原控件；第二次前重新等待稳定。出现其他子控件或其他前台窗口立即取消，销毁/身份变化继续拒绝。最终仍要求精确原控件稳定40ms才发送按键，总等待上限1.2秒；本轮没有变更按键注入。

新增 `--other-control-focus` 在首次恢复消息中把焦点转到同一窗口的另一个编辑框，断言取消且两个编辑框均无内容。该负向测试及之前六组探针实际通过；12项核心测试、含ui-capture的全目标Clippy和默认构建通过。

SetFocus诊断现在调用前清LastError、返回后立即采样；避免查询空句柄产生的陈旧1400污染判读。Qt官方源码表明嵌入式原生控件可能把焦点转回顶层，再由逻辑focusObject路由键盘事件；这说明不能以顶层HWND本身推断逻辑编辑框。但当前证据不足以放宽原控件一致性条件。本轮仍保留严格控件校验，钉钉真实复测尚待结果。

源码参考：[Qt QWidget焦点恢复](https://github.com/qt/qtbase/blob/v5.15.2/src/widgets/kernel/qwidget.cpp#L6406-L6420)、[Qt Windows激活](https://github.com/qt/qtbase/blob/v5.15.2/src/plugins/platforms/windows/qwindowswindow.cpp#L2539-L2574)。

## 最终验收：用户确认当前钉钉输入框自动填入

2026-10-25，用户在第四轮版进程41968执行“点击钉钉输入框 → Ctrl+Alt+Shift+9 → 双击文本记录 → 不再点击输入框”，确认“能直接填入”。该次真实输入框验收通过，未发送聊天消息。

同版诊断两次尝试均保存/恢复到钉钉原输入控件0x106e2：在67/68ms完成SetFocus，123/124ms提交4/4事件，未重现原先1.2秒超时。日志只用于解释阶段；实际填入由用户确认，不能将SendInput计数代替此结果。日志显示首轮激活期间focus先从空变成root；等待稳定后第一次恢复即成功，未用到第二次兜底。

收尾已从源代码移除临时 `PasteDiagnostics`、环境变量入口及文件日志写入。稳定焦点等待、最多两次精确恢复、其他控件/窗口取消等行为保留。移除后重新通过八组真实控件探针、12项核心测试、Clippy与默认构建，正式测试包不含诊断写入路径。上文环境变量和日志说明仅保留为排查历史，不是当前功能。

本次通过覆盖用户当前钉钉版本与输入框、同权限环境、自定义测试快捷键。系统Win+V互斥、其他钉钉版本/提权环境、Office及跨平台发布测试仍是独立项目，不能据此一并宣称通过。

## 标题栏搜索框：在唤起与选取阶段保留输入焦点

用户追加反馈：钉钉标题栏搜索框一旦失焦便不再接受原回贴。此前“先激活面板、选取后恢复焦点”不能覆盖此类状态会随失焦消失的输入控件。

代码核查发现两处激活：复用面板时 `show_window` 调用 SetForegroundWindow；GPUI Windows 0.3.9 的 WM_MOUSEACTIVATE 默认返回MA_ACTIVATE，鼠标选记录也会激活。修复同时覆盖首次创建与再次打开：Windows快捷面板先隐藏创建（show=false/focus=false），在UI线程安装独立子类，设置WS_EX_NOACTIVATE，以SWP_NOACTIVATE显示至顶层。鼠标激活消息返回MA_NOACTIVATE，让点击继续送到面板而保留原前台窗口。该子类在WM_NCDESTROY移除，不修改GPUI依赖源码。

明确点击面板自己的搜索框会通过InputEvent::Focus切换到面板输入，搜索可正常打字。普通行选择/双击/粘贴按钮不主动激活。面板保持焦点保护且原目标仍在前台时，通过现有低级键盘钩子转发上/下、Enter、Shift+Enter、Space、Esc到原有UI处理；键盘钩子仅投递事件，配对吞掉对应keyup。面板隐藏、主动搜索、目标切到其他窗口后不转发新按键，原窗口不会收到这次面板导航或Enter。

新增命令：`cargo run --locked --no-default-features --example paste_probe -- --preserve-focus`。使用自身Win32编辑框记录WM_KILLFOCUS，覆盖显示面板、WM_MOUSEACTIVATE判定、隐藏和回贴；原代码实跑退出1：`showing quick panel blurred the original search input`。修复后实跑通过，原编辑框直到收到真实剪贴板文本始终没有失焦，鼠标激活返回MA_NOACTIVATE。这个自建控件测试证明平台函数行为，不代替钉钉标题栏输入框与真实GPUI鼠标/键盘的实机验收。

12项核心测试、含ui-capture全目标Clippy、默认Debug构建，以及新增焦点保留与原有8组回贴探针通过。新Debug版已按正常数据目录后台启动，钉钉标题栏实测待用户结果。

依据：[微软WS_EX_NOACTIVATE说明](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)、[WM_MOUSEACTIVATE返回值](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mouseactivate)、[SetWindowSubclass线程约束](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass)。子类安装与移除均在拥有该窗口的UI线程执行。
