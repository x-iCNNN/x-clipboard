use super::*;
use crate::model::{FormatData, MAX_CAPTURE_BYTES};
use ::windows::{
    Win32::{
        Foundation::*,
        System::{DataExchange::*, LibraryLoader::GetModuleHandleW, Memory::*, Threading::*},
        UI::{
            Input::KeyboardAndMouse::*,
            Shell::{DefSubclassProc, DragQueryFileW, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};
use anyhow::{Context, bail, ensure};
use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

static EVENTS: OnceLock<Sender<PlatformEvent>> = OnceLock::new();
static HOTKEY: AtomicU32 = AtomicU32::new(0x0800 | 0x56);
static KEY_HELD: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicBool = AtomicBool::new(false);
static OWNER: AtomicUsize = AtomicUsize::new(0);
const INPUT_ORIGIN: usize = 0x58434c50;
static QUICK_PANEL: AtomicUsize = AtomicUsize::new(0);
static QUICK_TARGET: AtomicUsize = AtomicUsize::new(0);
static QUICK_KEYS: AtomicU32 = AtomicU32::new(0);
static QUICK_SEARCH: AtomicBool = AtomicBool::new(false);
static SEARCH_KEYS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
const SEARCH_KEY_MESSAGE: u32 = WM_APP + 0x584;
const QUICK_RESIZE_MESSAGE: u32 = WM_APP + 0x585;
static SEARCH_MESSAGE_HOOK: AtomicUsize = AtomicUsize::new(0);
static HOOK_MODIFIERS: AtomicU32 = AtomicU32::new(0);
const MODIFIER_KEYS: [VIRTUAL_KEY; 8] = [
    VK_LCONTROL,
    VK_RCONTROL,
    VK_LMENU,
    VK_RMENU,
    VK_LSHIFT,
    VK_RSHIFT,
    VK_LWIN,
    VK_RWIN,
];

#[derive(Clone, Copy)]
struct PasteFocus {
    window: usize,
    control: usize,
    thread: u32,
    process: u32,
    control_thread: u32,
    control_process: u32,
}
static PASTE_FOCUS: Mutex<Option<PasteFocus>> = Mutex::new(None);

pub struct PlatformClipboard {
    own_version: AtomicU32,
    io: Mutex<()>,
    thread_id: u32,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl PlatformClipboard {
    pub fn start(events: Sender<PlatformEvent>) -> Result<Self> {
        ensure!(EVENTS.set(events).is_ok(), "剪贴板服务已启动");
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("clipboard-events".into())
            .spawn(move || unsafe {
                let setup = (|| -> Result<(HWND, HHOOK)> {
                    let instance = GetModuleHandleW(None)?;
                    let class = WNDCLASSW {
                        hInstance: instance.into(),
                        lpszClassName: w!("XClipboardMonitor"),
                        lpfnWndProc: Some(window_proc),
                        ..Default::default()
                    };
                    ensure!(RegisterClassW(&class) != 0, "无法注册剪贴板监听窗口");
                    let hwnd = CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("XClipboardMonitor"),
                        w!(""),
                        WINDOW_STYLE::default(),
                        0,
                        0,
                        0,
                        0,
                        Some(HWND_MESSAGE),
                        None,
                        Some(instance.into()),
                        None,
                    )?;
                    AddClipboardFormatListener(hwnd)?;
                    let hook = SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(keyboard_hook),
                        Some(instance.into()),
                        0,
                    )?;
                    let held = MODIFIER_KEYS
                        .iter()
                        .enumerate()
                        .fold(0, |mask, (bit, key)| {
                            mask | if GetAsyncKeyState(key.0 as i32) < 0 {
                                1 << bit
                            } else {
                                0
                            }
                        });
                    HOOK_MODIFIERS.store(held, Ordering::Release);
                    OWNER.store(hwnd.0 as usize, Ordering::Release);
                    Ok((hwnd, hook))
                })();
                match setup {
                    Ok((hwnd, hook)) => {
                        let _ = tx.send(Ok(GetCurrentThreadId()));
                        let mut msg = MSG::default();
                        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                        let _ = UnhookWindowsHookEx(hook);
                        let _ = RemoveClipboardFormatListener(hwnd);
                        let _ = DestroyWindow(hwnd);
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                    }
                }
            })?;
        let thread_id = rx.recv().context("监听线程启动失败")??;
        Ok(Self {
            own_version: AtomicU32::new(0),
            io: Mutex::new(()),
            thread_id,
            thread: Mutex::new(Some(thread)),
        })
    }
}
impl Drop for PlatformClipboard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn dispatch(event: PlatformEvent) {
    if let Some(tx) = EVENTS.get()
        && tx.try_send(event).is_err()
    {
        DROPPED.store(true, Ordering::Release);
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        dispatch(PlatformEvent::Changed(unsafe {
            GetClipboardSequenceNumber()
        }));
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
unsafe extern "system" fn keyboard_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let data = unsafe { &*(l.0 as *const KBDLLHOOKSTRUCT) };
        if data.dwExtraInfo != INPUT_ORIGIN {
            let down = matches!(w.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            // Consumed modifier events do not update GetAsyncKeyState. Track both
            // sides explicitly so Ctrl+A/C/V and Shift selection still work.
            let modifier = MODIFIER_KEYS
                .iter()
                .position(|key| key.0 as u32 == data.vkCode)
                .or(match data.vkCode as u16 {
                    value if value == VK_CONTROL.0 => Some(0),
                    value if value == VK_MENU.0 => Some(2),
                    value if value == VK_SHIFT.0 => Some(4),
                    _ => None,
                });
            if let Some(bit) = modifier {
                if down {
                    HOOK_MODIFIERS.fetch_or(1 << bit, Ordering::AcqRel);
                } else {
                    HOOK_MODIFIERS.fetch_and(!(1 << bit), Ordering::AcqRel);
                }
            }
            let held = HOOK_MODIFIERS.load(Ordering::Acquire);
            let modifiers = (((held & 3 != 0) as usize) << 8)
                | (((held & 12 != 0) as usize) << 9)
                | (((held & 48 != 0) as usize) << 10)
                | (((held & 192 != 0) as usize) << 11);
            let vk = data.vkCode as usize;
            if vk < 256 {
                let bit = 1u64 << (vk % 64);
                let captured =
                    !down && SEARCH_KEYS[vk / 64].fetch_and(!bit, Ordering::AcqRel) & bit != 0;
                let panel = QUICK_PANEL.load(Ordering::Acquire);
                let own_shortcut = data.vkCode == HOTKEY.load(Ordering::Acquire) & 255
                    && modifiers as u32 & 0xf00 == HOTKEY.load(Ordering::Acquire) & 0xf00;
                if captured
                    || (QUICK_SEARCH.load(Ordering::Acquire)
                        && quick_panel_keeps_focus(panel)
                        && unsafe { GetForegroundWindow().0 as usize }
                            == QUICK_TARGET.load(Ordering::Acquire)
                        && modifiers & 0x800 == 0
                        && !own_shortcut)
                {
                    if down {
                        SEARCH_KEYS[vk / 64].fetch_or(bit, Ordering::AcqRel);
                    }
                    let flags = modifiers
                        | ((!down as usize) << 12)
                        | (((data.flags.0 & LLKHF_EXTENDED.0 != 0) as usize) << 13);
                    // The hook only queues events. Text translation and GPUI input run on the UI thread.
                    if quick_panel_keeps_focus(panel) {
                        let _ = unsafe {
                            PostMessageW(
                                Some(HWND(panel as *mut _)),
                                SEARCH_KEY_MESSAGE,
                                WPARAM(vk | flags),
                                LPARAM(data.scanCode as isize),
                            )
                        };
                    }
                    return LRESULT(1);
                }
            }
            if let Some((key, bit)) = match data.vkCode as u16 {
                value if value == VK_UP.0 => Some(("up", 1)),
                value if value == VK_DOWN.0 => Some(("down", 2)),
                value if value == VK_RETURN.0 => Some(("enter", 4)),
                value if value == VK_ESCAPE.0 => Some(("escape", 8)),
                value if value == VK_SPACE.0 => Some(("space", 16)),
                _ => None,
            } {
                let down = matches!(w.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                if !down && QUICK_KEYS.fetch_and(!bit, Ordering::AcqRel) & bit != 0 {
                    return LRESULT(1);
                }
                let panel = QUICK_PANEL.load(Ordering::Acquire);
                let pressed = |vk: VIRTUAL_KEY| unsafe { GetAsyncKeyState(vk.0 as i32) < 0 };
                if down
                    && quick_panel_keeps_focus(panel)
                    && unsafe { GetForegroundWindow().0 as usize }
                        == QUICK_TARGET.load(Ordering::Acquire)
                    && ![VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
                        .iter()
                        .any(|key| pressed(*key))
                    && (!pressed(VK_SHIFT) || key == "enter")
                {
                    QUICK_KEYS.fetch_or(bit, Ordering::AcqRel);
                    dispatch(PlatformEvent::QuickKey {
                        key,
                        shift: pressed(VK_SHIFT),
                    });
                    return LRESULT(1);
                }
            }
            let packed = HOTKEY.load(Ordering::Acquire);
            let key = packed & 255;
            if data.vkCode == key {
                let down = w.0 as u32 == WM_KEYDOWN || w.0 as u32 == WM_SYSKEYDOWN;
                if !down && KEY_HELD.swap(false, Ordering::AcqRel) {
                    return LRESULT(1);
                }
                let mods = modifiers as u32;
                if down && mods == packed & 0xf00 {
                    if !KEY_HELD.swap(true, Ordering::AcqRel) {
                        dispatch(PlatformEvent::ShowQuick(foreground()));
                    }
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, w, l) }
}

struct ClipboardLock;
impl ClipboardLock {
    fn acquire() -> Result<Self> {
        let started = Instant::now();
        loop {
            if unsafe { OpenClipboard(Some(HWND(OWNER.load(Ordering::Acquire) as *mut _))) }.is_ok()
            {
                return Ok(Self);
            }
            if started.elapsed() > Duration::from_millis(350) {
                bail!("剪贴板被其他应用占用，350 ms 内重试失败");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ClipboardLock {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn format_name(id: u32) -> Option<String> {
    if [8, 13, 15, 17].contains(&id) {
        return Some(format!("windows:{id}"));
    }
    if id < 0xc000 {
        return None;
    }
    let mut name = [0u16; 256];
    let n = unsafe { GetClipboardFormatNameW(id, &mut name) };
    if n <= 0 {
        return None;
    }
    let name = String::from_utf16_lossy(&name[..n as usize]);
    ["HTML Format", "Rich Text Format", "PNG"]
        .contains(&name.as_str())
        .then(|| format!("windows:{name}"))
}
fn registered(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

fn read_memory(id: u32, remaining: usize) -> Result<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(id)?;
        let memory = HGLOBAL(handle.0);
        let len = GlobalSize(memory);
        ensure!(len <= remaining, "单次复制超过 64 MB，未保存");
        ensure!(len > 0, "剪贴板格式数据为空或无效");
        let ptr = GlobalLock(memory);
        ensure!(!ptr.is_null(), "无法锁定剪贴板数据");
        let bytes = std::slice::from_raw_parts(ptr.cast::<u8>(), len).to_vec();
        let _ = GlobalUnlock(memory);
        Ok(bytes)
    }
}

impl ClipboardBackend for PlatformClipboard {
    fn read(&self, version: u32, respect_sensitive: bool) -> Result<Option<ClipContent>> {
        let _io = self.io.lock().unwrap();
        let _lock = ClipboardLock::acquire()?;
        let actual = unsafe { GetClipboardSequenceNumber() };
        // The clipboard owner is changed by every external copy, including identical content.
        // Checking ownership avoids a close/read-sequence race that could suppress another app's copy.
        if unsafe { GetClipboardOwner() }
            .ok()
            .is_some_and(|h| h.0 as usize == OWNER.load(Ordering::Acquire))
        {
            return Ok(None);
        }
        if version != 0 && actual != version {
            bail!("复制变化过快：上一版剪贴板在读取前已被覆盖");
        }
        if respect_sensitive
            && unsafe {
                IsClipboardFormatAvailable(registered(
                    "ExcludeClipboardContentFromMonitorProcessing",
                ))
            }
            .is_ok()
        {
            return Ok(None);
        }
        let mut content = ClipContent::default();
        let mut id = 0;
        let mut used = 0;
        loop {
            id = unsafe { EnumClipboardFormats(id) };
            if id == 0 {
                break;
            }
            let Some(name) = format_name(id) else {
                continue;
            };
            let mut bytes = read_memory(id, MAX_CAPTURE_BYTES - used)?;
            if id == 13 {
                let utf: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|x| u16::from_le_bytes([x[0], x[1]]))
                    .take_while(|c| *c != 0)
                    .collect();
                content.text = String::from_utf16_lossy(&utf);
                bytes.truncate((utf.len() + 1) * 2);
            }
            if id == 15 {
                unsafe {
                    let drop = ::windows::Win32::UI::Shell::HDROP(GetClipboardData(id)?.0);
                    let count = DragQueryFileW(drop, u32::MAX, None);
                    ensure!(count <= 10000, "文件数量超过 10,000");
                    for n in 0..count {
                        let len = DragQueryFileW(drop, n, None);
                        let mut path = vec![0; len as usize + 1];
                        DragQueryFileW(drop, n, Some(&mut path));
                        content
                            .files
                            .push(String::from_utf16_lossy(&path[..len as usize]));
                    }
                }
            }
            used += bytes.len();
            content.formats.push(FormatData { name, bytes });
        }
        content.source = clipboard_source();
        if content.formats.is_empty() {
            return Ok(None);
        }
        content.validate()?;
        Ok(Some(content))
    }
    fn write(&self, content: &ClipContent, plain: bool) -> Result<()> {
        content.validate()?;
        let _io = self.io.lock().unwrap();
        let mut formats = Vec::new();
        if plain || content.formats.iter().any(|f| f.name == "text/plain") {
            ensure!(!content.text.is_empty(), "此记录没有纯文本内容");
            formats.push((
                13,
                content
                    .text
                    .encode_utf16()
                    .chain(Some(0))
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ));
        } else {
            for f in &content.formats {
                if let Some(name) = f.name.strip_prefix("windows:") {
                    let id = name.parse::<u32>().unwrap_or_else(|_| registered(name));
                    ensure!(id != 0, "无法注册剪贴板格式");
                    formats.push((id, f.bytes.clone()));
                }
            }
        }
        ensure!(!formats.is_empty(), "当前平台不支持写回此记录的格式");
        if !plain && !content.files.is_empty() {
            for path in &content.files {
                ensure!(
                    std::path::Path::new(path).try_exists()?,
                    "文件引用已失效：{path}"
                );
            }
            formats.push((
                registered("Preferred DropEffect"),
                1u32.to_le_bytes().to_vec(),
            ));
        }
        // Allocate before clearing the system clipboard so allocation failures preserve it.
        let mut handles = Vec::new();
        for (id, bytes) in formats {
            handles.push((id, GlobalBuffer::new(&bytes)?));
        }
        let _lock = ClipboardLock::acquire()?;
        unsafe {
            EmptyClipboard()?;
        }
        for (id, mut buffer) in handles {
            unsafe {
                SetClipboardData(id, Some(HANDLE(buffer.0.0)))?;
            }
            buffer.0 = HGLOBAL::default();
        }
        // CloseClipboard publishes the sequence number before releasing our I/O lock.
        drop(_lock);
        self.own_version
            .store(unsafe { GetClipboardSequenceNumber() }, Ordering::Release);
        Ok(())
    }
    fn shortcut(&self, shortcut: &str) -> Result<()> {
        let packed = parse_shortcut(shortcut)?;
        if packed != 0x856 {
            let mut mods = MOD_NOREPEAT;
            if packed & 0x100 != 0 {
                mods |= MOD_CONTROL;
            }
            if packed & 0x200 != 0 {
                mods |= MOD_ALT;
            }
            if packed & 0x400 != 0 {
                mods |= MOD_SHIFT;
            }
            if packed & 0x800 != 0 {
                mods |= MOD_WIN;
            }
            unsafe {
                RegisterHotKey(None, 0x5843, mods, packed & 255)
                    .context("快捷键已被其他应用占用")?;
                UnregisterHotKey(None, 0x5843)?;
            }
        }
        HOTKEY.store(packed, Ordering::Release);
        Ok(())
    }
    fn paste(&self, target: usize) -> Result<()> {
        let hwnd = HWND(target as *mut _);
        let saved = PASTE_FOCUS
            .lock()
            .unwrap()
            .filter(|saved| saved.window == target);

        ensure!(target != 0, "原目标窗口不可用；内容已复制，请手动粘贴");
        if let Some(saved) = saved {
            validate_paste_target(hwnd, saved)?;
        }
        let previous = unsafe { GetForegroundWindow() };

        unsafe {
            ensure!(
                IsWindow(Some(hwnd)).as_bool(),
                "原目标窗口已关闭；内容已复制，请手动粘贴"
            );
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let activated = SetForegroundWindow(hwnd).as_bool();

            ensure!(
                activated || GetForegroundWindow() == hwnd,
                "无法恢复目标窗口焦点；内容已复制，请手动粘贴"
            );
        }

        let deadline = Instant::now() + Duration::from_millis(1200);
        while [VK_LWIN, VK_RWIN, VK_CONTROL, VK_SHIFT, VK_MENU]
            .iter()
            .any(|k| unsafe { GetAsyncKeyState(k.0 as i32) < 0 })
        {
            ensure!(
                Instant::now() < deadline,
                "快捷键仍被按住；内容已复制，请松开后手动粘贴"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        wait_for_paste_focus(hwnd, previous, saved)?;

        ensure!(
            unsafe { GetForegroundWindow() } == hwnd,
            "目标焦点已变化；内容已复制，请手动粘贴"
        );
        ensure!(
            [VK_LWIN, VK_RWIN, VK_CONTROL, VK_SHIFT, VK_MENU]
                .iter()
                .all(|key| unsafe { GetAsyncKeyState(key.0 as i32) >= 0 }),
            "粘贴期间按键状态已变化；内容已复制，请手动粘贴"
        );
        send_keys(&[
            (VK_CONTROL, false),
            (VK_V, false),
            (VK_V, true),
            (VK_CONTROL, true),
        ])
    }
    fn capabilities(&self) -> &'static str {
        "Windows · 事件监听 · 多格式采集 · Win+V 接管 · 原窗口粘贴"
    }
    fn shutdown(&self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
struct GlobalBuffer(HGLOBAL);
impl GlobalBuffer {
    fn new(bytes: &[u8]) -> Result<Self> {
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
            let result = Self(handle);
            let ptr = GlobalLock(handle);
            ensure!(!ptr.is_null(), "剪贴板内存分配失败");
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
            let _ = GlobalUnlock(handle);
            Ok(result)
        }
    }
}
impl Drop for GlobalBuffer {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe {
                let _ = GlobalFree(Some(self.0));
            }
        }
    }
}

pub fn foreground() -> usize {
    let window = unsafe { GetForegroundWindow() };
    // Capture only window identities here. Never inspect editor text or wait in the hook.
    let focus = focus_snapshot(window);
    if let Ok(mut saved) = PASTE_FOCUS.try_lock() {
        *saved = focus;
    }
    window.0 as usize
}

fn focus_snapshot(window: HWND) -> Option<PasteFocus> {
    unsafe {
        let mut process = 0;
        let thread = GetWindowThreadProcessId(window, Some(&mut process));
        if thread == 0 {
            return None;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(thread, &mut info).ok()?;
        let mut control_process = 0;
        let control_thread = GetWindowThreadProcessId(info.hwndFocus, Some(&mut control_process));
        Some(PasteFocus {
            window: window.0 as usize,
            control: info.hwndFocus.0 as usize,
            thread,
            process,
            control_thread,
            control_process,
        })
    }
}

fn wait_for_paste_focus(window: HWND, previous: HWND, saved: Option<PasteFocus>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_millis(1200);
    let mut ready_since = None;
    let mut ready_control = 0;
    let mut activated = false;
    let mut restore_attempts = 0;
    let mut restore_since = None;
    let mut observed_control = None;
    loop {
        unsafe {
            ensure!(
                IsWindow(Some(window)).as_bool(),
                "原目标窗口已关闭；内容已复制，请手动粘贴"
            );
            let foreground = GetForegroundWindow();
            if foreground != window {
                ensure!(
                    !activated && (foreground == previous || foreground.0.is_null()),
                    "目标焦点已变化；内容已复制，请手动粘贴"
                );
                ensure!(
                    Instant::now() < deadline,
                    "目标窗口尚未激活；内容已复制，请手动粘贴"
                );
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            if !activated {
                activated = true;
            }
        }
        let current =
            focus_snapshot(window).context("无法读取目标输入焦点；内容已复制，请手动粘贴")?;
        if observed_control != Some(current.control) {
            observed_control = Some(current.control);
            restore_since = None;
        }
        if let Some(saved) = saved {
            validate_paste_target(window, saved)?;
        }
        let expected = saved.filter(|saved| saved.control != 0);
        let control = HWND(current.control as *mut _);
        let belongs = current.control != 0
            && (control == window || unsafe { IsChild(window, control).as_bool() });
        let ready = belongs && expected.is_none_or(|saved| current.control == saved.control);
        if ready {
            restore_since = None;
            if ready_control != current.control {
                ready_since = None;
                ready_control = current.control;
            }
            let since = ready_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_millis(40) {
                return Ok(());
            }
        } else {
            ready_since = None;
            if let Some(saved) = expected {
                ensure!(
                    current.control == 0 || control == window,
                    "目标输入框已变化；内容已复制，请手动粘贴"
                );
                let since = restore_since.get_or_insert_with(Instant::now);
                // Activation handlers can replace the first restored focus with the root.
                // Retry only the original control, after a stable root/empty focus interval.
                if restore_attempts < 2 && since.elapsed() >= Duration::from_millis(40) {
                    restore_paste_focus(window, saved)?;
                    restore_attempts += 1;
                    restore_since = None;
                }
            }
        }
        ensure!(
            Instant::now() < deadline,
            "原输入框尚未恢复焦点；内容已复制，请点击输入框后手动粘贴"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn validate_paste_target(window: HWND, saved: PasteFocus) -> Result<()> {
    unsafe {
        let mut process = 0;
        let thread = GetWindowThreadProcessId(window, Some(&mut process));
        ensure!(
            window.0 as usize == saved.window && thread == saved.thread && process == saved.process,
            "原目标窗口已被替换；内容已复制，请手动粘贴"
        );
        if saved.control != 0 {
            let control = HWND(saved.control as *mut _);
            ensure!(
                IsWindow(Some(control)).as_bool(),
                "原输入框已关闭；内容已复制，请点击输入框后手动粘贴"
            );
            let mut process = 0;
            let thread = GetWindowThreadProcessId(control, Some(&mut process));
            ensure!(
                thread != 0
                    && thread == saved.control_thread
                    && process == saved.control_process
                    && (control == window || IsChild(window, control).as_bool()),
                "原输入框已被替换；内容已复制，请手动粘贴"
            );
        }
    }
    Ok(())
}

struct InputQueueAttachment {
    source: u32,
    target: u32,
}
impl InputQueueAttachment {
    fn attach(target: u32) -> Result<Option<Self>> {
        unsafe {
            let source = GetCurrentThreadId();
            if source == target {
                return Ok(None);
            }
            // The clipboard worker may not yet have a USER message queue.
            let mut message = MSG::default();
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
            ensure!(
                AttachThreadInput(source, target, true).as_bool(),
                "无法连接原输入框的焦点队列；内容已复制，请手动粘贴"
            );
            Ok(Some(Self { source, target }))
        }
    }
}
impl Drop for InputQueueAttachment {
    fn drop(&mut self) {
        unsafe {
            let _ = AttachThreadInput(self.source, self.target, false);
        }
    }
}

fn restore_paste_focus(window: HWND, saved: PasteFocus) -> Result<()> {
    validate_paste_target(window, saved)?;
    let control = HWND(saved.control as *mut _);
    unsafe {
        ensure!(
            GetForegroundWindow() == window,
            "目标焦点已变化；内容已复制，请手动粘贴"
        );
        ensure!(
            IsWindowVisible(control).as_bool() && IsWindowEnabled(control).as_bool(),
            "原输入框不可用；内容已复制，请手动粘贴"
        );
    }
    {
        let attachment = InputQueueAttachment::attach(saved.control_thread);

        let _attachment = attachment?;
        validate_paste_target(window, saved)?;
        unsafe {
            ensure!(
                GetForegroundWindow() == window,
                "目标焦点已变化；内容已复制，请手动粘贴"
            );
            // Only restore the exact captured control; never discover or click a new field.
            // SetFocus can return null when there was no previous focus; verify via GUI info.
            let _ = SetFocus(Some(control));
        }
    }

    ensure!(
        unsafe { GetForegroundWindow() } == window,
        "目标焦点已变化；内容已复制，请手动粘贴"
    );
    validate_paste_target(window, saved)?;
    Ok(())
}
pub fn hide_window(handle: usize) {
    unsafe {
        let _ = ShowWindow(HWND(handle as *mut _), SW_HIDE);
    }
}
pub fn show_window(handle: usize) {
    unsafe {
        let hwnd = HWND(handle as *mut _);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
const QUICK_SUBCLASS: usize = 0x58435150;
unsafe extern "system" fn quick_panel_proc(
    hwnd: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        if message == QUICK_RESIZE_MESSAGE {
            let mut outer = RECT::default();
            let mut client = RECT::default();
            if GetWindowRect(hwnd, &mut outer).is_ok() && GetClientRect(hwnd, &mut client).is_ok() {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    w.0 as i32 + outer.right - outer.left - client.right,
                    l.0 as i32 + outer.bottom - outer.top - client.bottom,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                place_panel(hwnd.0 as usize, "clamp");
            }
            return LRESULT(0);
        }
        if message == WM_MOUSEACTIVATE
            && GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0 != 0
        {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        if message == WM_NCDESTROY {
            let _ = QUICK_PANEL.compare_exchange(
                hwnd.0 as usize,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            let hook = SEARCH_MESSAGE_HOOK.swap(0, Ordering::AcqRel);
            if hook != 0 {
                let _ = UnhookWindowsHookEx(HHOOK(hook as *mut _));
            }
            let _ = RemoveWindowSubclass(hwnd, Some(quick_panel_proc), QUICK_SUBCLASS);
        }
        DefSubclassProc(hwnd, message, w, l)
    }
}
// Rewrite our private queued events before the normal Windows/GPUI accelerator
// and IME translation pass. This keeps text editing on the UI thread without
// activating the panel or depending on GPUI's private window-message IDs.
unsafe extern "system" fn search_message_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 && w.0 == PM_REMOVE.0 as usize {
            let msg = &mut *(l.0 as *mut MSG);
            if msg.message == SEARCH_KEY_MESSAGE {
                let packed = msg.wParam.0;
                let vk = packed & 255;
                let up = packed & (1 << 12) != 0;
                let mut state = [0u8; 256];
                let _ = GetKeyboardState(&mut state);
                for (key, flag) in [(VK_CONTROL, 8), (VK_MENU, 9), (VK_SHIFT, 10)] {
                    state[key.0 as usize] = if packed & (1 << flag) != 0 { 0x80 } else { 0 };
                }
                state[vk] = (state[vk] & 1) | if up { 0 } else { 0x80 };
                let _ = SetKeyboardState(&state);
                let scan = msg.lParam.0;
                msg.lParam = LPARAM(
                    1 | ((scan & 255) << 16)
                        | (((packed >> 13) as isize & 1) << 24)
                        | if up {
                            (1isize << 30) | (1isize << 31)
                        } else {
                            0
                        },
                );
                msg.wParam = WPARAM(vk);
                msg.message = if !quick_panel_keeps_focus(msg.hwnd.0 as usize) {
                    WM_NULL
                } else if vk == VK_PACKET.0 as usize {
                    msg.wParam = WPARAM(scan as usize);
                    if up { WM_NULL } else { WM_CHAR }
                } else if up {
                    WM_KEYUP
                } else {
                    WM_KEYDOWN
                };
            }
        }
        CallNextHookEx(None, code, w, l)
    }
}
pub fn quick_panel_keeps_focus(handle: usize) -> bool {
    if handle == 0 {
        return false;
    }
    unsafe {
        let hwnd = HWND(handle as *mut _);
        IsWindowVisible(hwnd).as_bool()
            && GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0 != 0
    }
}
pub fn prepare_quick_panel(handle: usize, target: usize) -> Result<()> {
    unsafe {
        let hwnd = HWND(handle as *mut _);
        if SEARCH_MESSAGE_HOOK.load(Ordering::Acquire) == 0 {
            let hook = SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(search_message_hook),
                None,
                GetCurrentThreadId(),
            )?;
            SEARCH_MESSAGE_HOOK.store(hook.0 as usize, Ordering::Release);
        }
        ensure!(
            SetWindowSubclass(hwnd, Some(quick_panel_proc), QUICK_SUBCLASS, 0).as_bool(),
            "无法启用快捷面板焦点保护"
        );
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, (style | WS_EX_NOACTIVATE.0) as i32);
        QUICK_TARGET.store(target, Ordering::Release);
        QUICK_PANEL.store(handle, Ordering::Release);
        QUICK_SEARCH.store(false, Ordering::Release);
    }
    Ok(())
}
pub fn show_quick_panel(handle: usize, target: usize) -> Result<()> {
    prepare_quick_panel(handle, target)?;
    unsafe {
        let hwnd = HWND(handle as *mut _);
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    Ok(())
}
pub fn set_quick_search_mode(handle: usize, enabled: bool) {
    if QUICK_PANEL.load(Ordering::Acquire) == handle {
        if QUICK_SEARCH.swap(enabled, Ordering::AcqRel) == enabled {
            return;
        }
        if enabled {
            unsafe {
                let target = HWND(QUICK_TARGET.load(Ordering::Acquire) as *mut _);
                let thread = GetWindowThreadProcessId(target, None);
                let _ = ActivateKeyboardLayout(
                    GetKeyboardLayout(thread),
                    ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0),
                );
            }
        }
    }
}
pub fn resize_quick_panel(handle: usize, width: f32, height: f32, scale: f32) {
    // GPUI's normal resize activates a window. Queue an explicit nonactivating
    // resize so WM_SIZE is delivered outside GPUI's current window borrow.
    let _ = unsafe {
        PostMessageW(
            Some(HWND(handle as *mut _)),
            QUICK_RESIZE_MESSAGE,
            WPARAM((width * scale).round() as usize),
            LPARAM((height * scale).round() as isize),
        )
    };
}
pub fn place_panel(handle: usize, position: &str) {
    use ::windows::Win32::Graphics::Gdi::*;
    unsafe {
        let hwnd = HWND(handle as *mut _);
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return;
        }
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let monitor = if position != "clamp" {
            MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
        };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let work = info.rcWork;
        let x = match position {
            "cursor" => cursor.x - 24,
            "center" => work.left + (work.right - work.left - width) / 2,
            _ => rect.left,
        };
        let y = match position {
            "cursor" => cursor.y + 16,
            "center" => work.top + (work.bottom - work.top - height) / 2,
            _ => rect.top,
        };
        let x = x.max(work.left).min((work.right - width).max(work.left));
        let y = y.max(work.top).min((work.bottom - height).max(work.top));
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
pub fn dropped_events() -> bool {
    DROPPED.swap(false, Ordering::AcqRel)
}
pub fn mask_windows_menu() {
    let _ = send_keys(&[(VK_CONTROL, false), (VK_CONTROL, true)]);
}
fn send_keys(keys: &[(VIRTUAL_KEY, bool)]) -> Result<()> {
    let inputs: Vec<_> = keys
        .iter()
        .map(|(vk, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    dwFlags: if *up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    dwExtraInfo: INPUT_ORIGIN,
                    ..Default::default()
                },
            },
        })
        .collect();
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize;
    if sent != inputs.len() {
        let pending = pending_keys(&keys[..sent.min(keys.len())]);
        let releases: Vec<_> = pending
            .into_iter()
            .map(|key| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: key,
                        dwFlags: KEYEVENTF_KEYUP,
                        dwExtraInfo: INPUT_ORIGIN,
                        ..Default::default()
                    },
                },
            })
            .collect();
        if !releases.is_empty() {
            unsafe {
                let _ = SendInput(&releases, std::mem::size_of::<INPUT>() as i32);
            }
        }
        bail!("系统拒绝了模拟粘贴（可能是应用权限不同）；内容已复制，请手动粘贴");
    }
    Ok(())
}
fn pending_keys(keys: &[(VIRTUAL_KEY, bool)]) -> Vec<VIRTUAL_KEY> {
    let mut pending = vec![];
    for (key, released) in keys {
        pending.retain(|pending| pending != key);
        if !released {
            pending.push(*key);
        }
    }
    pending.reverse();
    pending
}

fn clipboard_source() -> String {
    unsafe {
        let Ok(owner) = GetClipboardOwner() else {
            return "未知应用".into();
        };
        let mut pid = 0;
        GetWindowThreadProcessId(owner, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return "未知应用".into();
        };
        let mut path = vec![0; 32768];
        let mut size = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            ::windows::core::PWSTR(path.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        if result.is_err() {
            return "未知应用".into();
        }
        std::path::Path::new(&String::from_utf16_lossy(&path[..size as usize]))
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}
fn parse_shortcut(value: &str) -> Result<u32> {
    let mut packed = 0;
    for part in value.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "win" | "super" | "meta" => packed |= 0x800,
            "ctrl" | "control" => packed |= 0x100,
            "alt" => packed |= 0x200,
            "shift" => packed |= 0x400,
            key => {
                ensure!(
                    key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric(),
                    "快捷键须为修饰键 + 字母或数字"
                );
                ensure!(packed & 255 == 0, "快捷键只能包含一个字母或数字");
                packed |= key.as_bytes()[0].to_ascii_uppercase() as u32;
            }
        }
    }
    ensure!(
        packed & 0xb00 != 0 && packed & 255 != 0,
        "请至少使用 Ctrl、Alt 或 Win 加一个字母或数字"
    );
    Ok(packed)
}

pub fn set_startup(enabled: bool) -> Result<()> {
    use ::windows::Win32::System::Registry::*;
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = if enabled {
            let text = format!("\"{}\" --background", std::env::current_exe()?.display());
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            RegSetValueExW(key, w!("XClipboard"), Some(0), REG_SZ, Some(&bytes)).ok()
        } else {
            let result = RegDeleteValueW(key, w!("XClipboard"));
            if result == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                result.ok()
            }
        };
        let _ = RegCloseKey(key);
        result?;
        Ok(())
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;
    #[test]
    fn partial_injection_releases_only_keys_left_down() {
        let chord = [
            (VK_CONTROL, false),
            (VK_V, false),
            (VK_V, true),
            (VK_CONTROL, true),
        ];
        assert!(pending_keys(&[]).is_empty());
        assert_eq!(pending_keys(&chord[..1]), vec![VK_CONTROL]);
        assert_eq!(pending_keys(&chord[..2]), vec![VK_V, VK_CONTROL]);
        assert_eq!(pending_keys(&chord[..3]), vec![VK_CONTROL]);
        assert!(pending_keys(&chord).is_empty());
    }
}
