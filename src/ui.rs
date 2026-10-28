mod chrome;
mod detail;
mod dialog;
mod history;
mod pages;
#[cfg(all(feature = "ui-capture", windows))]
mod review;
mod tray_menu;

const QUICK_WIDTH: f32 = 360.;
const QUICK_HEIGHT: f32 = 540.;
const PREVIEW_WIDTH: f32 = 480.;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::*,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    kbd::Kbd,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    popover::Popover,
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use x_clipboard::{
    i18n::{self, message, tr},
    model::{Entry, Settings},
    platform::{self, PlatformEvent},
    service::{Command, Service, Snapshot},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Main,
    Quick,
    Settings,
    Welcome,
    Tray,
}
struct Owner {
    service: Service,
    updater: x_clipboard::updater::Updater,
    update_revision: u64,
    update_armed: bool,
    windows: HashMap<Kind, (AnyWindowHandle, Entity<Desktop>)>,
    hidden_since: HashMap<Kind, Instant>,
    ui_initialized: bool,
    tray: Option<tray_icon::TrayIcon>,
    tray_visible: bool,
    #[cfg(not(windows))]
    native_tray_state: Option<(String, bool)>,
    target: usize,
    #[cfg(not(windows))]
    hotkeys: Option<global_hotkey::GlobalHotKeyManager>,
    #[cfg(not(windows))]
    shortcut: Option<global_hotkey::hotkey::HotKey>,
}
impl Global for Owner {}

pub fn run(service: Service) -> anyhow::Result<()> {
    gpui_kit::application()
        .with_quit_mode(QuitMode::Explicit)
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            cx.set_global(Owner {
                updater: x_clipboard::updater::Updater::new(service.root.as_ref().clone()),
                update_revision: 0,
                update_armed: false,
                service: service.clone(),
                windows: HashMap::new(),
                hidden_since: HashMap::new(),
                ui_initialized: false,
                tray: None,
                tray_visible: false,
                #[cfg(not(windows))]
                native_tray_state: None,
                target: 0,
                #[cfg(not(windows))]
                hotkeys: global_hotkey::GlobalHotKeyManager::new().ok(),
                #[cfg(not(windows))]
                shortcut: None,
            });
            let background = std::env::args().any(|v| v == "--background" || v == "--ui-review");
            cx.spawn(async move |cx| {
                let mut initialized = false;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(60))
                        .await;
                    let review_started = cx.update(|cx| {
                        let snapshot = cx.global::<Owner>().service.snapshot();
                        if !initialized && (snapshot.ready || snapshot.error.is_some()) {
                            initialized = true;
                            #[cfg(all(feature = "ui-capture", windows))]
                            if snapshot.ready && std::env::args().any(|arg| arg == "--ui-review") {
                                ensure_ui(cx);
                                review::start(cx);
                                return true;
                            }
                            if cx.global::<Owner>().ui_initialized {
                                apply_theme(&snapshot.settings.theme, cx);
                            }
                            let show_guide =
                                !snapshot.settings.setup_complete && !snapshot.settings.guide_seen;
                            if show_guide {
                                let mut settings = snapshot.settings.clone();
                                settings.guide_seen = true;
                                service.send(Command::Settings(settings));
                            }
                            if snapshot.settings.setup_complete
                                && let Some((handle, _)) =
                                    cx.global::<Owner>().windows.get(&Kind::Welcome).cloned()
                            {
                                let _ = handle.update(cx, |_, window, cx| hide(window, cx));
                            }
                            if !background || show_guide {
                                open_window(
                                    if show_guide {
                                        Kind::Welcome
                                    } else {
                                        Kind::Main
                                    },
                                    cx,
                                );
                            }
                        }
                        poll(cx);
                        false
                    });
                    if review_started {
                        break;
                    }
                }
            })
            .detach();
        });
    Ok(())
}
#[cfg(not(windows))]
fn native_tray_menu(cx: &App, paused: bool) -> anyhow::Result<tray_icon::menu::Menu> {
    use tray_icon::menu::*;
    let menu = Menu::new();
    for (id, label, icon) in [
        ("quick", "打开快捷面板", IconName::Clipboard),
        ("manager", "管理历史", IconName::LayoutGrid),
        (
            "pause",
            if paused {
                "开启记录"
            } else {
                "暂停记录"
            },
            if paused {
                IconName::Play
            } else {
                IconName::Pause
            },
        ),
        ("settings", "设置", IconName::Settings),
        ("quit", "退出 X Clipboard", IconName::Power),
    ] {
        let svg = gpui_kit::assets::AllAssets
            .load(&icon.path())?
            .ok_or_else(|| anyhow::anyhow!("托盘菜单图标缺失：{id}"))?;
        let renderer = cx.svg_renderer();
        let parsed = renderer.parse_svg(&svg)?;
        let image = renderer.render_parsed(
            &parsed,
            SvgSize::ExactSize(size(DevicePixels(16), DevicePixels(16))),
        )?;
        let mut rgba = image.as_bytes(0).unwrap().to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[..3].copy_from_slice(&[0xa5, 0x6c, 0x00]);
        }
        let icon = tray_icon::menu::Icon::from_rgba(rgba, 16, 16)?;
        menu.append(&IconMenuItem::with_id(
            id,
            tr(label),
            true,
            Some(icon),
            None,
        ))?;
    }
    Ok(menu)
}
fn tray(cx: &App) -> anyhow::Result<tray_icon::TrayIcon> {
    use tray_icon::{Icon, TrayIconBuilder};
    #[cfg(not(windows))]
    let menu = native_tray_menu(cx, false)?;
    #[cfg(windows)]
    let _ = cx;
    let pixels = image::load_from_memory(include_bytes!("../assets/branding/app-icon-32.png"))?
        .into_rgba8()
        .into_raw();
    let builder = TrayIconBuilder::new()
        .with_tooltip("X Clipboard · 本地加密剪贴板")
        .with_icon(Icon::from_rgba(pixels, 32, 32)?);
    #[cfg(not(windows))]
    let builder = builder.with_menu(Box::new(menu));
    #[cfg(windows)]
    let builder = builder.with_menu_on_left_click(false);
    Ok(builder.build()?)
}
fn set_tray_visible(visible: bool, cx: &mut App) -> anyhow::Result<()> {
    if let Some(tray) = &cx.global::<Owner>().tray {
        tray.set_visible(visible)?;
    } else if visible {
        let tray = tray(cx)?;
        cx.global_mut::<Owner>().tray = Some(tray);
        #[cfg(not(windows))]
        {
            cx.global_mut::<Owner>().native_tray_state = None;
        }
    }
    cx.global_mut::<Owner>().tray_visible = visible;
    Ok(())
}
fn poll(cx: &mut App) {
    if cx.global::<Owner>().service.snapshot().shutdown_complete {
        if !cx.global::<Owner>().update_armed
            && cx.global::<Owner>().service.snapshot().settings.auto_update
            && let Err(error) = cx.global::<Owner>().updater.install(false)
        {
            cx.global::<Owner>()
                .service
                .error(format!("更新安装失败：{error:#}"));
            eprintln!("更新安装失败：{error:#}");
        }
        cx.quit();
        return;
    }
    let snapshot = cx.global::<Owner>().service.snapshot();
    if snapshot.ready
        && snapshot.settings.show_tray_icon != cx.global::<Owner>().tray_visible
        && let Err(error) = set_tray_visible(snapshot.settings.show_tray_icon, cx)
    {
        cx.global::<Owner>()
            .service
            .error(format!("托盘显示设置失败：{error}"));
    }
    if cx.global::<Owner>().update_armed && !snapshot.quitting {
        cx.global::<Owner>().service.send(Command::Shutdown);
    }
    if snapshot.ready && !snapshot.quitting && !std::env::args().any(|a| a == "--ui-review") {
        cx.global_mut::<Owner>()
            .updater
            .tick(snapshot.settings.auto_update);
    }
    let revision = cx.global::<Owner>().updater.snapshot().0;
    if revision != cx.global::<Owner>().update_revision {
        cx.global_mut::<Owner>().update_revision = revision;
        let views: Vec<_> = cx
            .global::<Owner>()
            .windows
            .values()
            .map(|(_, v)| v.clone())
            .collect();
        for view in views {
            view.update(cx, |_, cx| cx.notify());
        }
    }
    #[cfg(not(windows))]
    {
        let settings = cx.global::<Owner>().service.snapshot().settings;
        let state = (settings.language.clone(), settings.paused);
        if cx.global::<Owner>().native_tray_state.as_ref() != Some(&state) {
            i18n::set_locale(&settings.language);
            match native_tray_menu(cx, settings.paused) {
                Ok(menu) => {
                    if let Some(tray) = &cx.global::<Owner>().tray {
                        tray.set_menu(Some(Box::new(menu)));
                    }
                }
                Err(error) => cx.global::<Owner>().service.error(error.to_string()),
            }
            cx.global_mut::<Owner>().native_tray_state = Some(state);
        }
    }
    while let Ok(event) = cx.global::<Owner>().service.ui_events.try_recv() {
        match event {
            PlatformEvent::ShowQuick(target) => {
                cx.global_mut::<Owner>().target = target;
                platform::mask_windows_menu();
                open_window(Kind::Quick, cx);
            }
            #[cfg(windows)]
            PlatformEvent::QuickKey { key, shift } => {
                if let Some((handle, _)) = cx.global::<Owner>().windows.get(&Kind::Quick).cloned() {
                    let _ = handle.update(cx, |_, window, cx| {
                        if platform::quick_panel_keeps_focus(native(window)) {
                            let mut keystroke = Keystroke::parse(key).unwrap();
                            keystroke.modifiers.shift = shift;
                            window.dispatch_event(
                                PlatformInput::KeyDown(KeyDownEvent {
                                    keystroke,
                                    is_held: false,
                                    prefer_character_input: false,
                                }),
                                cx,
                            );
                        }
                    });
                }
            }
            PlatformEvent::Error(e) => cx.global::<Owner>().service.error(e),
            PlatformEvent::ConfigureShortcut(shortcut, response) => {
                let _ = response.send(register_shortcut(&shortcut, cx).map_err(|e| e.to_string()));
            }
            _ => {}
        }
    }
    #[cfg(not(windows))]
    while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
        if event.state == global_hotkey::HotKeyState::Pressed {
            cx.global_mut::<Owner>().target = platform::foreground();
            open_window(Kind::Quick, cx);
        }
    }
    #[cfg(windows)]
    while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
        if cx.global::<Owner>().tray_visible
            && cx
                .global::<Owner>()
                .tray
                .as_ref()
                .is_some_and(|tray| event.id() == tray.id())
            && matches!(
                event,
                tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left | tray_icon::MouseButton::Right,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            )
        {
            cx.global_mut::<Owner>().target = platform::foreground();
            open_window(Kind::Tray, cx);
        }
    }
    #[cfg(not(windows))]
    while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
        match event.id.as_ref() {
            "quick" => {
                cx.global_mut::<Owner>().target = platform::foreground();
                open_window(Kind::Quick, cx);
            }
            "manager" => open_window(Kind::Main, cx),
            "settings" => open_window(Kind::Settings, cx),
            "pause" => {
                let service = &cx.global::<Owner>().service;
                let mut settings = service.snapshot().settings;
                settings.paused = !settings.paused;
                service.send(Command::Settings(settings));
            }
            "quit" => {
                cx.global::<Owner>().service.send(Command::Shutdown);
            }
            _ => {}
        }
    }
    if platform::dropped_events() {
        cx.global::<Owner>()
            .service
            .error("事件队列已满，部分复制未能保存".into());
    }
    let pairs: Vec<_> = cx
        .global::<Owner>()
        .windows
        .iter()
        .map(|(kind, pair)| (*kind, pair.clone()))
        .collect();
    for (kind, (handle, view)) in pairs {
        if let Some(hidden) = cx.global::<Owner>().hidden_since.get(&kind) {
            if hidden.elapsed() >= Duration::from_secs(1) {
                cx.global_mut::<Owner>().windows.remove(&kind);
                cx.global_mut::<Owner>().hidden_since.remove(&kind);
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
            continue;
        }
        let _ = handle.update(cx, |_, window, cx| {
            #[cfg(windows)]
            if kind == Kind::Quick && platform::quick_panel_should_dismiss(native(window)) {
                hide(window, cx);
                return;
            }
            view.update(cx, |this, cx| this.sync(window, cx));
        });
    }
    update_history_activity(cx);
}
fn update_history_activity(cx: &mut App) {
    let owner = cx.global::<Owner>();
    let active = owner.windows.keys().any(|kind| {
        matches!(kind, Kind::Main | Kind::Quick | Kind::Settings)
            && !owner.hidden_since.contains_key(kind)
    });
    owner.service.set_history_active(active);
}
#[cfg(not(windows))]
fn register_shortcut(value: &str, cx: &mut App) -> anyhow::Result<()> {
    let hotkey = value.parse::<global_hotkey::hotkey::HotKey>()?;
    let owner = cx.global_mut::<Owner>();
    let manager = owner
        .hotkeys
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("桌面不支持全局快捷键"))?;
    if owner.shortcut == Some(hotkey) {
        return Ok(());
    }
    manager.register(hotkey)?;
    if let Some(old) = owner.shortcut {
        let _ = manager.unregister(old);
    }
    owner.shortcut = Some(hotkey);
    Ok(())
}
#[cfg(windows)]
fn register_shortcut(_: &str, _: &mut App) -> anyhow::Result<()> {
    Ok(())
}
fn background_appearance(settings: &Settings) -> WindowBackgroundAppearance {
    if cfg!(windows) && settings.frosted_background {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    }
}
fn open_window(kind: Kind, cx: &mut App) {
    // Navigation may originate inside the same window's callback. Reuse it only
    // after that callback releases GPUI's window borrow.
    cx.defer(move |cx| open_window_now(kind, cx));
}
fn open_window_now(kind: Kind, cx: &mut App) {
    ensure_ui(cx);
    apply_theme(&cx.global::<Owner>().service.snapshot().settings.theme, cx);
    cx.global_mut::<Owner>().hidden_since.remove(&kind);
    if matches!(kind, Kind::Main | Kind::Quick | Kind::Settings) {
        cx.global::<Owner>().service.set_history_active(true);
    }
    if let Some((handle, view)) = cx.global::<Owner>().windows.get(&kind).cloned() {
        if let Err(error) = handle.update(cx, |_, window, cx| {
            view.update(cx, |this, cx| {
                this.target = cx.global::<Owner>().target;
                if kind == Kind::Quick {
                    this.route = "all";
                    this.filter = "all";
                    this.search
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                this.sync(window, cx);
                if kind == Kind::Welcome {
                    this.step = 0;
                }
                #[cfg(windows)]
                if kind == Kind::Quick {
                    platform::place_panel(native(window), &this.snapshot.settings.position);
                } else if kind == Kind::Tray {
                    this.tray_selected = None;
                    platform::place_panel(native(window), "cursor");
                }
                this.focus.focus(window, cx);
                cx.notify();
            });
            reveal(window, kind, cx);
        }) {
            cx.global::<Owner>()
                .service
                .error(format!("无法切换已有窗口：{error}"));
        }
        return;
    }
    let service = cx.global::<Owner>().service.clone();
    let (width, height) = match kind {
        Kind::Main => (1120., 760.),
        Kind::Quick => (QUICK_WIDTH, QUICK_HEIGHT),
        Kind::Settings => (960., 760.),
        Kind::Welcome => (740., 550.),
        Kind::Tray => (224., 200.),
    };
    let options = WindowOptions {
        window_background: background_appearance(&cx.global::<Owner>().service.snapshot().settings),
        focus: !(cfg!(windows) && kind == Kind::Quick),
        show: !(cfg!(windows) && kind == Kind::Quick),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(width), px(height)),
            cx,
        ))),
        titlebar: None,
        kind: if matches!(kind, Kind::Quick | Kind::Tray) {
            WindowKind::PopUp
        } else {
            WindowKind::Normal
        },
        is_resizable: !matches!(kind, Kind::Welcome | Kind::Tray),
        is_minimizable: kind != Kind::Tray,
        window_min_size: (kind != Kind::Tray).then_some(size(
            px(if kind == Kind::Quick { 360. } else { 700. }),
            px(500.),
        )),
        app_id: Some("com.xclipboard.app".into()),
        ..Default::default()
    };
    let result = gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title(match kind {
            Kind::Main => "X Clipboard",
            Kind::Quick => "X Clipboard · 快捷面板",
            Kind::Settings => "X Clipboard · 设置",
            Kind::Welcome => "欢迎使用 X Clipboard",
            Kind::Tray => "X Clipboard · 托盘菜单",
        });
        let view = cx.new(|cx| Desktop::new(kind, service, window, cx));
        window.on_window_should_close(cx, move |window, cx| {
            hide(window, cx);
            false
        });
        view
    });
    match result {
        Ok(pair) => {
            #[cfg(windows)]
            if kind == Kind::Quick {
                let position = cx.global::<Owner>().service.snapshot().settings.position;
                show_initial_quick(pair.0, position, cx);
            } else if kind == Kind::Tray {
                let _ = pair.0.update(cx, |_, window, cx| {
                    platform::place_panel(native(window), "cursor");
                    reveal(window, Kind::Tray, cx);
                });
            } else {
                let _ = pair.0.update(cx, |_, window, cx| reveal(window, kind, cx));
            }
            cx.global_mut::<Owner>().windows.insert(kind, pair);
        }
        Err(e) => cx
            .global::<Owner>()
            .service
            .error(format!("无法打开窗口：{e}")),
    }
}
fn ensure_ui(cx: &mut App) {
    if cx.global::<Owner>().ui_initialized {
        return;
    }
    gpui_kit::init(cx);
    apply_language(&cx.global::<Owner>().service.snapshot().settings.language);
    for source in x_clipboard::themes::THEME_SETS {
        ThemeRegistry::global_mut(cx)
            .load_themes_from_str(source)
            .expect("bundled GPUI Kit theme must be valid");
    }
    cx.global_mut::<Owner>().ui_initialized = true;
    apply_theme(&cx.global::<Owner>().service.snapshot().settings.theme, cx);
}
fn apply_language(language: &str) {
    i18n::set_locale(language);
    gpui_kit::component::set_locale(language);
}
#[cfg(windows)]
fn show_initial_quick(handle: AnyWindowHandle, position: String, cx: &mut App) {
    let service = cx.global::<Owner>().service.clone();
    let target = cx.global::<Owner>().target;
    let resized = handle
        .update(cx, |_, window, _| -> anyhow::Result<()> {
            platform::prepare_quick_panel(native(window), target)?;
            resize_quick_window(window, QUICK_WIDTH, QUICK_HEIGHT);
            Ok(())
        })
        .and_then(|result| result);
    if let Err(error) = resized {
        service.error(format!("快捷面板初始化失败：{error}"));
        return;
    }
    cx.spawn(async move |cx| {
        // Windows applies GPUI resize asynchronously. Keep the HWND hidden until
        // WM_SIZE reports the requested viewport, so its CW_USEDEFAULT size never flashes.
        for _ in 0..100 {
            cx.background_executor()
                .timer(Duration::from_millis(20))
                .await;
            let shown = cx.update(|cx| {
                handle.update(cx, |_, window, cx| {
                    let viewport = window.viewport_size();
                    if (f32::from(viewport.width) - QUICK_WIDTH).abs() > 1.
                        || (f32::from(viewport.height) - QUICK_HEIGHT).abs() > 1.
                    {
                        return false;
                    }
                    platform::place_panel(native(window), &position);
                    reveal(window, Kind::Quick, cx);
                    true
                })
            });
            match shown {
                Ok(true) | Err(_) => return,
                Ok(false) => {}
            }
        }
        service.error("快捷面板尺寸初始化超时，请重新打开".into());
    })
    .detach();
}
#[cfg(windows)]
fn native(window: &Window) -> usize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).unwrap().as_raw() {
        RawWindowHandle::Win32(h) => h.hwnd.get() as usize,
        _ => 0,
    }
}
fn hide(window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let kind = cx
        .global::<Owner>()
        .windows
        .iter()
        .find(|(_, (handle, _))| handle.window_id() == id)
        .map(|(kind, _)| *kind);
    if let Some(kind) = kind {
        cx.global_mut::<Owner>()
            .hidden_since
            .insert(kind, Instant::now());
        update_history_activity(cx);
    }
    #[cfg(windows)]
    platform::hide_window(native(window));
    #[cfg(not(windows))]
    window.minimize_window();
}
fn resize_quick_window(window: &mut Window, width: f32, height: f32) {
    #[cfg(windows)]
    platform::resize_quick_panel(native(window), width, height, window.scale_factor());
    #[cfg(not(windows))]
    window.resize(size(px(width), px(height)));
}
fn reveal(window: &mut Window, kind: Kind, cx: &mut App) {
    #[cfg(windows)]
    if kind == Kind::Quick {
        if let Err(error) = platform::show_quick_panel(native(window), cx.global::<Owner>().target)
        {
            cx.global::<Owner>().service.error(error.to_string());
        }
    } else {
        platform::show_window(native(window));
    }
    #[cfg(not(windows))]
    {
        let _ = (kind, cx);
        window.activate_window();
    }
}
fn apply_theme(preference: &str, cx: &mut App) {
    let config = ThemeRegistry::global(cx).themes().get(preference).cloned();
    let mode = config
        .as_ref()
        .map(|theme| theme.mode)
        .unwrap_or_else(|| match preference {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            _ => ThemeMode::from(cx.window_appearance()),
        });
    let light = ThemeRegistry::global(cx).default_light_theme().clone();
    let dark = ThemeRegistry::global(cx).default_dark_theme().clone();
    Theme::update(cx, |t| {
        t.light_theme = light;
        t.dark_theme = dark;
        if let Some(config) = &config {
            t.apply_config(config);
        }
    });
    Theme::change(mode, None, cx);
    Theme::update(cx, |t| {
        t.font_family = if cfg!(windows) {
            "Microsoft YaHei UI"
        } else {
            ".SystemUIFont"
        }
        .into();
        t.font_size = px(14.);
        t.radius = px(6.);
        t.radius_lg = px(10.);
        // Switch::switch is the unchecked track, not its checked accent.
        t.switch = rgb(if mode.is_dark() { 0x58616d } else { 0xbec6d0 }).into();
        t.switch_thumb = rgb(0xffffff).into();
        if config.is_some() {
            return;
        }
        t.primary = rgb(if mode.is_dark() { 0xffca45 } else { 0xa56c00 }).into();
        t.primary_hover = rgb(0xc18b0c).into();
        t.primary_active = rgb(0xa56c00).into();
        t.primary_foreground = rgb(0x29230f).into();
        t.button_primary = rgb(0xf5bc21).into();
        t.button_primary_hover = rgb(0xe9ac0b).into();
        t.button_primary_active = rgb(0xd99b00).into();
        t.button_primary_foreground = t.primary_foreground;
        t.ring = t.button_primary;
        t.progress_bar = t.button_primary;
        t.slider_bar = t.button_primary;
        t.list_active = rgb(if mode.is_dark() { 0x443a24 } else { 0xfff3ce }).into();
        t.list_active_border = t.primary;
        t.selection = t.list_active;
        t.sidebar = rgb(if mode.is_dark() { 0x1d2125 } else { 0xf0f2f4 }).into();
        t.background = rgb(if mode.is_dark() { 0x25292d } else { 0xffffff }).into();
        t.foreground = rgb(if mode.is_dark() { 0xe7eaed } else { 0x20252b }).into();
        t.border = rgb(if mode.is_dark() { 0x3b4249 } else { 0xe3e7ea }).into();
        t.muted = rgb(if mode.is_dark() { 0x202326 } else { 0xf6f7f8 }).into();
        t.muted_foreground = rgb(if mode.is_dark() { 0xa7b0b9 } else { 0x66717d }).into();
        t.switch_thumb = rgb(0xffffff).into();
        t.table_head = t.muted;
        t.table_head_foreground = t.foreground;
        t.popover = rgb(if mode.is_dark() { 0x2c3136 } else { 0xffffff }).into();
    });
}

struct Desktop {
    kind: Kind,
    service: Service,
    snapshot: Snapshot,
    route: &'static str,
    filter: &'static str,
    selected: Option<String>,
    multi: bool,
    checked: HashSet<String>,
    preview_open: bool,
    preview_key: String,
    preview_generation: u64,
    image: Option<Arc<RenderImage>>,
    thumbnails: HashMap<String, Arc<RenderImage>>,
    zoom: f32,
    search: Entity<InputState>,
    editor: Entity<TextareaState>,
    shortcut: Entity<InputState>,
    limit: Entity<InputState>,
    capacity: Entity<InputState>,
    excluded: Entity<InputState>,
    sync_endpoint: Entity<InputState>,
    sync_username: Entity<InputState>,
    sync_password: Entity<InputState>,
    sync_secret: Entity<InputState>,
    sync_inputs_loaded: bool,
    focus: FocusHandle,
    target: usize,
    menu: bool,
    confirm: Option<bool>,
    step: usize,
    recording_shortcut: bool,
    history_scroll: ListState,
    history_signature: Vec<String>,
    visible_cache: std::cell::RefCell<Option<history::VisibleCache>>,
    history_lines: Arc<Vec<history::HistoryLine>>,
    history_rows: Arc<Vec<usize>>,
    history_compact: bool,
    more: Option<String>,
    clear_protected: bool,
    dialog_focus: FocusHandle,
    tray_selected: Option<usize>,
}
impl Desktop {
    fn surface(&self, color: Hsla) -> Hsla {
        if cfg!(windows) && self.snapshot.settings.frosted_background {
            color.opacity(0.84)
        } else {
            color
        }
    }
    fn new(kind: Kind, service: Service, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let snapshot = service.snapshot();
        let settings = &snapshot.settings;
        window.set_background_appearance(background_appearance(settings));
        // Component Root defaults to an opaque fill. Let this view own its surface
        // so native acrylic can show through without altering the global theme.
        cx.defer_in(window, |_, window, cx| {
            gpui_kit::base::Root::update(window, cx, |root, _, cx| {
                root.style().background = Some(cx.theme().background.opacity(0.).into());
                cx.notify();
            });
        });
        apply_language(&settings.language);
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索内容、文件名称或来源…")));
        cx.subscribe_in(
            &search,
            window,
            |this, _, event: &InputEvent, window, cx| {
                #[cfg(windows)]
                if this.kind == Kind::Quick && matches!(event, InputEvent::Focus | InputEvent::Blur)
                {
                    platform::set_quick_search_mode(
                        native(window),
                        matches!(event, InputEvent::Focus),
                    );
                }
                #[cfg(not(windows))]
                let _ = window;
                if matches!(event, InputEvent::Change) {
                    if matches!(this.route, "all" | "pinned" | "favorites") {
                        this.service
                            .search(this.search.read(cx).value().to_string());
                    }
                    cx.notify();
                }
            },
        )
        .detach();
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        cx.observe_window_activation(window, |this, window, cx| {
            if (this.kind == Kind::Tray || (!cfg!(windows) && this.kind == Kind::Quick))
                && !window.is_window_active()
            {
                hide(window, cx);
            }
        })
        .detach();
        cx.observe_window_appearance(window, |this, _, cx| {
            if this.snapshot.settings.theme == "system" {
                apply_theme("system", cx);
            }
        })
        .detach();
        Self {
            kind,
            route: if kind == Kind::Settings {
                "general"
            } else {
                "all"
            },
            filter: "all",
            selected: None,
            multi: false,
            checked: HashSet::new(),
            preview_open: false,
            preview_key: String::new(),
            preview_generation: 0,
            image: None,
            thumbnails: HashMap::new(),
            zoom: 0.,
            editor: cx.new(|cx| TextareaState::new(window, cx)),
            shortcut: cx
                .new(|cx| InputState::new(window, cx).default_value(settings.shortcut.clone())),
            limit: cx.new(|cx| {
                InputState::new(window, cx).default_value(settings.max_entries.to_string())
            }),
            capacity: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value((settings.max_bytes / 1024 / 1024).to_string())
            }),
            excluded: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(tr("例如：KeePass.exe, 1Password.exe"))
                    .default_value(settings.excluded_apps.join(", "))
            }),
            sync_endpoint: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("https://dav.example.com/folder/")
                    .default_value(snapshot.sync.endpoint.clone())
            }),
            sync_username: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("WebDAV 用户名（匿名服务可留空）")
                    .default_value(snapshot.sync.username.clone())
            }),
            sync_password: cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("WebDAV 登录密码；已保存时留空不变")
            }),
            sync_secret: cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("至少 12 个字符；各设备使用相同同步密码")
            }),
            sync_inputs_loaded: snapshot.sync.configured,
            focus,
            target: cx.global::<Owner>().target,
            menu: false,
            confirm: None,
            step: 0,
            recording_shortcut: false,
            history_scroll: ListState::new(0, ListAlignment::Top, px(300.)),
            history_signature: vec![],
            visible_cache: Default::default(),
            history_lines: Default::default(),
            history_rows: Default::default(),
            history_compact: false,
            more: None,
            clear_protected: false,
            dialog_focus: cx.focus_handle(),
            tray_selected: None,
            service,
            snapshot,
            search,
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.service.snapshot();
        if snapshot.revision == self.snapshot.revision {
            return;
        }
        if !self.sync_inputs_loaded && snapshot.sync.configured {
            if self.sync_endpoint.read(cx).value().is_empty() {
                self.sync_endpoint.update(cx, |input, cx| {
                    input.set_value(snapshot.sync.endpoint.clone(), window, cx)
                });
                self.sync_username.update(cx, |input, cx| {
                    input.set_value(snapshot.sync.username.clone(), window, cx)
                });
            }
            self.sync_inputs_loaded = true;
        }
        if snapshot.settings.frosted_background != self.snapshot.settings.frosted_background {
            window.set_background_appearance(background_appearance(&snapshot.settings));
        }
        if snapshot.settings.theme != self.snapshot.settings.theme {
            apply_theme(&snapshot.settings.theme, cx);
        }
        if snapshot.settings.language != self.snapshot.settings.language {
            apply_language(&snapshot.settings.language);
            self.search.update(cx, |input, cx| {
                input.set_placeholder(tr("搜索内容、文件名称或来源…"), window, cx)
            });
            self.excluded.update(cx, |input, cx| {
                input.set_placeholder(tr("例如：KeePass.exe, 1Password.exe"), window, cx)
            });
        }
        self.snapshot = snapshot;
        self.thumbnails.retain(|id, image| {
            if self.snapshot.thumbnails.contains_key(id) {
                true
            } else {
                let _ = window.drop_image(image.clone());
                false
            }
        });
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.snapshot.entries.iter().any(|e| &e.id == id))
        {
            self.selected = None;
            self.preview_key.clear();
            if let Some(image) = self.image.take() {
                let _ = window.drop_image(image);
            }
        }
        if let Some(preview) = self
            .snapshot
            .preview
            .as_ref()
            .filter(|p| Some(&p.id) == self.selected.as_ref())
        {
            let key = format!("{}:{}", preview.id, preview.page);
            if self.preview_key != key {
                self.preview_key = key;
                if let Some(image) = self.image.take() {
                    let _ = window.drop_image(image);
                }
                self.editor
                    .update(cx, |e, cx| e.set_value(preview.text.clone(), window, cx));
                self.image = preview.image.as_ref().and_then(|(w, h, bytes)| {
                    let mut bgra = bytes.clone();
                    for pixel in bgra.chunks_exact_mut(4) {
                        pixel.swap(0, 2);
                    }
                    image::RgbaImage::from_raw(*w, *h, bgra)
                        .map(|image| Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
                });
            }
        }
        cx.notify();
    }
    fn visible(&self, cx: &App) -> Arc<Vec<usize>> {
        let search = self.search.read(cx).value().to_lowercase();
        let mut cache = self.visible_cache.borrow_mut();
        if let Some(cache) = cache.as_ref()
            && cache.matches(&self.snapshot, self.route, self.filter, &search)
        {
            return cache.rows.clone();
        }
        let rows: Arc<Vec<usize>> = Arc::new(
            self.snapshot
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    (self.route != "pinned" || e.pinned)
                        && (self.route != "favorites" || e.favorite)
                        && matches_filter(self.filter, e)
                        && (search.is_empty()
                            || (self.snapshot.search_query == search
                                && self.snapshot.search_ids.contains(&e.id))
                            || e.title.to_lowercase().contains(&search)
                            || e.source.to_lowercase().contains(&search))
                })
                .map(|(index, _)| index)
                .collect(),
        );
        *cache = Some(history::VisibleCache {
            entries: self.snapshot.entries.clone(),
            search_ids: self.snapshot.search_ids.clone(),
            search_query: self.snapshot.search_query.clone(),
            route: self.route,
            filter: self.filter,
            search,
            rows: rows.clone(),
        });
        rows
    }
    fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi && !self.checked.insert(id.clone()) {
            self.checked.remove(&id);
        }
        self.selected = Some(id.clone());
        self.preview_key.clear();
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        self.editor.update(cx, |e, cx| e.set_value("", window, cx));
        if self.kind != Kind::Quick || self.preview_open {
            self.preview_generation = self.service.preview(id, 0);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn write(&self, id: String, plain: bool, window: &mut Window, cx: &mut App) {
        let target = (self.kind == Kind::Quick).then_some(self.target);
        if target.is_some() {
            hide(window, cx);
        }
        self.service.send(Command::Write(id, plain, target));
    }
    fn update_settings(&self, update: impl FnOnce(&mut Settings)) {
        let mut settings = self.snapshot.settings.clone();
        update(&mut settings);
        self.service.send(Command::Settings(settings));
    }
    fn save_inputs(&self, cx: &App) {
        let mut settings = self.snapshot.settings.clone();
        let result = (|| -> anyhow::Result<()> {
            settings.shortcut = self.shortcut.read(cx).value().to_string();
            settings.max_entries = self.limit.read(cx).value().parse()?;
            settings.max_bytes = self
                .capacity
                .read(cx)
                .value()
                .parse::<u64>()?
                .checked_mul(1024 * 1024)
                .ok_or_else(|| anyhow::anyhow!("容量数值过大"))?;
            settings.excluded_apps = self
                .excluded
                .read(cx)
                .value()
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
            settings.validate()?;
            Ok(())
        })();
        match result {
            Ok(()) => self.service.send(Command::Settings(settings)),
            Err(e) => self.service.error(format!("设置未保存：{e}")),
        }
    }
    fn nav(&mut self, route: &'static str, cx: &mut Context<Self>) {
        self.route = route;
        self.more = None;
        self.filter = "all";
        self.multi = false;
        self.checked.clear();
        self.confirm = None;
        cx.notify();
    }
    fn titlebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = div()
            .h(px(if self.kind == Kind::Quick { 38. } else { 46. }))
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0();
        if cfg!(target_os = "macos") {
            for (i, color) in [0xff5f57, 0xfebc2e, 0x28c840].into_iter().enumerate() {
                bar = bar.child(
                    div()
                        .id(("traffic", i))
                        .size(px(12.))
                        .rounded_full()
                        .bg(rgb(color))
                        .on_click(move |_, window, cx| match i {
                            0 => hide(window, cx),
                            1 => window.minimize_window(),
                            _ => window.zoom_window(),
                        }),
                );
            }
        }
        bar = bar.child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap_2()
                .window_control_area(WindowControlArea::Drag)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(app_mark(18.))
                .child(tr(if self.kind == Kind::Welcome {
                    "欢迎使用"
                } else if self.kind == Kind::Settings {
                    settings_menu_name(self.route)
                } else if self.kind == Kind::Main {
                    "剪贴板"
                } else {
                    "X Clipboard"
                })),
        );
        bar = bar.child(
            Button::new("theme")
                .ghost()
                .w(px(30.))
                .h(px(30.))
                .p_0()
                .accessibility_label(tr("切换主题"))
                .icon(if cx.theme().is_dark() {
                    IconName::Sun
                } else {
                    IconName::Moon
                })
                .tooltip(tr("切换主题"))
                .dropdown_menu({
                    let service = self.service.clone();
                    let selected = self.snapshot.settings.theme.clone();
                    move |menu, _, cx| theme_menu(menu, service.clone(), &selected, cx)
                }),
        );
        let menu = div().w(px(196.)).p_1().flex().flex_col().gap_1().children(
            [
                ("quick", "快捷面板"),
                ("manager", "管理历史"),
                (
                    "pause",
                    if self.snapshot.settings.paused {
                        "开启记录"
                    } else {
                        "暂停记录"
                    },
                ),
                ("settings", "设置"),
                ("about", "关于"),
            ]
            .into_iter()
            .map(|(id, label)| {
                menu_action(
                    id,
                    label,
                    match id {
                        "quick" => IconName::Clipboard,
                        "manager" => IconName::LayoutDashboard,
                        "pause" => {
                            if self.snapshot.settings.paused {
                                IconName::Play
                            } else {
                                IconName::Pause
                            }
                        }
                        "settings" => IconName::Settings,
                        _ => IconName::Info,
                    },
                    if id == "quick" {
                        Some(&self.snapshot.settings.shortcut)
                    } else {
                        None
                    },
                    cx,
                )
                .disabled(is_current_navigation(self.kind, self.route, id))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.menu = false;
                    match id {
                        "quick" => open_window(Kind::Quick, cx),
                        "manager" => open_window(Kind::Main, cx),
                        "pause" => this.service.send(Command::TogglePause),
                        "settings" => open_window(Kind::Settings, cx),
                        "about" => cx.defer(|cx| {
                            open_window_now(Kind::Settings, cx);
                            if let Some((_, view)) =
                                cx.global::<Owner>().windows.get(&Kind::Settings).cloned()
                            {
                                view.update(cx, |view, cx| view.nav("about", cx));
                            }
                        }),
                        _ => {}
                    };
                    cx.notify();
                }))
            }),
        );
        bar = bar.child(
            Popover::new("window-menu")
                .anchor(Anchor::TopRight)
                .open(self.menu)
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    this.menu = *open;
                    cx.notify();
                }))
                .trigger(
                    Button::new("window-menu-button")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .p_0()
                        .accessibility_label(tr("窗口菜单"))
                        .icon(IconName::ChevronDown)
                        .tooltip(tr("窗口菜单")),
                )
                .child(menu),
        );
        if !cfg!(target_os = "macos") {
            bar = bar
                .child(div().w(px(1.)).h(px(16.)).bg(cx.theme().border))
                .child(
                    Button::new("minimize")
                        .ghost()
                        .w(px(32.))
                        .h(px(30.))
                        .p_0()
                        .accessibility_label(tr("最小化"))
                        .icon(IconName::WindowMinimize)
                        .on_click(|_, w, _| w.minimize_window()),
                );
            if self.kind == Kind::Main {
                let icon = if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                };
                bar = bar.child(if cfg!(windows) {
                    div()
                        .id("maximize")
                        .w(px(32.))
                        .h(px(30.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .role(accesskit::Role::Button)
                        .aria_label(tr("最大化 / 还原"))
                        .text_color(cx.theme().secondary_foreground)
                        .hover(|style| {
                            style
                                .bg(if cx.theme().is_dark() {
                                    cx.theme().accent.opacity(0.5)
                                } else {
                                    cx.theme().accent
                                })
                                .text_color(cx.theme().accent_foreground)
                        })
                        .window_control_area(WindowControlArea::Max)
                        .child(Icon::new(icon).size(px(16.)))
                        .into_any_element()
                } else {
                    Button::new("maximize")
                        .ghost()
                        .w(px(32.))
                        .h(px(30.))
                        .p_0()
                        .accessibility_label(tr("最大化 / 还原"))
                        .icon(icon)
                        .on_click(|_, w, _| w.zoom_window())
                        .into_any_element()
                });
            }
            bar = bar.child(
                Button::new("close")
                    .ghost()
                    .w(px(32.))
                    .h(px(30.))
                    .p_0()
                    .accessibility_label(tr("关闭窗口"))
                    .icon(IconName::WindowClose)
                    .on_click(|_, w, cx| hide(w, cx)),
            );
        }
        bar.into_any_element()
    }
    fn keyboard(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if self.kind == Kind::Tray {
            match key.key.as_str() {
                "escape" => hide(window, cx),
                "down" => {
                    self.tray_selected =
                        Some(self.tray_selected.map_or(0, |index| (index + 1) % 5));
                    cx.notify();
                }
                "up" => {
                    self.tray_selected =
                        Some(self.tray_selected.map_or(4, |index| (index + 4) % 5));
                    cx.notify();
                }
                "enter" => {
                    if let Some(index) = self.tray_selected {
                        self.tray_action(tray_menu::ACTIONS[index], window, cx);
                    }
                }
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if self.recording_shortcut {
            if key.key == "escape" {
                self.recording_shortcut = false;
                cx.notify();
                return;
            }
            if key.key.len() == 1 && key.key.as_bytes()[0].is_ascii_alphanumeric() {
                let mut parts = vec![];
                if key.modifiers.platform {
                    parts.push(if cfg!(windows) { "Win" } else { "Super" }.into());
                }
                if key.modifiers.control {
                    parts.push("Ctrl".into());
                }
                if key.modifiers.alt {
                    parts.push("Alt".into());
                }
                if key.modifiers.shift {
                    parts.push("Shift".into());
                }
                parts.push(key.key.to_uppercase());
                self.shortcut
                    .update(cx, |i, cx| i.set_value(parts.join("+"), window, cx));
                self.recording_shortcut = false;
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if self.confirm.is_some() && key.key == "enter" {
            self.confirm = None;
            self.focus.focus(window, cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if key.key == "escape" {
            if self.confirm.take().is_some() {
                self.focus.focus(window, cx);
            } else if self.more.take().is_some() {
            } else if self.menu {
                self.menu = false;
            } else if self.preview_open {
                self.preview_open = false;
                if self.kind == Kind::Quick {
                    resize_quick_window(window, QUICK_WIDTH, QUICK_HEIGHT);
                }
            } else {
                hide(window, cx);
            }
            cx.notify();
            return;
        }
        if !self.focus.is_focused(window) || !matches!(self.route, "all" | "pinned" | "favorites") {
            return;
        }
        if key.key == "space" {
            if let Some(id) = self.selected.clone().or_else(|| {
                self.visible(cx)
                    .first()
                    .map(|index| self.snapshot.entries[*index].id.clone())
            }) {
                if self.preview_open {
                    self.close_preview(window, cx);
                } else {
                    self.open_preview(id, window, cx);
                }
            }
            cx.stop_propagation();
            return;
        }
        if key.key == "enter"
            && let Some(id) = self.selected.clone().or_else(|| {
                self.visible(cx)
                    .first()
                    .map(|index| self.snapshot.entries[*index].id.clone())
            })
        {
            self.write(id, key.modifiers.shift, window, cx);
        }
        if key.key == "down" || key.key == "up" {
            let entries = self.visible(cx);
            if entries.is_empty() {
                return;
            }
            let at = entries.iter().position(|index| {
                Some(&self.snapshot.entries[*index].id) == self.selected.as_ref()
            });
            let next = if let Some(at) = at {
                if key.key == "down" {
                    (at + 1).min(entries.len() - 1)
                } else {
                    at.saturating_sub(1)
                }
            } else {
                0
            };
            self.select(self.snapshot.entries[entries[next]].id.clone(), window, cx);
            let pinned = entries
                .iter()
                .filter(|index| self.snapshot.entries[**index].pinned)
                .count();
            self.history_scroll
                .scroll_to_reveal_item(next + if next < pinned || pinned == 0 { 1 } else { 2 });
        }
    }
}
impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(windows)]
        if self.kind == Kind::Quick {
            platform::set_quick_search_mode(
                native(window),
                self.search.read(cx).focus_handle(cx).is_focused(window),
            );
        }
        let placeholder = tr(match self.route {
            "emoji" => "搜索表情名称或关键词…",
            "symbols" => "搜索符号名称或关键词…",
            _ => "搜索内容、文件名称或来源…",
        });
        if self.search.read(cx).presentation().placeholder().as_ref() != placeholder {
            self.search.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        let mut root = div()
            .size_full()
            .relative()
            .bg(self.surface(cx.theme().background))
            .text_color(cx.theme().foreground)
            .text_size(px(14.))
            .font_family(cx.theme().font_family.clone())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::keyboard));
        if self.kind == Kind::Tray {
            return root
                .bg(self.surface(cx.theme().popover))
                .child(self.tray_panel(cx));
        }
        if self.kind == Kind::Welcome {
            root = root.child(self.welcome(window, cx));
        } else {
            let title = self.titlebar(window, cx);
            let content = if self.kind == Kind::Settings {
                self.settings(cx)
            } else if self.route == "emoji" || self.route == "symbols" {
                self.symbols(window, cx)
            } else {
                self.history(window, cx)
            };
            let main = div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .flex()
                .flex_col()
                .child(title)
                .child(content);
            let body = div()
                .flex_1()
                .min_h_0()
                .flex()
                .when(self.kind != Kind::Quick, |d| d.child(self.sidebar(cx)))
                .child(main);
            root = root.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(body)
                    .when(self.kind != Kind::Settings, |d| {
                        d.child(self.footer(window, cx))
                    }),
            );
        }
        if let Some(error) = self.snapshot.error.clone() {
            root = root.child(
                div()
                    .absolute()
                    .bottom_12()
                    .left_4()
                    .right_4()
                    .p_3()
                    .rounded_md()
                    .shadow_lg()
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(rgb(0xb97842))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .whitespace_normal()
                            .child(tr(&error).to_owned()),
                    )
                    .when(
                        !self.snapshot.ready || error.contains("保存") || error.contains("存储"),
                        |d| {
                            d.child(
                                Button::new("retry")
                                    .flex_shrink_0()
                                    .label(tr("重试"))
                                    .small()
                                    .h(px(30.))
                                    .on_click(
                                        cx.listener(|t, _, _, _| t.service.send(Command::Retry)),
                                    ),
                            )
                        },
                    )
                    .child(
                        Button::new("dismiss-error")
                            .flex_shrink_0()
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .accessibility_label(tr("关闭提示"))
                            .icon(IconName::X)
                            .ghost()
                            .small()
                            .on_click(cx.listener(|t, _, _, _| t.service.dismiss_error())),
                    ),
            );
        }
        if self.confirm.is_some() {
            root = root.child(self.confirmation(cx));
        }
        root
    }
}
fn app_mark(size: f32) -> AnyElement {
    static MARK: std::sync::LazyLock<Arc<Image>> = std::sync::LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/branding/app-icon-128.png").to_vec(),
        ))
    });
    img(MARK.clone())
        .size(px(size))
        .flex_shrink_0()
        .into_any_element()
}

fn accent(cx: &App) -> Hsla {
    cx.theme().primary
}
fn accent_soft(cx: &App) -> Hsla {
    cx.theme().primary.opacity(0.12)
}
fn selected_color(cx: &App) -> Hsla {
    cx.theme().list_active
}
fn warning(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() {
        0xefc37d
    } else {
        0x946113
    })
    .into()
}
fn warning_soft(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() {
        0x463c29
    } else {
        0xfff4df
    })
    .into()
}
fn danger(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() {
        0xff8e99
    } else {
        0xc63643
    })
    .into()
}
fn icon_button(id: impl Into<ElementId>, icon: IconName, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .w(px(26.))
        .h(px(26.))
        .p_0()
        .icon(Icon::new(icon).size(px(14.)))
        .tooltip(label)
        .accessibility_label(tr(label).to_owned())
}
fn shortcut_keystroke(label: &str) -> Option<Keystroke> {
    let binding = label
        .replace("Shift ↵", "shift-enter")
        .replace('↵', "enter")
        .replace("Esc", "escape")
        .replace("Win", "cmd")
        .replace("Super", "cmd")
        .replace('⌘', "cmd-")
        .replace('⇧', "shift-")
        .replace('⌃', "ctrl-")
        .replace('⌥', "alt-")
        .replace('+', "-")
        .to_lowercase();
    Keystroke::parse(&binding).ok()
}

fn keycap(label: impl Into<SharedString>, cx: &App) -> AnyElement {
    let label = label.into();
    match shortcut_keystroke(&label) {
        Some(stroke) => Kbd::new(stroke)
            .text_size(px(9.))
            .h(px(20.))
            .into_any_element(),
        None => div()
            .text_size(px(10.))
            .text_color(cx.theme().muted_foreground)
            .child(label)
            .into_any_element(),
    }
}
fn theme_menu(mut menu: PopupMenu, service: Service, selected: &str, cx: &App) -> PopupMenu {
    menu = menu.scrollable(true).max_h(px(360.));
    for (name, label) in [("system", "跟随系统"), ("light", "浅色"), ("dark", "深色")] {
        let service = service.clone();
        menu = menu.item(
            PopupMenuItem::new(tr(label))
                .checked(selected == name)
                .on_click(move |_, _, _| {
                    let mut settings = service.snapshot().settings;
                    settings.theme = name.into();
                    service.send(Command::Settings(settings));
                }),
        );
    }
    menu = menu.separator();
    for config in ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .filter(|config| !config.is_default)
    {
        let name = config.name.to_string();
        let service = service.clone();
        menu = menu.item(
            PopupMenuItem::new(name.clone())
                .checked(selected == name)
                .on_click(move |_, _, _| {
                    let mut settings = service.snapshot().settings;
                    settings.theme = name.clone();
                    service.send(Command::Settings(settings));
                }),
        );
    }
    menu
}
fn status_badge(snapshot: &Snapshot, cx: &App) -> AnyElement {
    let (label, attention) = if !snapshot.ready {
        ("记录不可用", true)
    } else if snapshot.settings.paused {
        ("已暂停", true)
    } else if snapshot.status.contains("保存中") {
        ("保存中…", false)
    } else if snapshot.status.contains("超过容量") || snapshot.status.contains("超限") {
        ("保护内容超限", true)
    } else if snapshot.error.is_some() {
        ("需要处理", true)
    } else {
        ("正在记录", false)
    };
    div()
        .px(px(7.))
        .py(px(3.))
        .rounded(px(5.))
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(10.))
        .bg(if attention {
            warning_soft(cx)
        } else {
            accent_soft(cx)
        })
        .text_color(if attention { warning(cx) } else { accent(cx) })
        .child(
            Icon::new(if attention {
                IconName::Info
            } else {
                IconName::Check
            })
            .size(px(12.)),
        )
        .child(tr(label).to_owned())
        .into_any_element()
}
#[inline(never)]
fn is_current_navigation(kind: Kind, route: &str, action: &str) -> bool {
    matches!(
        (kind, action),
        (Kind::Quick, "quick") | (Kind::Main, "manager") | (Kind::Settings, "settings")
    ) || (kind == Kind::Settings && route == "about" && action == "about")
}
fn settings_menu_name(route: &str) -> &'static str {
    match route {
        "appearance" => "外观设置",
        "shortcuts" => "快捷键",
        "storage" => "本地存储",
        "privacy" => "隐私保护",
        "sync" => "同步",
        "about" => "关于",
        _ => "通用设置",
    }
}
fn matches_filter(filter: &str, entry: &Entry) -> bool {
    match filter {
        "all" => true,
        "pinned" => entry.pinned,
        "favorites" => entry.favorite,
        kind => entry.kind == kind,
    }
}

fn menu_action(
    id: impl Into<ElementId>,
    label: &'static str,
    icon: IconName,
    shortcut: Option<&str>,
    cx: &App,
) -> Button {
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(34.))
        .px_2()
        .accessibility_label(tr(label).to_owned())
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .child(Icon::new(icon).size(px(15.)))
                .child(tr(label).to_owned())
                .child(div().flex_1())
                .when_some(shortcut, |d, s| d.child(keycap(s.to_owned(), cx))),
        )
}
fn record_title(entry: &Entry) -> String {
    if matches!(entry.kind.as_str(), "text" | "rich")
        && let Some(line) = entry.text.lines().find(|line| !line.trim().is_empty())
    {
        return line.chars().take(100).collect();
    }
    entry.title.clone()
}
fn kind_label(kind: &str) -> &'static str {
    match kind {
        "image" => "图片",
        "file" => "文件",
        "rich" => "富文本",
        _ => "文本",
    }
}
fn bytes_label(value: u64) -> String {
    if value >= 1024 * 1024 {
        format!("{:.1} MB", value as f64 / 1048576.)
    } else {
        format!("{:.1} KB", value as f64 / 1024.)
    }
}
fn age(time: i64) -> String {
    let seconds = (x_clipboard::model::now_ms() - time).max(0) / 1000;
    if seconds < 60 {
        tr("刚刚").into()
    } else if seconds < 3600 {
        message("{} 分钟前", &[("", (seconds / 60).to_string())])
    } else if seconds < 86400 {
        message("{} 小时前", &[("", (seconds / 3600).to_string())])
    } else {
        message("{} 天前", &[("", (seconds / 86400).to_string())])
    }
}

#[cfg(test)]
mod ui_tests {
    #[test]
    fn navigation_to_current_window_or_about_page_is_disabled() {
        use super::Kind;
        for (kind, route, action) in [
            (Kind::Main, "all", "manager"),
            (Kind::Quick, "all", "quick"),
            (Kind::Settings, "general", "settings"),
            (Kind::Settings, "about", "about"),
        ] {
            assert!(super::is_current_navigation(kind, route, action));
        }
        for (kind, route, action) in [
            (Kind::Main, "all", "settings"),
            (Kind::Welcome, "all", "manager"),
            (Kind::Tray, "all", "quick"),
            (Kind::Settings, "general", "about"),
        ] {
            assert!(!super::is_current_navigation(kind, route, action));
        }
    }
    #[test]
    fn kbd_aliases_keep_modifier_meanings() {
        for (label, canonical) in [
            ("Win+V", "cmd-v"),
            ("Super+Shift+V", "cmd-shift-v"),
            ("⌘⇧V", "cmd-shift-v"),
            ("Shift ↵", "shift-enter"),
            ("Esc", "escape"),
        ] {
            assert_eq!(
                super::shortcut_keystroke(label).unwrap(),
                gpui_kit::Keystroke::parse(canonical).unwrap()
            );
        }
    }
}
