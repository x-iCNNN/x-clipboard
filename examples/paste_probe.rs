//! Opt-in native paste probe. Only interacts with its own temporary windows.
//! Simulates delayed or absent input-control focus restoration after activation.
#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use std::{
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::{LPARAM, WPARAM},
            System::LibraryLoader::GetModuleHandleW,
            UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
        core::w,
    };
    use x_clipboard::{model::ClipContent, platform};
    let arguments: Vec<_> = std::env::args().collect();
    let terminal = arguments.iter().any(|arg| arg == "--terminal");
    probe_window::TERMINAL.store(terminal, Ordering::Release);
    let interrupted = arguments.iter().any(|arg| arg == "--interrupt-focus");
    let destroyed = arguments.iter().any(|arg| arg == "--destroy-control");
    let redirect_focus = arguments.iter().any(|arg| arg == "--redirect-first-focus");
    let other_control = arguments.iter().any(|arg| arg == "--other-control-focus");
    let preserve_focus = arguments.iter().any(|arg| arg == "--preserve-focus");
    let search_focus = arguments.iter().any(|arg| arg == "--search-focus");
    probe_window::PRESERVE_FOCUS.store(preserve_focus, Ordering::Release);
    let manual_focus =
        redirect_focus || other_control || arguments.iter().any(|arg| arg == "--manual-focus");
    probe_window::INTERRUPT.store(interrupted, Ordering::Release);
    probe_window::MANUAL_FOCUS.store(manual_focus, Ordering::Release);
    probe_window::REDIRECT_FIRST_FOCUS.store(redirect_focus, Ordering::Release);
    probe_window::OTHER_CONTROL_FOCUS.store(other_control, Ordering::Release);
    let delay = if arguments.iter().any(|arg| arg == "--immediate-focus") {
        0
    } else {
        arguments
            .iter()
            .find_map(|arg| arg.strip_prefix("--focus-delay="))
            .map(str::parse::<u32>)
            .transpose()?
            .unwrap_or(160)
    };
    let deferred = delay > 0;
    probe_window::DELAY.store(delay, Ordering::Release);
    let (events, _receiver) = crossbeam_channel::bounded(256);
    let backend = platform::start(events)?;
    let original = backend.read(0, false)?;
    struct Restore {
        backend: std::sync::Arc<dyn platform::ClipboardBackend>,
        original: Option<ClipContent>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(content) = &self.original {
                let _ = self.backend.write(content, false);
            }
            self.backend.shutdown();
        }
    }
    let _restore = Restore {
        backend: backend.clone(),
        original,
    };
    backend.write(
        &ClipContent::text("xclipboard-own-window-paste-probe"),
        true,
    )?;
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            hInstance: instance.into(),
            lpszClassName: w!("XClipboardPasteProbe"),
            lpfnWndProc: Some(probe_window::procedure),
            ..Default::default()
        };
        anyhow::ensure!(RegisterClassW(&class) != 0, "cannot register probe window");
        let target = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("XClipboardPasteProbe"),
            w!("X Clipboard · own target probe"),
            WS_OVERLAPPEDWINDOW,
            80,
            80,
            560,
            240,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let editor = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_MULTILINE as u32),
            20,
            30,
            480,
            100,
            Some(target),
            None,
            Some(instance.into()),
            None,
        )?;
        probe_window::EDITOR.store(editor.0 as usize, Ordering::Release);
        if interrupted || redirect_focus || other_control || preserve_focus || terminal {
            let previous = SetWindowLongPtrW(
                editor,
                GWLP_WNDPROC,
                probe_window::editor_procedure as *const () as isize,
            );
            anyhow::ensure!(previous != 0, "cannot subclass probe editor");
            probe_window::EDITOR_PROCEDURE.store(previous, Ordering::Release);
        }
        let _ = ShowWindow(target, SW_SHOW);
        let _ = SetForegroundWindow(target);
        if GetForegroundWindow() != target && arguments.iter().any(|arg| arg == "--activate-owned")
        {
            use windows::Win32::Foundation::{POINT, RECT};
            struct Cursor(POINT);
            impl Drop for Cursor {
                fn drop(&mut self) {
                    unsafe {
                        let _ = SetCursorPos(self.0.x, self.0.y);
                    }
                }
            }
            let mut original = POINT::default();
            GetCursorPos(&mut original)?;
            let _cursor = Cursor(original);
            SetWindowPos(
                target,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )?;
            let mut rect = RECT::default();
            GetWindowRect(target, &mut rect)?;
            let point = POINT {
                x: rect.left + 60,
                y: rect.top + 70,
            };
            anyhow::ensure!(
                GetAncestor(WindowFromPoint(point), GA_ROOT) == target,
                "owned target obscured; refuse to click"
            );
            SetCursorPos(point.x, point.y)?;
            let click = [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP].map(|flags| INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            });
            anyhow::ensure!(
                SendInput(&click, std::mem::size_of::<INPUT>() as i32) == 2,
                "owned activation click denied"
            );
            pump(Duration::from_millis(120));
            SetWindowPos(
                target,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )?;
        }
        if GetForegroundWindow() != target {
            println!("WAIT: activate the window named X Clipboard · own target probe (30 seconds)");
            let deadline = Instant::now() + Duration::from_secs(30);
            while GetForegroundWindow() != target && Instant::now() < deadline {
                pump(Duration::from_millis(10));
            }
        }
        anyhow::ensure!(
            GetForegroundWindow() == target,
            "probe target was not activated within 30 seconds"
        );
        let _ = SetFocus(Some(editor));
        pump(Duration::from_millis(120));
        anyhow::ensure!(
            GetForegroundWindow() == target,
            "probe target is not foreground"
        );
        let saved = platform::foreground();
        let panel = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            if search_focus {
                w!("EDIT")
            } else {
                w!("STATIC")
            },
            w!("X Clipboard · own panel probe"),
            WS_OVERLAPPEDWINDOW,
            180,
            130,
            400,
            180,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let third = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("X Clipboard · own interruption probe"),
            WS_OVERLAPPEDWINDOW,
            230,
            180,
            440,
            200,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let third_editor = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_MULTILINE as u32),
            20,
            30,
            360,
            90,
            Some(if other_control { target } else { third }),
            None,
            Some(instance.into()),
            None,
        )?;
        probe_window::THIRD.store(third.0 as usize, Ordering::Release);
        probe_window::THIRD_EDITOR.store(third_editor.0 as usize, Ordering::Release);
        if preserve_focus {
            platform::show_quick_panel(panel.0 as usize, saved)?;
            if search_focus {
                platform::set_quick_search_mode(panel.0 as usize, true);
                SetWindowTextW(panel, w!(""))?;
                use windows::Win32::UI::Input::KeyboardAndMouse::*;
                for character in "abc 中文".encode_utf16() {
                    let input = [false, true].map(|up| INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wScan: character,
                                dwFlags: KEYEVENTF_UNICODE
                                    | if up {
                                        KEYEVENTF_KEYUP
                                    } else {
                                        KEYBD_EVENT_FLAGS(0)
                                    },
                                ..Default::default()
                            },
                        },
                    });
                    anyhow::ensure!(
                        SendInput(&input, std::mem::size_of::<INPUT>() as i32) == 2,
                        "cannot inject owned-window search probe input"
                    );
                    pump(Duration::from_millis(20));
                }
                let mut text = [0u16; 64];
                let n = GetWindowTextW(panel, &mut text);
                anyhow::ensure!(
                    String::from_utf16_lossy(&text[..n as usize]) == "abc 中文",
                    "nonactivating search did not receive committed text"
                );
                anyhow::ensure!(
                    GetWindowTextLengthW(editor) == 0,
                    "search input leaked into the original target"
                );
            }
        } else {
            let _ = ShowWindow(panel, SW_SHOW);
            let _ = SetForegroundWindow(panel);
            anyhow::ensure!(
                GetForegroundWindow() == panel,
                "probe panel cannot be activated"
            );
            let _ = SetFocus(Some(panel));
        }
        pump(Duration::from_millis(80));
        if preserve_focus {
            anyhow::ensure!(
                !probe_window::LOST_FOCUS.load(Ordering::Acquire)
                    && GetForegroundWindow() == target,
                "FAIL: showing quick panel blurred the original search input"
            );
            let activation = SendMessageW(
                panel,
                WM_MOUSEACTIVATE,
                Some(WPARAM(target.0 as usize)),
                Some(LPARAM(HTCLIENT as isize)),
            );
            anyhow::ensure!(
                activation.0 == MA_NOACTIVATE as isize,
                "FAIL: clicking quick panel would activate it"
            );
            platform::hide_window(panel.0 as usize);
        }
        if destroyed {
            let _ = DestroyWindow(editor);
            probe_window::EDITOR.store(0, Ordering::Release);
        }
        probe_window::ARMED.store(true, Ordering::Release);
        let started = Instant::now();
        let paste_shortcut = if arguments.iter().any(|arg| arg == "--shift-insert") {
            "Shift+Insert"
        } else {
            "Ctrl+V"
        };
        let worker = std::thread::spawn(move || backend.paste_using(saved, paste_shortcut));
        while !worker.is_finished() && started.elapsed() < Duration::from_secs(4) {
            pump(Duration::from_millis(10));
        }
        anyhow::ensure!(worker.is_finished(), "paste worker timed out");
        let result = worker
            .join()
            .map_err(|_| anyhow::anyhow!("paste worker panicked"))?;
        pump(Duration::from_millis(250));
        if preserve_focus {
            anyhow::ensure!(
                !probe_window::LOST_FOCUS.load(Ordering::Acquire),
                "FAIL: paste blurred the original search input"
            );
        }
        let mut text = [0u16; 128];
        let count = if destroyed {
            0
        } else {
            GetWindowTextW(editor, &mut text)
        };
        let actual = String::from_utf16_lossy(&text[..count as usize]);
        let inserted = actual == "xclipboard-own-window-paste-probe";
        let third_count = GetWindowTextW(third_editor, &mut text);
        let third_still_foreground = GetForegroundWindow() == third;
        probe_window::ARMED.store(false, Ordering::Release);
        let _ = DestroyWindow(panel);
        let _ = DestroyWindow(target);
        let _ = DestroyWindow(third);
        let _ = UnregisterClassW(w!("XClipboardPasteProbe"), Some(instance.into()));
        if interrupted || destroyed || other_control {
            let error = result
                .expect_err("paste must stop when the intended input target is lost")
                .to_string();
            anyhow::ensure!(
                if interrupted {
                    error.contains("目标焦点已变化") || error.contains("目标输入框已变化")
                } else if other_control {
                    error.contains("目标输入框已变化")
                } else {
                    error.contains("原输入框已关闭")
                },
                "unexpected failure: {error}"
            );
            anyhow::ensure!(
                !inserted && third_count == 0,
                "input was inserted after its target became invalid"
            );
            anyhow::ensure!(
                !interrupted || third_still_foreground,
                "paste stole focus back after interruption"
            );
            println!(
                "PASS: cancelled without typing (interrupted={interrupted}, destroyed={destroyed}, other_control={other_control})"
            );
            return Ok(());
        }
        result?;
        anyhow::ensure!(
            inserted,
            "FAIL: paste did not insert expected text (terminal={terminal}, actual={actual:?}, deferred_focus={deferred})"
        );
        println!(
            "PASS: actual clipboard text reached original input control (deferred_focus={deferred}, manual_focus={manual_focus}, redirect_first_focus={redirect_focus})"
        );
    }
    Ok(())
}

#[cfg(windows)]
fn pump(duration: std::time::Duration) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    let until = std::time::Instant::now() + duration;
    while std::time::Instant::now() < until {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(windows)]
mod probe_window {
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicUsize, Ordering};
    use windows::Win32::{
        Foundation::*,
        UI::{Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
    };
    pub static TERMINAL: AtomicBool = AtomicBool::new(false);
    pub static EDITOR: AtomicUsize = AtomicUsize::new(0);
    pub static ARMED: AtomicBool = AtomicBool::new(false);
    pub static DELAY: AtomicU32 = AtomicU32::new(0);
    pub static INTERRUPT: AtomicBool = AtomicBool::new(false);
    pub static MANUAL_FOCUS: AtomicBool = AtomicBool::new(false);
    pub static REDIRECT_FIRST_FOCUS: AtomicBool = AtomicBool::new(false);
    pub static OTHER_CONTROL_FOCUS: AtomicBool = AtomicBool::new(false);
    pub static PRESERVE_FOCUS: AtomicBool = AtomicBool::new(false);
    pub static LOST_FOCUS: AtomicBool = AtomicBool::new(false);
    pub static THIRD: AtomicUsize = AtomicUsize::new(0);
    pub static THIRD_EDITOR: AtomicUsize = AtomicUsize::new(0);
    pub static EDITOR_PROCEDURE: AtomicIsize = AtomicIsize::new(0);
    pub unsafe extern "system" fn editor_procedure(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if TERMINAL.load(Ordering::Acquire) {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    GetAsyncKeyState, VK_CONTROL, VK_V,
                };
                if message == WM_KEYDOWN
                    && wparam.0 == VK_V.0 as usize
                    && GetAsyncKeyState(VK_CONTROL.0 as i32) < 0
                {
                    return LRESULT(0);
                }
                if message == WM_KEYDOWN
                    && wparam.0 == windows::Win32::UI::Input::KeyboardAndMouse::VK_INSERT.0 as usize
                    && windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                        windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32,
                    ) < 0
                {
                    return SendMessageW(hwnd, WM_PASTE, None, None);
                }
                if message == WM_CHAR && wparam.0 == 22 {
                    SetWindowTextW(hwnd, windows::core::w!("^V")).unwrap();
                    return LRESULT(0);
                }
            }
            let original: WNDPROC = std::mem::transmute(EDITOR_PROCEDURE.load(Ordering::Acquire));
            let result = CallWindowProcW(original, hwnd, message, wparam, lparam);
            if message == WM_KILLFOCUS && PRESERVE_FOCUS.load(Ordering::Acquire) {
                LOST_FOCUS.store(true, Ordering::Release);
            }
            if message == WM_SETFOCUS
                && ARMED.load(Ordering::Acquire)
                && OTHER_CONTROL_FOCUS.swap(false, Ordering::AcqRel)
            {
                let _ = SetFocus(Some(HWND(THIRD_EDITOR.load(Ordering::Acquire) as *mut _)));
            }
            if message == WM_SETFOCUS
                && ARMED.load(Ordering::Acquire)
                && REDIRECT_FIRST_FOCUS.swap(false, Ordering::AcqRel)
            {
                // Model activation code replacing the first restored editor focus with its root.
                // This mode schedules no timer; only a second backend restoration can recover it.
                let _ = SetFocus(Some(GetAncestor(hwnd, GA_ROOT)));
            }
            if message == WM_SETFOCUS
                && ARMED.load(Ordering::Acquire)
                && INTERRUPT.swap(false, Ordering::AcqRel)
            {
                // Interrupt at the actual focus-restoration seam, not a timer racing SendInput.
                let _ = KillTimer(Some(GetAncestor(hwnd, GA_ROOT)), 1);
                let third = HWND(THIRD.load(Ordering::Acquire) as *mut _);
                let _ = ShowWindow(third, SW_SHOW);
                let _ = SetForegroundWindow(third);
                let _ = SetFocus(Some(HWND(THIRD_EDITOR.load(Ordering::Acquire) as *mut _)));
            }
            result
        }
    }
    pub unsafe extern "system" fn procedure(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if message == WM_ACTIVATE
                && wparam.0 as u16 != WA_INACTIVE as u16
                && ARMED.load(Ordering::Acquire)
            {
                let delay = DELAY.load(Ordering::Acquire);
                if MANUAL_FOCUS.load(Ordering::Acquire) {
                    let _ = SetFocus(Some(hwnd));
                } else if delay == 0 {
                    let _ = SetFocus(Some(HWND(EDITOR.load(Ordering::Acquire) as *mut _)));
                } else {
                    let _ = SetFocus(Some(hwnd));
                    SetTimer(Some(hwnd), 1, delay, None);
                }
                return LRESULT(0);
            }
            if message == WM_TIMER && wparam.0 == 1 {
                let _ = KillTimer(Some(hwnd), 1);
                let editor = EDITOR.load(Ordering::Acquire);
                if editor != 0 {
                    let _ = SetFocus(Some(HWND(editor as *mut _)));
                }
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows native paste probe only");
}
