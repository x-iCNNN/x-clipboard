//! Feature-gated native rendering review. Never sends input to another application.
use super::*;
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
struct Fixture {
    schema: u32,
    synthetic: bool,
    ids: BTreeMap<String, String>,
}

#[derive(Clone)]
struct Scene {
    name: String,
    kind: Kind,
    route: &'static str,
    preview: Option<&'static str>,
    state: &'static str,
    step: usize,
    width: f32,
    height: f32,
}

#[derive(Serialize)]
struct Capture {
    name: String,
    theme: String,
    file: String,
    expected_width: f32,
    expected_height: f32,
    logical_width: f32,
    logical_height: f32,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
    preview_error: Option<String>,
    expected_preview_error: bool,
    visual_review: &'static str,
}

struct Keeper;
impl Render for Keeper {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub(super) fn start(cx: &mut App) {
    let service = cx.global::<Owner>().service.clone();
    // Review runs only with the explicit fixture marker. Refuse normal user databases.
    let fixture = std::fs::read(service.root.join("UI-REVIEW-FIXTURE.json"))
        .context("--ui-review 需要 review_fixture 创建的隔离测试目录")
        .and_then(|bytes| Ok(serde_json::from_slice::<Fixture>(&bytes)?))
        .and_then(|fixture| {
            ensure!(fixture.schema == 1 && fixture.synthetic, "验收目录标记无效");
            Ok(fixture)
        });
    let fixture = match fixture {
        Ok(fixture) => fixture,
        Err(error) => {
            eprintln!("UI-REVIEW ERROR: {error:#}");
            service.send(Command::Shutdown);
            cx.quit();
            return;
        }
    };
    // Prevent a review session from capturing clipboard changes or intercepting hotkeys.
    if std::env::var("X_CLIPBOARD_UI_REVIEW_SEARCH").as_deref() != Ok("1") {
        service.backend.shutdown();
    }
    let output = std::env::var_os("X_CLIPBOARD_UI_REVIEW_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/native-screenshots/polish"));
    cx.spawn(async move |cx| {
        if let Err(error) = run(cx, &service, fixture, &output).await {
            eprintln!("UI-REVIEW ERROR: {error:#}");
            let _ = std::fs::create_dir_all(&output);
            let _ = std::fs::write(output.join("capture-error.txt"), format!("{error:#}"));
        }
        service.send(Command::Shutdown);
        for _ in 0..100 {
            if service.snapshot().shutdown_complete {
                break;
            }
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// Exercises the real GPUI InputState while a disposable native edit window
/// retains foreground/focus. All injected input is confined to these windows.
async fn check_search_input(cx: &mut AsyncApp, output: &std::path::Path) -> Result<()> {
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
        core::w,
    };
    unsafe extern "system" fn target_proc(
        hwnd: HWND,
        message: u32,
        w: windows::Win32::Foundation::WPARAM,
        l: windows::Win32::Foundation::LPARAM,
    ) -> windows::Win32::Foundation::LRESULT {
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }
    let (target, editor) = unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(target_proc),
            lpszClassName: w!("XClipboardSearchProbeTarget"),
            ..Default::default()
        };
        ensure!(
            RegisterClassW(&class) != 0,
            "cannot register owned search target"
        );
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("XClipboardSearchProbeTarget"),
            w!("Search probe"),
            WS_OVERLAPPEDWINDOW,
            40,
            40,
            480,
            180,
            None,
            None,
            None,
            None,
        )?;
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            10,
            10,
            400,
            80,
            Some(hwnd),
            None,
            None,
            None,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        // Start-Process -WindowStyle Hidden applies to the first ShowWindow call.
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(edit));
        (hwnd.0 as usize, edit.0 as usize)
    };
    struct Target(usize);
    impl Drop for Target {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(HWND(self.0 as *mut _));
            }
        }
    }
    let _target = Target(target);
    // Windows may deny programmatic activation of a background review process.
    // A real click is confined to our own verified target, never another app.
    if unsafe { GetForegroundWindow().0 as usize } != target {
        use windows::Win32::Foundation::{POINT, RECT};
        unsafe {
            let hwnd = HWND(target as *mut _);
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect)?;
            let point = POINT {
                x: rect.left + 50,
                y: rect.top + 60,
            };
            ensure!(
                GetAncestor(WindowFromPoint(point), GA_ROOT) == hwnd,
                "owned search target is obscured; refuse to click"
            );
            SetCursorPos(point.x, point.y)?;
            let clicks = [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP].map(|flags| INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            });
            ensure!(
                SendInput(&clicks, std::mem::size_of::<INPUT>() as i32) == 2,
                "owned search target click was denied"
            );
        }
        cx.background_executor()
            .timer(Duration::from_millis(120))
            .await;
    }
    ensure!(
        unsafe { GetForegroundWindow().0 as usize } == target,
        "owned native target could not become foreground"
    );
    cx.update(|cx| {
        cx.global_mut::<Owner>().target = platform::foreground();
        open_window(Kind::Quick, cx);
    });
    cx.background_executor()
        .timer(Duration::from_millis(500))
        .await;
    let (handle, view) = cx
        .update(|cx| cx.global::<Owner>().windows.get(&Kind::Quick).cloned())
        .context("quick window was not opened")?;
    cx.update(|cx| {
        handle.update(cx, |_, window, _| -> Result<()> {
            ensure!(
                (f32::from(window.viewport_size().width) - QUICK_WIDTH).abs() <= 1.,
                "nonactivating quick panel has wrong initial width"
            );
            Ok(())
        })
    })??;
    ensure!(
        unsafe { GetForegroundWindow().0 as usize } == target,
        "opening the quick panel activated it"
    );
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |desktop, cx| {
                desktop
                    .search
                    .update(cx, |input, cx| input.focus(window, cx));
            });
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    ensure!(
        unsafe { GetForegroundWindow().0 as usize } == target,
        "focusing GPUI search activated the panel"
    );
    let send = |key: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS| -> Result<()> {
        let pair = [false, true].map(|up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: scan,
                    dwFlags: flags
                        | if up {
                            KEYEVENTF_KEYUP
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                    ..Default::default()
                },
            },
        });
        ensure!(
            unsafe { SendInput(&pair, std::mem::size_of::<INPUT>() as i32) } == 2,
            "owned search probe input was denied"
        );
        Ok(())
    };
    // Physical Latin keys exercise the native accelerator/TranslateMessage path.
    for key in [VK_A, VK_B, VK_C, VK_SPACE] {
        send(key, 0, KEYBD_EVENT_FLAGS(0))?;
        cx.background_executor()
            .timer(Duration::from_millis(35))
            .await;
    }
    // Unicode commits exercise UTF-16 text entry independently of a specific IME.
    for unit in "中文".encode_utf16() {
        send(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE)?;
        cx.background_executor()
            .timer(Duration::from_millis(35))
            .await;
    }
    send(VK_BACK, 0, KEYBD_EVENT_FLAGS(0))?;
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    let value = cx.update(|cx| view.read(cx).search.read(cx).value().to_string());
    ensure!(
        value.to_lowercase() == "abc 中",
        "GPUI search edit mismatch: {value:?}"
    );
    unsafe {
        ensure!(
            GetForegroundWindow().0 as usize == target && GetFocus().0 as usize == editor,
            "GPUI search stole focus from the original input"
        );
        ensure!(
            GetWindowTextLengthW(HWND(editor as *mut _)) == 0,
            "search keys leaked to target"
        );
    }
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |desktop, cx| {
                desktop
                    .search
                    .update(cx, |input, cx| input.set_value("", window, cx));
            })
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    send(VK_Z, 0, KEYBD_EVENT_FLAGS(0))?;
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    ensure!(
        cx.update(|cx| view.read(cx).search.read(cx).value() == "z"),
        "search was not editable after clearing"
    );
    for up in [false, true] {
        let modifier = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    ..Default::default()
                },
            },
        };
        ensure!(
            unsafe { SendInput(&[modifier], std::mem::size_of::<INPUT>() as i32) } == 1,
            "owned search modifier input was denied"
        );
        cx.background_executor()
            .timer(Duration::from_millis(35))
            .await;
        if !up {
            send(VK_A, 0, KEYBD_EVENT_FLAGS(0))?;
            cx.background_executor()
                .timer(Duration::from_millis(35))
                .await;
        }
    }
    send(VIRTUAL_KEY(0), '重' as u16, KEYEVENTF_UNICODE)?;
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    ensure!(
        cx.update(|cx| view.read(cx).search.read(cx).value() == "重"),
        "Ctrl+A did not select the search text"
    );
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |desktop, cx| {
                if let Some(entry) = desktop.snapshot.entries.first() {
                    desktop.open_preview(entry.id.clone(), window, cx);
                }
            })
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    ensure!(
        unsafe { GetForegroundWindow().0 as usize } == target,
        "preview expansion activated quick panel"
    );
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |desktop, cx| {
                desktop.close_preview(window, cx);
            })
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    ensure!(
        unsafe { GetForegroundWindow().0 as usize } == target,
        "preview collapse activated quick panel"
    );
    cx.update(|cx| handle.update(cx, |_, window, cx| hide(window, cx)))?;
    std::fs::write(
        output.join("search-input-result.json"),
        r#"{"passed":true,"native_focus_preserved":true,"latin_and_unicode_commit":true,"backspace":true,"clear_then_edit":true,"physical_ime_composition":"not_verified"}"#,
    )?;
    println!("UI-REVIEW SEARCH PASS: real GPUI input, focus preserved, text/edit/clear verified");
    Ok(())
}

async fn run(
    cx: &mut AsyncApp,
    service: &Service,
    fixture: Fixture,
    output: &std::path::Path,
) -> Result<()> {
    std::fs::create_dir_all(output)?;
    service.set_history_active(true);
    for _ in 0..200 {
        if service.snapshot().history_loaded {
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    ensure!(
        service.snapshot().history_loaded,
        "history hydration timed out"
    );
    if let Ok(endpoint) = std::env::var("X_CLIPBOARD_UI_REVIEW_WEBDAV") {
        return check_webdav(cx, service, output, &endpoint).await;
    }
    if std::env::var("X_CLIPBOARD_UI_REVIEW_SEARCH").as_deref() == Ok("1") {
        return check_search_input(cx, output).await;
    }
    if std::env::var("X_CLIPBOARD_UI_REVIEW_LANGUAGE_SWITCH").as_deref() == Ok("1") {
        let original = service.snapshot().settings;
        cx.update(|cx| {
            for kind in [Kind::Main, Kind::Settings, Kind::Welcome] {
                open_window(kind, cx);
            }
        });
        lifecycle_tick(cx, 250).await;
        for &(language, _) in i18n::LANGUAGES {
            let mut settings = service.snapshot().settings;
            settings.language = language.into();
            service.send(Command::Settings(settings));
            lifecycle_tick(cx, 200).await;
            cx.update(|cx| -> Result<()> {
                ensure!(
                    service.snapshot().settings.language == language,
                    "language was not saved"
                );
                for kind in [Kind::Main, Kind::Settings, Kind::Welcome] {
                    let (_, view) = cx
                        .global::<Owner>()
                        .windows
                        .get(&kind)
                        .context("language window missing")?;
                    ensure!(
                        view.read(cx).snapshot.settings.language == language,
                        "open window did not update its language"
                    );
                }
                Ok(())
            })?;
        }
        service.send(Command::Settings(original));
        lifecycle_tick(cx, 200).await;
        std::fs::write(
            output.join("language-switch-result.json"),
            r#"{"passed":true,"languages":9,"existing_windows_updated":3,"restart_persistence":"covered_by_encrypted_store_test"}"#,
        )?;
        println!("UI-REVIEW LANGUAGE PASS: nine languages updated all three existing windows");
        return Ok(());
    }
    if std::env::var("X_CLIPBOARD_UI_REVIEW_DORMANCY").as_deref() == Ok("1") {
        return check_dormancy(cx, service, output).await;
    }
    // Keep legacy screenshot scenes separate from the production zero-window
    // lifecycle probe above. Production uses QuitMode::Explicit.
    let _keeper = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: false,
                focus: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(320.), px(240.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Keeper),
        )
    })?;
    if std::env::var("X_CLIPBOARD_UI_REVIEW_INITIAL_SIZE").as_deref() == Ok("1") {
        return check_initial_quick_size(cx, output).await;
    }
    if std::env::var("X_CLIPBOARD_UI_REVIEW_SINGLETON").as_deref() == Ok("1") {
        return check_window_singletons(cx, output).await;
    }
    let mut base = service.snapshot();
    base.entries = Arc::new(
        base.entries
            .iter()
            .filter(|entry| fixture.ids.values().any(|id| id == &entry.id))
            .cloned()
            .collect(),
    );
    ensure!(
        base.entries.len() == fixture.ids.len(),
        "合成记录未完整加载"
    );
    base.settings.paused = false;
    base.settings.shortcut = Settings::default().shortcut;
    if let Ok(language) = std::env::var("X_CLIPBOARD_UI_REVIEW_LANGUAGE") {
        ensure!(i18n::supported(&language), "unsupported review language");
        base.settings.language = language;
    }
    cx.update(|_| apply_language(&base.settings.language));
    base.status = "正在记录 · 已保存".into();
    base.error = None;
    base.preview = None;
    let mut previews = BTreeMap::new();
    for (name, id) in &fixture.ids {
        service.preview(id.clone(), 0);
        let mut generated = None;
        for _ in 0..200 {
            let snapshot = service.snapshot();
            if let Some(preview) = snapshot.preview.filter(|preview| preview.id == *id) {
                generated = Some(preview);
                break;
            }
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
        }
        previews.insert(
            name.clone(),
            generated.with_context(|| format!("预览超时：{name}"))?,
        );
    }
    let mut captures = vec![];
    for entry in base.entries.iter().filter(|entry| entry.kind == "image") {
        service.thumbnail(&entry.id);
    }
    for _ in 0..100 {
        let snapshot = service.snapshot();
        if base
            .entries
            .iter()
            .filter(|entry| entry.kind == "image")
            .all(|entry| snapshot.thumbnails.contains_key(&entry.id))
        {
            base.thumbnails = snapshot.thumbnails;
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(30))
            .await;
    }
    let mut scenes = scenes();
    scenes.push(Scene {
        name: "file-type-marks".into(),
        kind: Kind::Main,
        route: "all",
        preview: Some("folder"),
        state: "file-marks",
        step: 0,
        width: 1120.,
        height: 760.,
    });
    scenes.push(Scene {
        name: "settings-general-switches".into(),
        kind: Kind::Settings,
        route: "general",
        preview: None,
        state: "switches",
        step: 0,
        width: 960.,
        height: 760.,
    });
    for (name, state) in [("tray-menu", "tray"), ("tray-menu-paused", "tray-paused")] {
        scenes.push(Scene {
            name: name.into(),
            kind: Kind::Tray,
            route: "all",
            preview: None,
            state,
            step: 0,
            width: 224.,
            height: 200.,
        });
    }
    for (name, route) in [
        ("settings-window-menu", "general"),
        ("about-window-menu", "about"),
    ] {
        scenes.push(Scene {
            name: name.into(),
            kind: Kind::Settings,
            route,
            preview: None,
            state: "menu",
            step: 0,
            width: 960.,
            height: 760.,
        });
    }
    if let Ok(filter) = std::env::var("X_CLIPBOARD_UI_REVIEW_SCENES") {
        scenes.retain(|scene| filter.split(',').any(|name| name.trim() == scene.name));
    }
    let first_scene = scenes.first().context("没有可渲染的验收场景")?;
    let (handle, view) = cx.update(|cx| {
        apply_theme("light", cx);
        create_scene(
            cx,
            service.clone(),
            first_scene.clone(),
            base.clone(),
            None,
            None,
            "light",
        )
    })?;
    let mut themes = vec!["light", "dark"];
    if std::env::var("X_CLIPBOARD_UI_REVIEW_THEMES").as_deref() == Ok("1") {
        themes.extend(x_clipboard::themes::THEME_NAMES.iter().copied());
    }
    for theme in themes {
        cx.update(|cx| -> Result<()> {
            apply_theme(theme, cx);
            if theme != "light" && theme != "dark" {
                ensure!(
                    cx.theme().theme_name().as_ref() == theme,
                    "主题未实际生效：{theme}"
                );
            }
            Ok(())
        })?;
        for scene in &scenes {
            if theme != "light"
                && theme != "dark"
                && !["quick", "tray-menu"].contains(&scene.name.as_str())
            {
                continue;
            }
            let snapshot = base.clone();
            let preview = scene.preview.and_then(|name| previews.get(name)).cloned();
            let selected = scene
                .preview
                .and_then(|name| fixture.ids.get(name))
                .cloned();
            cx.update(|cx| {
                handle.update(cx, |_, window, cx| {
                    view.update(cx, |desktop, cx| {
                        configure(
                            desktop, scene, snapshot, selected, preview, theme, window, cx,
                        );
                        cx.notify();
                    });
                })
            })?;
            settle_size(cx, handle, scene).await?;
            // Two actual GPUI draws allow text shaping, image upload, and deferred popovers to settle.
            cx.update(|cx| {
                handle.update(cx, |_, window, cx| {
                    window.draw(cx).clear(cx);
                })
            })?;
            cx.background_executor()
                .timer(Duration::from_millis(220))
                .await;
            let filename = format!("{}-{theme}.png", scene.name);
            let path = output.join(&filename);
            let capture = cx.update(|cx| {
                handle.update(cx, |_, window, cx| -> Result<Capture> {
                    window.draw(cx).clear(cx);
                    verify_size(window, scene)?;
                    let image = window.render_to_image()?;
                    let scale = window.scale_factor();
                    ensure!(
                        (image.width() as f32 - scene.width * scale).abs() <= 1.
                            && (image.height() as f32 - scene.height * scale).abs() <= 1.,
                        "截图渲染尺寸异常：{filename} 实际 {}×{}，期望逻辑 {}×{}，缩放 {scale}",
                        image.width(),
                        image.height(),
                        scene.width,
                        scene.height
                    );
                    image.save(&path)?;
                    Ok(Capture {
                        name: scene.name.clone(),
                        theme: theme.into(),
                        file: filename.clone(),
                        expected_width: scene.width,
                        expected_height: scene.height,
                        logical_width: f32::from(window.viewport_size().width),
                        logical_height: f32::from(window.viewport_size().height),
                        pixel_width: image.width(),
                        pixel_height: image.height(),
                        scale_factor: scale,
                        preview_error: view
                            .read(cx)
                            .snapshot
                            .preview
                            .as_ref()
                            .and_then(|p| p.error.clone()),
                        expected_preview_error: matches!(
                            scene.preview,
                            Some("missing" | "preview-error")
                        ),
                        visual_review: "pending-human-or-agent-image-inspection",
                    })
                })
            })??;
            captures.push(capture);
            println!("UI-REVIEW rendered {filename}");
        }
    }
    let manifest = serde_json::json!({
        "schema": 1,
        "renderer": "GPUI native render_to_image",
        "fixture": "synthetic encrypted database",
        "visual_acceptance": "NOT_PROVEN_BY_CAPTURE_EXISTENCE",
        "count": captures.len(),
        "captures": captures,
    });
    std::fs::write(
        output.join("capture-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "UI-REVIEW COMPLETE: {} native captures; visual inspection remains required",
        captures.len()
    );
    Ok(())
}

async fn check_initial_quick_size(cx: &mut AsyncApp, output: &std::path::Path) -> Result<()> {
    // Use the production creation/show path, with no test resize or settle_size call.
    cx.update(|cx| open_window(Kind::Quick, cx));
    cx.background_executor()
        .timer(Duration::from_millis(250))
        .await;
    let (handle, view) = cx.update(|cx| cx.global::<Owner>().windows[&Kind::Quick].clone());
    let first = cx.update(|cx| {
        handle.update(cx, |_, window, cx| -> Result<_> {
            window.draw(cx).clear(cx);
            window
                .render_to_image()?
                .save(output.join("first-open.png"))?;
            Ok((
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            ))
        })
    })??;
    let id = cx
        .update(|cx| {
            view.read(cx)
                .snapshot
                .entries
                .iter()
                .find(|entry| entry.kind == "image")
                .map(|entry| entry.id.clone())
        })
        .context("probe fixture has no entry")?;
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.open_preview(id, window, cx))
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(200))
        .await;
    let preview_image = cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.sync(window, cx));
            window.draw(cx).clear(cx);
            view.read(cx).image.clone()
        })
    })?;
    ensure!(
        preview_image.is_some(),
        "image preview did not finish loading"
    );
    cx.background_executor()
        .timer(Duration::from_millis(220))
        .await;
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| -> Result<_> {
            window.draw(cx).clear(cx);
            window
                .render_to_image()?
                .save(output.join("image-before-close.png"))?;
            Ok(())
        })
    })??;
    let preview_image_weak = Arc::downgrade(preview_image.as_ref().unwrap());
    drop(preview_image);
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.close_preview(window, cx))
        })
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(200))
        .await;
    let closed = cx.update(|cx| {
        handle.update(cx, |_, window, cx| -> Result<_> {
            ensure!(
                view.read(cx).image.is_none(),
                "closed preview retained its CPU image"
            );
            ensure!(
                view.read(cx).snapshot.preview.is_none(),
                "closed preview retained decoded data"
            );
            window.draw(cx).clear(cx);
            Ok((
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            ))
        })
    })??;
    ensure!(
        preview_image_weak.upgrade().is_none(),
        "closed preview retained a render image reference"
    );
    std::fs::write(
        output.join("initial-size-result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"first":first,"after_preview_close":closed}),
        )?,
    )?;
    ensure!(
        (first.0 - QUICK_WIDTH).abs() <= 1. && (first.1 - QUICK_HEIGHT).abs() <= 1.,
        "first quick panel was {}x{}, expected {}x{}; after preview close: {}x{}",
        first.0,
        first.1,
        QUICK_WIDTH,
        QUICK_HEIGHT,
        closed.0,
        closed.1
    );
    ensure!(
        (closed.0 - QUICK_WIDTH).abs() <= 1. && (closed.1 - QUICK_HEIGHT).abs() <= 1.,
        "preview close size differs"
    );
    println!(
        "INITIAL-SIZE PASS: first open and preview close are {}x{}",
        QUICK_WIDTH, QUICK_HEIGHT
    );
    println!(
        "PREVIEW-MEMORY PASS: closing the preview released decoded data and render image references"
    );
    Ok(())
}

fn process_memory() -> Result<serde_json::Value> {
    use ::windows::Win32::System::{ProcessStatus::*, Threading::GetCurrentProcess};
    let mut memory = PROCESS_MEMORY_COUNTERS_EX::default();
    memory.cb = std::mem::size_of_val(&memory) as u32;
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            memory.cb,
        )?;
    }
    Ok(
        serde_json::json!({"working_set_bytes":memory.WorkingSetSize,"private_bytes":memory.PrivateUsage}),
    )
}
async fn lifecycle_tick(cx: &mut AsyncApp, milliseconds: u64) {
    for _ in 0..milliseconds.div_ceil(60) {
        cx.background_executor()
            .timer(Duration::from_millis(60))
            .await;
        cx.update(poll);
    }
}
async fn check_webdav(
    cx: &mut AsyncApp,
    service: &Service,
    output: &std::path::Path,
    endpoint: &str,
) -> Result<()> {
    let base = url::Url::parse(endpoint)?;
    ensure!(
        matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
        "WebDAV UI review requires loopback fixture"
    );
    let endpoint = base
        .join(&format!("native-{}/", uuid::Uuid::new_v4()))?
        .to_string();
    let create = endpoint.clone();
    cx.background_executor()
        .spawn(async move {
            reqwest::blocking::Client::builder()
                .no_proxy()
                .build()?
                .request("MKCOL".parse()?, create)
                .basic_auth("sync-test", Some("fixture-login"))
                .send()?
                .error_for_status()?;
            anyhow::Ok(())
        })
        .await?;
    cx.update(|cx| open_window(Kind::Settings, cx));
    lifecycle_tick(cx, 300).await;
    let (handle, view) = cx.update(|cx| cx.global::<Owner>().windows[&Kind::Settings].clone());
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            window.resize(size(px(960.), px(1020.)));
            view.update(cx, |desktop, cx| {
                desktop.route = "sync";
                desktop
                    .sync_endpoint
                    .update(cx, |input, cx| input.set_value(endpoint, window, cx));
                desktop
                    .sync_username
                    .update(cx, |input, cx| input.set_value("sync-test", window, cx));
                desktop
                    .sync_password
                    .update(cx, |input, cx| input.set_value("fixture-login", window, cx));
                desktop
                    .sync_secret
                    .update(cx, |input, cx| input.set_value("short", window, cx));
                desktop.save_webdav(window, cx);
            });
        })
    })?;
    ensure!(
        service
            .snapshot()
            .error
            .as_deref()
            .is_some_and(|e| e.contains("12")),
        "Native handler accepted a short secret"
    );
    service.dismiss_error();
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            view.update(cx, |desktop, cx| {
                desktop.sync_secret.update(cx, |input, cx| {
                    input.set_value("native fixture shared secret", window, cx)
                });
                desktop.save_webdav(window, cx);
                ensure!(
                    desktop.sync_password.read(cx).value().is_empty()
                        && desktop.sync_secret.read(cx).value().is_empty(),
                    "Native form retained submitted secrets"
                );
                anyhow::Ok(())
            })
        })
    })??;
    for _ in 0..300 {
        lifecycle_tick(cx, 100).await;
        let status = service.snapshot().sync;
        if status.message.starts_with("同步失败") {
            anyhow::bail!(status.message);
        }
        if status.last_success.is_some() && !status.busy {
            break;
        }
    }
    ensure!(
        service.snapshot().sync.last_success.is_some(),
        "Native WebDAV configuration did not synchronize"
    );
    for theme in ["light", "dark"] {
        cx.update(|cx| apply_theme(theme, cx));
        lifecycle_tick(cx, 250).await;
        cx.update(|cx| {
            handle.update(cx, |_, window, cx| -> Result<()> {
                window.draw(cx).clear(cx);
                window
                    .render_to_image()?
                    .save(output.join(format!("webdav-connected-{theme}.png")))?;
                Ok(())
            })
        })??;
    }
    service.send(Command::Sync(x_clipboard::sync::Request::Enabled(false)));
    for _ in 0..100 {
        lifecycle_tick(cx, 50).await;
        if !service.snapshot().sync.enabled {
            break;
        }
    }
    ensure!(
        !service.snapshot().sync.enabled,
        "Native automatic sync toggle did not persist"
    );
    let before = service.snapshot().sync.last_success;
    service.send(Command::Sync(x_clipboard::sync::Request::Now));
    for _ in 0..300 {
        lifecycle_tick(cx, 100).await;
        if service.snapshot().sync.last_success > before {
            break;
        }
    }
    ensure!(
        service.snapshot().sync.last_success > before,
        "Native manual sync failed while automatic sync disabled"
    );
    std::fs::write(
        output.join("webdav-native-result.json"),
        r#"{"passed":true,"short_secret_rejected":true,"submitted_secrets_cleared":true,"real_webdav_roundtrip":true,"automatic_toggle":true,"manual_while_disabled":true}"#,
    )?;
    println!(
        "UI-REVIEW WEBDAV PASS: native form, secret validation, background sync, auto toggle, manual sync"
    );
    Ok(())
}

async fn check_dormancy(
    cx: &mut AsyncApp,
    service: &Service,
    output: &std::path::Path,
) -> Result<()> {
    let initial = service.snapshot().usage;
    let before = process_memory()?;
    for kind in [Kind::Main, Kind::Settings, Kind::Quick] {
        cx.update(|cx| open_window(kind, cx));
        lifecycle_tick(cx, 300).await;
    }
    let image_id = service
        .snapshot()
        .entries
        .iter()
        .find(|entry| entry.kind == "image")
        .context("fixture image missing")?
        .id
        .clone();
    for kind in [Kind::Main, Kind::Quick] {
        cx.update(|cx| {
            let (handle, view) = cx.global::<Owner>().windows[&kind].clone();
            handle.update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.open_preview(image_id.clone(), window, cx)
                })
            })
        })?;
        lifecycle_tick(cx, 400).await;
    }
    cx.update(|cx| {
        let (handle, _) = cx.global::<Owner>().windows[&Kind::Quick].clone();
        handle.update(cx, |_, window, cx| -> Result<()> {
            window.draw(cx).clear(cx);
            window
                .render_to_image()?
                .save(output.join("warm-quick.png"))?;
            Ok(())
        })
    })??;
    let warm = process_memory()?;
    // A rapid reopen cancels retirement and still reuses the same singleton.
    let quick = cx.update(|cx| cx.global::<Owner>().windows[&Kind::Quick].0);
    cx.update(|cx| quick.update(cx, |_, window, cx| hide(window, cx)))?;
    cx.update(|cx| open_window(Kind::Quick, cx));
    lifecycle_tick(cx, 180).await;
    ensure!(
        cx.update(|cx| cx.global::<Owner>().windows[&Kind::Quick].0) == quick,
        "rapid reopen duplicated quick panel"
    );
    let weak_views = cx.update(|cx| {
        let pairs: Vec<_> = cx.global::<Owner>().windows.values().cloned().collect();
        let weak: Vec<_> = pairs.iter().map(|(_, view)| view.downgrade()).collect();
        for (handle, _) in pairs {
            handle.update(cx, |_, window, cx| hide(window, cx))?;
        }
        anyhow::Ok(weak)
    })?;
    lifecycle_tick(cx, 2100).await;
    ensure!(
        cx.update(|cx| cx.global::<Owner>().windows.is_empty() && cx.windows().is_empty()),
        "closed windows are still retained"
    );
    ensure!(
        cx.update(|_| weak_views.iter().all(|view| view.upgrade().is_none())),
        "closed Desktop entities are still retained"
    );
    let idle = service.snapshot();
    ensure!(
        !idle.history_loaded
            && idle.entries.is_empty()
            && idle.preview.is_none()
            && idle.thumbnails.is_empty(),
        "dormant UI data is still retained"
    );
    drop(idle);
    let dormant = process_memory()?;
    let mut settings = service.snapshot().settings;
    let original_settings = settings.clone();
    settings.paused = false;
    service.send(Command::Settings(settings));
    service.send(Command::Capture(x_clipboard::model::ClipContent::text(
        format!("synthetic dormant capture {}", uuid::Uuid::new_v4()),
    )));
    lifecycle_tick(cx, 300).await;
    ensure!(
        service.snapshot().usage.entries == initial.entries + 1
            && service.snapshot().entries.is_empty(),
        "background capture failed or hydrated history"
    );
    for _ in 0..3 {
        cx.update(|cx| open_window(Kind::Quick, cx));
        lifecycle_tick(cx, 400).await;
        ensure!(
            service.snapshot().history_loaded
                && service.snapshot().entries.len() == initial.entries + 1,
            "reopen did not hydrate latest history"
        );
        cx.update(|cx| {
            let handle = cx.global::<Owner>().windows[&Kind::Quick].0;
            handle.update(cx, |_, window, cx| -> Result<()> {
                ensure!(
                    (f32::from(window.viewport_size().width) - QUICK_WIDTH).abs() <= 1.,
                    "recreated panel has wrong width"
                );
                hide(window, cx);
                Ok(())
            })
        })??;
        lifecycle_tick(cx, 1500).await;
        ensure!(
            cx.update(|cx| cx.windows().is_empty()),
            "recreated panel did not retire"
        );
    }
    service.send(Command::Settings(original_settings));
    let final_idle = process_memory()?;
    std::fs::write(
        output.join("dormancy-result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"before_windows":before,"windows_and_image_previews":warm,"all_windows_closed":dormant,"after_three_reopen_cycles":final_idle,"native_windows_after_close":0,"desktop_entities_after_close":0,"history_rows_after_close":0,"background_capture":true,"rapid_reopen_singleton":true,"recreated_panel_width":QUICK_WIDTH}),
        )?,
    )?;
    println!(
        "DORMANCY PASS: zero native windows and Desktop entities; history unloaded; background capture and three reopen cycles passed"
    );
    Ok(())
}

async fn check_window_singletons(cx: &mut AsyncApp, output: &std::path::Path) -> Result<()> {
    let mut results = Vec::new();
    for kind in [
        Kind::Main,
        Kind::Quick,
        Kind::Settings,
        Kind::Welcome,
        Kind::Tray,
    ] {
        cx.update(|cx| open_window(kind, cx));
        cx.background_executor()
            .timer(Duration::from_millis(150))
            .await;
        let handle = cx.update(|cx| cx.global::<Owner>().windows[&kind].0);
        let before = cx.update(|cx| handle.update(cx, |_, window, _| native(window)))?;
        // Repeat from inside that window's callback, as its own navigation menu does.
        cx.update(|cx| handle.update(cx, |_, _, cx| open_window(kind, cx)))?;
        cx.background_executor()
            .timer(Duration::from_millis(150))
            .await;
        let after_handle = cx.update(|cx| cx.global::<Owner>().windows[&kind].0);
        let after = cx.update(|cx| after_handle.update(cx, |_, window, _| native(window)))?;
        results.push(serde_json::json!({"kind":format!("{kind:?}"),"before":before,"after":after}));
        std::fs::write(
            output.join("singleton-result.json"),
            serde_json::to_vec_pretty(&results)?,
        )?;
        ensure!(
            before == after,
            "navigation created a duplicate {kind:?} window: {before} -> {after}"
        );
    }
    println!("SINGLETON PASS: all five window types reused the original native handle");
    Ok(())
}

async fn settle_size(cx: &mut AsyncApp, handle: AnyWindowHandle, scene: &Scene) -> Result<()> {
    cx.update(|cx| {
        handle.update(cx, |_, window, _| {
            window.resize(size(px(scene.width), px(scene.height)));
        })
    })?;
    // Windows applies resize through its executor; let WM_SIZE update GPUI's
    // drawable viewport before drawing or reading back the render target.
    for _ in 0..60 {
        cx.background_executor()
            .timer(Duration::from_millis(30))
            .await;
        let matched =
            cx.update(|cx| handle.update(cx, |_, window, _| verify_size(window, scene).is_ok()))?;
        if matched {
            return Ok(());
        }
    }
    cx.update(|cx| handle.update(cx, |_, window, _| verify_size(window, scene)))??;
    Ok(())
}

fn verify_size(window: &Window, scene: &Scene) -> Result<()> {
    let size = window.viewport_size();
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    // A fractional DPI factor can round one physical pixel in the client area.
    let tolerance = 1. / window.scale_factor();
    ensure!(
        (width - scene.width).abs() <= tolerance && (height - scene.height).abs() <= tolerance,
        "窗口逻辑尺寸异常：{} 实际 {width}×{height}，期望 {}×{}",
        scene.name,
        scene.width,
        scene.height
    );
    Ok(())
}

// Keep window creation and its sizeable native view initializer out of the async poll frame.
#[inline(never)]
fn create_scene(
    cx: &mut App,
    service: Service,
    scene: Scene,
    snapshot: Snapshot,
    selected: Option<String>,
    preview: Option<Arc<x_clipboard::preview::Preview>>,
    theme: &'static str,
) -> Result<(AnyWindowHandle, Entity<Desktop>)> {
    gpui_kit::open_window(
        WindowOptions {
            show: false,
            focus: false,
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(scene.width), px(scene.height)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            cx.new(|cx| {
                let mut desktop = Desktop::new(scene.kind, service, window, cx);
                configure(
                    &mut desktop,
                    &scene,
                    snapshot,
                    selected,
                    preview,
                    theme,
                    window,
                    cx,
                );
                desktop
            })
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn configure(
    desktop: &mut Desktop,
    scene: &Scene,
    mut snapshot: Snapshot,
    selected: Option<String>,
    preview: Option<Arc<x_clipboard::preview::Preview>>,
    theme: &str,
    window: &mut Window,
    cx: &mut Context<Desktop>,
) {
    // Reuse one HWND so pending UI Automation events never address a destroyed
    // window. Reset view-local state before assigning the next synthetic scene.
    desktop.kind = scene.kind;
    desktop.tray_selected = None;
    desktop.checked.clear();
    desktop.image = None;
    desktop.zoom = 0.;
    desktop.preview_key.clear();
    desktop.history_scroll = ListState::new(0, ListAlignment::Top, px(300.));
    desktop.history_signature.clear();
    desktop
        .editor
        .update(cx, |editor, cx| editor.set_value("", window, cx));
    desktop.route = scene.route;
    desktop.step = scene.step;
    desktop.filter = "all";
    desktop.selected = selected;
    desktop.preview_open = scene.kind == Kind::Quick && scene.preview.is_some();
    desktop.menu = scene.state == "menu";
    desktop.multi = scene.state == "multi";
    desktop.confirm = match scene.state {
        "delete" => Some(false),
        "clear" | "clear-protected" => Some(true),
        _ => None,
    };
    desktop.clear_protected = scene.state == "clear-protected";
    desktop.more = if scene.state == "more" {
        desktop.selected.clone()
    } else {
        None
    };
    desktop.recording_shortcut = scene.state == "recording-shortcut";
    snapshot.settings.theme = theme.into();
    apply_language(&snapshot.settings.language);
    snapshot.settings.compact = scene.state == "compact";
    snapshot.settings.paused = matches!(scene.state, "paused" | "tray-paused");
    if scene.state == "switches" {
        snapshot.settings.start_at_login = true;
    }
    if scene.state == "paused" {
        snapshot.status = "已暂停记录".into();
    }
    if scene.state == "saving" {
        snapshot.status = "保存中…".into();
    }
    if scene.state == "protected-capacity" {
        snapshot.status = "正在记录 · 置顶/收藏已超过容量限制".into();
        snapshot.settings.max_entries = 1;
    }
    if scene.state == "empty" {
        snapshot.entries = Arc::new(vec![]);
        desktop.selected = None;
    }
    if scene.state == "pending-preview" {
        snapshot.preview = None;
    } else {
        snapshot.preview = preview;
    }
    if scene.state == "file-marks" {
        let template = snapshot
            .entries
            .iter()
            .find(|entry| entry.kind == "file")
            .unwrap()
            .clone();
        let mut entries = Vec::new();
        for (index, title) in [
            "26-09-周报.png",
            "钉钉订阅汇总表_26.xlsx",
            "DeepSeek Harness.lnk",
            "README",
        ]
        .into_iter()
        .enumerate()
        {
            let mut entry = template.clone();
            entry.id = format!("synthetic-file-icon-{index}");
            entry.title = title.into();
            entry.text.clear();
            entry.pinned = false;
            entry.favorite = false;
            entry.source = "explorer.exe".into();
            entries.push(entry);
        }
        if let Some(folder) = snapshot
            .entries
            .iter()
            .find(|entry| Some(&entry.id) == desktop.selected.as_ref())
        {
            entries.push(folder.clone());
        }
        snapshot.entries = Arc::new(entries);
    }
    snapshot.error = match scene.state {
        "storage-error" => Some("保存失败：本地存储暂不可用，请检查剩余空间后重试。".into()),
        "key-unavailable" => {
            snapshot.ready = false;
            Some("密钥不可用：请解锁系统密钥库后重试。现有加密数据保持不变。".into())
        }
        "permission" => {
            Some("内容已复制，目标应用权限不足，自动粘贴被系统拒绝；请手动粘贴。".into())
        }
        "shortcut-conflict" => Some("快捷键已被其他应用占用，请选择另一组按键。".into()),
        _ => None,
    };
    if scene.state == "multi" || scene.state == "delete" {
        desktop.checked = snapshot
            .entries
            .iter()
            .take(if scene.state == "multi" { 3 } else { 1 })
            .map(|entry| entry.id.clone())
            .collect();
    }
    let query = match scene.state {
        "search" => "品牌",
        "no-results" => "找不到的验收关键词",
        _ => "",
    };
    desktop
        .search
        .update(cx, |input, cx| input.set_value(query, window, cx));
    snapshot.search_query = query.into();
    snapshot.search_ids = Arc::new(
        snapshot
            .entries
            .iter()
            .filter(|entry| entry.title.contains(query) || entry.text.contains(query))
            .map(|entry| entry.id.clone())
            .collect(),
    );
    desktop.shortcut.update(cx, |input, cx| {
        input.set_value(snapshot.settings.shortcut.clone(), window, cx);
    });
    desktop.limit.update(cx, |input, cx| {
        input.set_value(snapshot.settings.max_entries.to_string(), window, cx);
    });
    desktop.capacity.update(cx, |input, cx| {
        input.set_value(
            (snapshot.settings.max_bytes / 1024 / 1024).to_string(),
            window,
            cx,
        );
    });
    desktop.excluded.update(cx, |input, cx| {
        input.set_value(snapshot.settings.excluded_apps.join(", "), window, cx);
    });
    if let Some(preview) = &snapshot.preview {
        desktop.editor.update(cx, |editor, cx| {
            editor.set_value(preview.text.clone(), window, cx)
        });
        desktop.image = preview.image.as_ref().and_then(|(width, height, bytes)| {
            let mut bgra = bytes.clone();
            for pixel in bgra.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(*width, *height, bgra)
                .map(|image| Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
        });
    }
    desktop.snapshot = snapshot;
    if desktop.confirm.is_some() {
        desktop.dialog_focus.focus(window, cx);
    } else {
        desktop.focus.focus(window, cx);
    }
}

fn scenes() -> Vec<Scene> {
    let mut scenes = vec![];
    let mut add = |name: &str, kind, route, preview, state, step, width, height| {
        scenes.push(Scene {
            name: name.into(),
            kind,
            route,
            preview,
            state,
            step,
            width,
            height,
        })
    };
    for (name, route, state) in [
        ("manager", "all", "normal"),
        ("search", "all", "search"),
        ("pinned", "pinned", "normal"),
        ("favorites", "favorites", "normal"),
        ("multiselect", "all", "multi"),
        ("history-empty", "all", "empty"),
        ("no-results", "all", "no-results"),
        ("pinned-empty", "pinned", "empty"),
        ("favorites-empty", "favorites", "empty"),
        ("emoji", "emoji", "normal"),
        ("symbols", "symbols", "normal"),
        ("delete", "all", "delete"),
        ("clear", "all", "clear"),
        ("clear-protected", "all", "clear-protected"),
        ("record-menu", "all", "more"),
        ("window-menu", "all", "menu"),
    ] {
        let selected = match route {
            "pinned" => "text",
            _ if state == "search" => "pdf",
            _ => "rich",
        };
        add(
            name,
            Kind::Main,
            route,
            Some(selected),
            state,
            0,
            1120.,
            760.,
        );
    }
    add(
        "manager-narrow",
        Kind::Main,
        "all",
        Some("rich"),
        "normal",
        0,
        760.,
        680.,
    );
    for route in [
        "general",
        "appearance",
        "shortcuts",
        "storage",
        "privacy",
        "sync",
        "about",
    ] {
        add(
            &format!("settings-{route}"),
            Kind::Settings,
            route,
            None,
            "normal",
            0,
            960.,
            760.,
        );
    }
    add(
        "shortcut-recording",
        Kind::Settings,
        "shortcuts",
        None,
        "recording-shortcut",
        0,
        960.,
        760.,
    );
    add(
        "shortcut-conflict",
        Kind::Settings,
        "shortcuts",
        None,
        "shortcut-conflict",
        0,
        960.,
        760.,
    );
    for step in 0..4 {
        add(
            &format!("welcome-{}", step + 1),
            Kind::Welcome,
            "all",
            None,
            "normal",
            step,
            740.,
            550.,
        );
    }
    add(
        "quick",
        Kind::Quick,
        "all",
        None,
        "normal",
        0,
        QUICK_WIDTH,
        QUICK_HEIGHT,
    );
    add(
        "quick-compact",
        Kind::Quick,
        "all",
        None,
        "compact",
        0,
        QUICK_WIDTH,
        QUICK_HEIGHT,
    );
    for name in [
        "text",
        "rich",
        "image",
        "code",
        "markdown",
        "pdf",
        "file",
        "folder",
        "missing",
        "preview-error",
    ] {
        add(
            &format!("preview-{name}"),
            Kind::Quick,
            "all",
            Some(name),
            "normal",
            0,
            QUICK_WIDTH + PREVIEW_WIDTH,
            QUICK_HEIGHT,
        );
    }
    add(
        "preview-inline",
        Kind::Quick,
        "all",
        Some("rich"),
        "normal",
        0,
        QUICK_WIDTH,
        QUICK_HEIGHT,
    );
    add(
        "preview-loading",
        Kind::Quick,
        "all",
        Some("image"),
        "pending-preview",
        0,
        QUICK_WIDTH + PREVIEW_WIDTH,
        QUICK_HEIGHT,
    );
    for (name, state) in [
        ("quick-empty", "empty"),
        ("quick-no-results", "no-results"),
        ("quick-paused", "paused"),
        ("save-pending", "saving"),
        ("storage-error", "storage-error"),
        ("key-unavailable", "key-unavailable"),
        ("permission", "permission"),
        ("protected-capacity", "protected-capacity"),
        ("quick-menu", "menu"),
    ] {
        add(
            name,
            Kind::Quick,
            "all",
            None,
            state,
            0,
            QUICK_WIDTH,
            QUICK_HEIGHT,
        );
    }
    scenes
}
