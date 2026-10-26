use crate::model::ClipContent;
use anyhow::Result;
use crossbeam_channel::Sender;

#[derive(Clone, Debug)]
pub enum PlatformEvent {
    Changed(u32),
    ShowQuick(usize),
    #[cfg(windows)]
    QuickKey {
        key: &'static str,
        shift: bool,
    },
    Error(String),
    ConfigureShortcut(String, Sender<std::result::Result<(), String>>),
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use unix::*;

pub trait ClipboardBackend: Send + Sync {
    fn read(&self, version: u32, respect_sensitive: bool) -> Result<Option<ClipContent>>;
    fn write(&self, content: &ClipContent, plain: bool) -> Result<()>;
    fn shortcut(&self, shortcut: &str) -> Result<()>;
    fn middle_click(&self, enabled: bool) -> Result<()> {
        anyhow::ensure!(!enabled, "此平台暂不支持鼠标中键激活");
        Ok(())
    }
    fn paste(&self, target: usize) -> Result<()>;
    fn paste_using(&self, target: usize, shortcut: &str) -> Result<()> {
        anyhow::ensure!(shortcut == "Ctrl+V", "此平台暂不支持此回贴按键");
        self.paste(target)
    }
    fn capabilities(&self) -> &'static str;
    fn shutdown(&self) {}
}

pub fn start(events: Sender<PlatformEvent>) -> Result<std::sync::Arc<dyn ClipboardBackend>> {
    Ok(std::sync::Arc::new(PlatformClipboard::start(events)?))
}
