use super::*;
use crate::model::FormatData;
use anyhow::{bail, ensure};
use clipboard_rs::{
    Clipboard, ClipboardContent, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, ContentFormat,
    common::{RustImage, RustImageData},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

pub struct PlatformClipboard {
    context: Mutex<ClipboardContext>,
    own_token: Mutex<Option<Vec<u8>>>,
    version: Arc<AtomicU32>,
    wayland: bool,
    events: Sender<PlatformEvent>,
    shutdown: Mutex<Option<clipboard_rs::WatcherShutdown>>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    xwayland: bool,
}
struct Watcher {
    events: Sender<PlatformEvent>,
    version: Arc<AtomicU32>,
}
impl ClipboardHandler for Watcher {
    fn on_clipboard_change(&mut self) {
        let version = self.version.fetch_add(1, Ordering::AcqRel) + 1;
        let _ = self.events.try_send(PlatformEvent::Changed(version));
    }
}
impl PlatformClipboard {
    pub fn start(events: Sender<PlatformEvent>) -> Result<Self> {
        let context =
            ClipboardContext::new().map_err(|e| anyhow::anyhow!("无法连接系统剪贴板：{e}"))?;
        let mut watcher = ClipboardWatcherContext::new_with_interval(Duration::from_millis(40))
            .map_err(|e| anyhow::anyhow!("无法监听系统剪贴板：{e}"))?;
        #[cfg(target_os = "linux")]
        let wayland = matches!(&context, ClipboardContext::Wayland(_));
        #[cfg(target_os = "macos")]
        let wayland = false;
        #[cfg(target_os = "linux")]
        ensure!(
            wayland == matches!(&watcher, ClipboardWatcherContext::Wayland(_)),
            "剪贴板读取和监听后端不一致，无法可靠记录"
        );
        let version = Arc::new(AtomicU32::new(1));
        watcher.add_handler(Watcher {
            events: events.clone(),
            version: version.clone(),
        });
        let shutdown = watcher.get_shutdown_channel();
        let thread = std::thread::Builder::new()
            .name("clipboard-events".into())
            .spawn(move || watcher.start_watch())?;
        Ok(Self {
            context: Mutex::new(context),
            own_token: Mutex::new(None),
            version,
            wayland,
            xwayland: !wayland && std::env::var_os("WAYLAND_DISPLAY").is_some(),
            events,
            shutdown: Mutex::new(Some(shutdown)),
            thread: Mutex::new(Some(thread)),
        })
    }
}
impl ClipboardBackend for PlatformClipboard {
    fn read(&self, version: u32, respect_sensitive: bool) -> Result<Option<ClipContent>> {
        let clipboard = self.context.lock().unwrap();
        #[cfg(target_os = "macos")]
        let pasteboard_count = objc2_app_kit::NSPasteboard::generalPasteboard().changeCount();
        let formats = clipboard
            .available_formats()
            .map_err(|e| anyhow::anyhow!("读取格式列表失败：{e}"))?;
        if respect_sensitive
            && formats.iter().any(|name| {
                matches!(
                    name.as_str(),
                    "org.nspasteboard.ConcealedType"
                        | "org.nspasteboard.TransientType"
                        | "x-kde-passwordManagerHint"
                )
            })
        {
            return Ok(None);
        }
        let before = self.version.load(Ordering::Acquire);
        if version != 0 && version != before {
            bail!("复制变化过快：上一版内容在读取前已覆盖");
        }
        if let Ok(token) = clipboard.get_buffer("application/x-xclipboard-origin") {
            let mut own = self.own_token.lock().unwrap();
            if own.as_ref() == Some(&token) {
                return Ok(None);
            }
            own.take();
        }
        let mut content = ClipContent::default();
        for (format, name) in [
            (ContentFormat::Text, "text/plain"),
            (ContentFormat::Html, "text/html"),
        ] {
            if clipboard.has(format.clone()) {
                let text = match format {
                    ContentFormat::Text => clipboard.get_text(),
                    ContentFormat::Html => clipboard.get_html(),
                    _ => clipboard.get_rich_text(),
                }
                .map_err(|e| anyhow::anyhow!("读取剪贴板格式失败：{e}"))?;
                if name == "text/plain" {
                    content.text = text.clone();
                }
                content.formats.push(FormatData {
                    name: name.into(),
                    bytes: text.into_bytes(),
                });
            }
        }
        if clipboard.has(ContentFormat::Rtf) {
            let format = if cfg!(target_os = "macos") {
                "public.rtf"
            } else {
                "text/rtf"
            };
            let bytes = clipboard
                .get_buffer(format)
                .map_err(|e| anyhow::anyhow!("读取 RTF 失败：{e}"))?;
            content.formats.push(FormatData {
                name: "text/rtf".into(),
                bytes,
            });
        }
        if clipboard.has(ContentFormat::Image) {
            let image = clipboard
                .get_image()
                .map_err(|e| anyhow::anyhow!("读取图片失败：{e}"))?;
            let png = image
                .to_png()
                .map_err(|e| anyhow::anyhow!("编码图片失败：{e}"))?;
            content.formats.push(FormatData {
                name: "image/png".into(),
                bytes: png.get_bytes().to_vec(),
            });
        }
        if clipboard.has(ContentFormat::Files) {
            content.files = clipboard
                .get_files()
                .map_err(|e| anyhow::anyhow!("读取文件列表失败：{e}"))?
                .into_iter()
                .map(|p| {
                    url::Url::parse(&p)
                        .ok()
                        .and_then(|u| u.to_file_path().ok())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(p)
                })
                .collect();
            content.formats.push(FormatData {
                name: "text/uri-list".into(),
                bytes: content.files.join("\n").into_bytes(),
            });
        }
        ensure!(
            before == self.version.load(Ordering::Acquire),
            "读取期间剪贴板已变化，本次快照已丢弃"
        );
        #[cfg(target_os = "macos")]
        ensure!(
            pasteboard_count == objc2_app_kit::NSPasteboard::generalPasteboard().changeCount(),
            "读取期间剪贴板已变化，本次快照已丢弃"
        );
        content.source = "系统剪贴板".into();
        if content.formats.is_empty() {
            return Ok(None);
        }
        content.validate()?;
        Ok(Some(content))
    }
    fn write(&self, content: &ClipContent, plain: bool) -> Result<()> {
        content.validate()?;
        let mut values = vec![];
        if plain {
            ensure!(!content.text.is_empty(), "此记录没有纯文本内容");
            values.push(ClipboardContent::Text(content.text.clone()));
        } else {
            for f in &content.formats {
                match f.name.as_str() {
                    "text/plain" => {
                        values.push(ClipboardContent::Text(String::from_utf8(f.bytes.clone())?))
                    }
                    "text/html" => {
                        values.push(ClipboardContent::Html(String::from_utf8(f.bytes.clone())?))
                    }
                    "text/rtf" => values.push(ClipboardContent::Other(
                        if cfg!(target_os = "macos") {
                            "public.rtf"
                        } else {
                            "text/rtf"
                        }
                        .into(),
                        f.bytes.clone(),
                    )),
                    "image/png" => values.push(ClipboardContent::Image(
                        RustImageData::from_bytes(&f.bytes)
                            .map_err(|e| anyhow::anyhow!("图片解码失败：{e}"))?,
                    )),
                    "text/uri-list" => {
                        for p in &content.files {
                            ensure!(std::path::Path::new(p).try_exists()?, "文件引用已失效：{p}");
                        }
                        values.push(ClipboardContent::Files(content.files.clone()));
                    }
                    _ => {}
                }
            }
        }
        ensure!(!values.is_empty(), "当前平台不支持此记录的格式");
        let token = uuid::Uuid::new_v4().as_bytes().to_vec();
        values.push(ClipboardContent::Other(
            "application/x-xclipboard-origin".into(),
            token.clone(),
        ));
        let clipboard = self.context.lock().unwrap();
        clipboard
            .set(values)
            .map_err(|e| anyhow::anyhow!("写回系统剪贴板失败：{e}"))?;
        *self.own_token.lock().unwrap() = Some(token);
        Ok(())
    }
    fn shortcut(&self, shortcut: &str) -> Result<()> {
        ensure!(
            !self.wayland && !self.xwayland,
            "Wayland 全局快捷键需要桌面授权；请在桌面设置中绑定应用，或通过托盘打开面板"
        );
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.events
            .send(PlatformEvent::ConfigureShortcut(shortcut.into(), tx))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("快捷键注册超时"))?
            .map_err(anyhow::Error::msg)
    }
    fn paste(&self, target: usize) -> Result<()> {
        ensure!(target != 0, "目标窗口不可用；内容已复制，请手动粘贴");
        if self.wayland || self.xwayland {
            bail!("当前 Wayland 环境未授予自动粘贴能力；内容已复制，请手动粘贴");
        }
        paste_to(target)
    }
    fn capabilities(&self) -> &'static str {
        if self.xwayland {
            "XWayland 回退：仅能记录桌面转发给 X11 的内容；原生 Wayland 应用可能不可读。请复制后手动粘贴。应用来源排除暂不可用。"
        } else if self.wayland {
            "Wayland data-control：快捷键及自动回贴受桌面限制，请复制后手动粘贴。应用来源排除暂不可用。"
        } else if cfg!(target_os = "macos") {
            "macOS：多格式采集；自动粘贴需要系统辅助功能与自动化权限。应用来源排除暂不可用。"
        } else {
            "X11：多格式采集、全局快捷键、焦点恢复与自动粘贴。应用来源排除暂不可用。"
        }
    }
    fn shutdown(&self) {
        if let Some(shutdown) = self.shutdown.lock().unwrap().take() {
            shutdown.stop();
        }
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Drop for PlatformClipboard {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub fn dropped_events() -> bool {
    false
}
pub fn mask_windows_menu() {}
#[cfg(target_os = "macos")]
pub fn foreground() -> usize {
    objc2_app_kit::NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier() as usize)
        .unwrap_or(0)
}
#[cfg(target_os = "macos")]
fn paste_to(target: usize) -> Result<()> {
    let script = format!(
        "tell application \"System Events\"\nset frontmost of first application process whose unix id is {target} to true\ndelay 0.1\nkeystroke \"v\" using command down\nend tell"
    );
    let result = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()?;
    ensure!(
        result.status.success(),
        "自动粘贴未获授权；请在系统设置中允许辅助功能与自动化，或手动粘贴"
    );
    Ok(())
}
#[cfg(target_os = "linux")]
pub fn foreground() -> usize {
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
    x11rb::connect(None)
        .ok()
        .and_then(|(c, _)| {
            c.get_input_focus()
                .ok()?
                .reply()
                .ok()
                .map(|r| r.focus as usize)
        })
        .unwrap_or(0)
}
#[cfg(target_os = "linux")]
fn paste_to(target: usize) -> Result<()> {
    use x11rb::{
        CURRENT_TIME,
        connection::Connection,
        protocol::{
            xproto::{ConnectionExt, InputFocus},
            xtest::ConnectionExt as _,
        },
    };
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    conn.set_input_focus(InputFocus::PARENT, target as u32, CURRENT_TIME)?
        .check()?;
    conn.flush()?;
    let min = conn.setup().min_keycode;
    let max = conn.setup().max_keycode;
    let map = conn.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let find = |sym| {
        map.keysyms
            .chunks(map.keysyms_per_keycode as usize)
            .position(|keys| keys.contains(&sym))
            .map(|index| min + index as u8)
            .ok_or_else(|| anyhow::anyhow!("键盘布局中找不到粘贴按键"))
    };
    let ctrl = find(0xffe3)?;
    let v = find(0x0076)?;
    std::thread::sleep(Duration::from_millis(100));
    for (kind, key) in [(2, ctrl), (2, v), (3, v), (3, ctrl)] {
        conn.xtest_fake_input(kind, key, CURRENT_TIME, root, 0, 0, 0)?
            .check()?;
    }
    conn.flush()?;
    Ok(())
}
pub fn set_startup(enabled: bool) -> Result<()> {
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "linux")]
    {
        let config = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("找不到配置目录"))?
            .config_dir()
            .join("autostart");
        std::fs::create_dir_all(&config)?;
        let path = config.join("x-clipboard.desktop");
        if enabled {
            std::fs::write(
                path,
                format!(
                    "[Desktop Entry]\nType=Application\nName=X Clipboard\nExec=\"{}\" --background\nX-GNOME-Autostart-enabled=true\n",
                    executable.display()
                ),
            )?;
        } else if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let home = directories::BaseDirs::new().ok_or_else(|| anyhow::anyhow!("找不到用户目录"))?;
        let folder = home.home_dir().join("Library/LaunchAgents");
        std::fs::create_dir_all(&folder)?;
        let path = folder.join("com.xclipboard.app.plist");
        if enabled {
            let exe = executable
                .to_string_lossy()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            std::fs::write(
                path,
                format!(
                    "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>com.xclipboard.app</string><key>ProgramArguments</key><array><string>{exe}</string><string>--background</string></array><key>RunAtLoad</key><true/></dict></plist>"
                ),
            )?;
        } else if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}
