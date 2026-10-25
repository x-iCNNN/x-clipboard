use crate::{
    model::{ClipContent, Entry, Settings},
    platform::{self, ClipboardBackend, PlatformEvent},
    preview::{self, Preview},
    storage::{HistoryStore, Usage},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

#[derive(Clone)]
pub struct Snapshot {
    pub usage: Usage,
    pub history_loaded: bool,
    pub entries: Arc<Vec<Entry>>,
    pub settings: Settings,
    pub status: String,
    pub error: Option<String>,
    pub error_expires_at: Option<std::time::Instant>,
    pub ready: bool,
    pub revision: u64,
    pub disk_bytes: u64,
    pub preview: Option<Arc<Preview>>,
    pub shutdown_complete: bool,
    pub quitting: bool,
    pub search_query: String,
    pub search_ids: Arc<std::collections::HashSet<String>>,
    pub thumbnails: Arc<std::collections::HashMap<String, Option<Arc<image::RgbaImage>>>>,
    thumbnail_requests: std::collections::HashSet<String>,
    pending_protection: std::collections::HashMap<String, (u64, bool, bool)>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            usage: Usage::default(),
            history_loaded: false,
            entries: Arc::new(vec![]),
            settings: Settings::default(),
            status: "正在解锁本地加密库…".into(),
            error: None,
            error_expires_at: None,
            ready: false,
            revision: 0,
            disk_bytes: 0,
            preview: None,
            shutdown_complete: false,
            quitting: false,
            search_query: String::new(),
            search_ids: Arc::new(Default::default()),
            thumbnails: Arc::new(Default::default()),
            thumbnail_requests: Default::default(),
            pending_protection: Default::default(),
        }
    }
}
pub enum Command {
    HistoryActivity,
    Capture(ClipContent),
    Protect(String, Option<bool>, Option<bool>),
    PersistProtection(String, bool, bool, u64),
    Delete(Vec<String>),
    Clear(bool),
    Settings(Settings),
    TogglePause,
    Write(String, bool, Option<usize>),
    Text(String, Option<usize>),
    Preview(String, u16, u64),
    Thumbnail(String),
    ThumbnailReady(String, Option<Vec<u8>>),
    Search(String, u64),
    Retry,
    Shutdown,
}
#[derive(Clone)]
pub struct Service {
    history_active: Arc<AtomicBool>,
    pub root: Arc<PathBuf>,
    tx: Sender<Command>,
    state: Arc<Mutex<Snapshot>>,
    pub backend: Arc<dyn ClipboardBackend>,
    pub ui_events: Receiver<PlatformEvent>,
    preview_generation: Arc<AtomicU64>,
    queued_bytes: Arc<AtomicUsize>,
    search_generation: Arc<AtomicU64>,
}
fn thumbnail_source(mut content: ClipContent) -> ClipContent {
    let selected = content
        .formats
        .iter()
        .position(|format| matches!(format.name.as_str(), "image/png" | "windows:PNG"))
        .or_else(|| {
            content
                .formats
                .iter()
                .position(|format| matches!(format.name.as_str(), "windows:17" | "windows:8"))
        });
    ClipContent {
        formats: selected
            .map(|index| vec![content.formats.swap_remove(index)])
            .unwrap_or_default(),
        ..Default::default()
    }
}
impl Service {
    pub fn start(root: PathBuf) -> anyhow::Result<Self> {
        let (events, event_rx) = bounded(256);
        let backend = platform::start(events)?;
        let (tx, rx) = bounded(64);
        let (ui, ui_events) = bounded(16);
        let service = Self {
            history_active: Arc::new(AtomicBool::new(false)),
            root: Arc::new(root),
            tx,
            state: Arc::new(Mutex::new(Snapshot::default())),
            backend,
            ui_events,
            preview_generation: Arc::new(AtomicU64::new(0)),
            queued_bytes: Arc::new(AtomicUsize::new(0)),
            search_generation: Arc::new(AtomicU64::new(0)),
        };
        let worker = service.clone();
        std::thread::Builder::new()
            .name("encrypted-history".into())
            .spawn(move || worker.run(rx))?;
        let capture = service.clone();
        std::thread::Builder::new()
            .name("clipboard-snapshot".into())
            .spawn(move || {
                for event in event_rx {
                    match event {
                        PlatformEvent::Changed(version) => {
                            let snapshot = capture.snapshot();
                            let settings = snapshot.settings;
                            if !snapshot.ready || settings.paused || snapshot.quitting {
                                continue;
                            }
                            match capture.backend.read(version, settings.respect_sensitive) {
                                Ok(Some(content)) => {
                                    if settings.excluded_apps.iter().any(|s| {
                                        !s.is_empty() && content.source.eq_ignore_ascii_case(s)
                                    }) {
                                        continue;
                                    }
                                    let bytes = content.bytes() as usize;
                                    if capture.queued_bytes.fetch_add(bytes, Ordering::AcqRel)
                                        + bytes
                                        > 128 * 1024 * 1024
                                    {
                                        capture.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
                                        capture.error(
                                            "保存队列达到 128 MB 上限，此次内容未保存".into(),
                                        );
                                        continue;
                                    }
                                    capture.status("保存中…");
                                    if capture.tx.try_send(Command::Capture(content)).is_err() {
                                        capture.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
                                        capture.error("保存队列已满，此次内容未保存".into());
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    capture.error(e.to_string());
                                    // A superseded notification cannot recover the old version, but still capture the current one.
                                    if let Ok(Some(content)) =
                                        capture.backend.read(0, settings.respect_sensitive)
                                        && !settings
                                            .excluded_apps
                                            .iter()
                                            .any(|s| content.source.eq_ignore_ascii_case(s))
                                    {
                                        let bytes = content.bytes() as usize;
                                        if capture.queued_bytes.fetch_add(bytes, Ordering::AcqRel)
                                            + bytes
                                            <= 128 * 1024 * 1024
                                        {
                                            if capture
                                                .tx
                                                .try_send(Command::Capture(content))
                                                .is_err()
                                            {
                                                capture
                                                    .queued_bytes
                                                    .fetch_sub(bytes, Ordering::AcqRel);
                                            }
                                        } else {
                                            capture.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
                                        }
                                    }
                                }
                            }
                        }
                        PlatformEvent::ShowQuick(target) => {
                            let _ = ui.try_send(PlatformEvent::ShowQuick(target));
                        }
                        #[cfg(windows)]
                        event @ PlatformEvent::QuickKey { .. } => {
                            let _ = ui.try_send(event);
                        }
                        PlatformEvent::ConfigureShortcut(shortcut, response) => {
                            let _ =
                                ui.try_send(PlatformEvent::ConfigureShortcut(shortcut, response));
                        }
                        PlatformEvent::Error(e) => capture.error(e),
                    }
                }
            })?;
        Ok(service)
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut state = self.state.lock().unwrap();
        if state.error_expires_at.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            state.error = None;
            state.error_expires_at = None;
            state.revision += 1;
        }
        state.clone()
    }
    pub fn set_history_active(&self, active: bool) {
        if self.history_active.swap(active, Ordering::AcqRel) == active {
            return;
        }
        if !active {
            let mut state = self.state.lock().unwrap();
            self.preview_generation.fetch_add(1, Ordering::AcqRel);
            self.search_generation.fetch_add(1, Ordering::AcqRel);
            state.entries = Arc::new(vec![]);
            state.history_loaded = false;
            state.preview = None;
            state.thumbnails = Arc::new(Default::default());
            state.thumbnail_requests.clear();
            state.search_query.clear();
            state.search_ids = Arc::new(Default::default());
            state.revision += 1;
        }
        if self.tx.try_send(Command::HistoryActivity).is_err() {
            self.history_active.store(!active, Ordering::Release);
            self.error("历史加载队列已满，请稍后重试".into());
        }
    }
    pub fn thumbnail(&self, id: &str) {
        let mut state = self.state.lock().unwrap();
        if !self.history_active.load(Ordering::Acquire) {
            return;
        }
        if state.thumbnails.contains_key(id) || !state.thumbnail_requests.insert(id.to_string()) {
            return;
        }
        if self
            .tx
            .try_send(Command::Thumbnail(id.to_string()))
            .is_err()
        {
            state.thumbnail_requests.remove(id);
        }
    }
    fn publish_thumbnail(&self, id: String, thumbnail: Option<Arc<image::RgbaImage>>) {
        let mut state = self.state.lock().unwrap();
        if !self.history_active.load(Ordering::Acquire) {
            return;
        }
        state.thumbnail_requests.remove(&id);
        if state.thumbnails.len() >= 48
            && let Some(oldest) = state.thumbnails.keys().next().cloned()
        {
            Arc::make_mut(&mut state.thumbnails).remove(&oldest);
        }
        Arc::make_mut(&mut state.thumbnails).insert(id, thumbnail);
        state.revision += 1;
    }
    pub fn send(&self, command: Command) {
        if let Command::Protect(id, pin, fav) = command {
            let mut state = self.state.lock().unwrap();
            let Some(index) = state.entries.iter().position(|entry| entry.id == id) else {
                return;
            };
            let pinned = pin.unwrap_or(state.entries[index].pinned);
            let favorite = fav.unwrap_or(state.entries[index].favorite);
            let generation = state.revision + 1;
            if self
                .tx
                .try_send(Command::PersistProtection(
                    id.clone(),
                    pinned,
                    favorite,
                    generation,
                ))
                .is_err()
            {
                drop(state);
                self.error("操作队列已满，请稍后重试".into());
                return;
            }
            let entries = Arc::make_mut(&mut state.entries);
            entries[index].pinned = pinned;
            entries[index].favorite = favorite;
            entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.order.cmp(&a.order)));
            state
                .pending_protection
                .insert(id, (generation, pinned, favorite));
            state.revision = generation;
            return;
        }
        let paste_target = match &command {
            Command::Write(_, _, target) | Command::Text(_, target) => *target,
            _ => None,
        };
        let shutdown = matches!(command, Command::Shutdown);
        if self.tx.try_send(command).is_err() {
            if paste_target.is_some() { self.paste_error("操作队列已满，请稍后重试".into()); }
            else { self.error("操作队列已满，请稍后重试".into()); }
        } else if shutdown {
            self.state.lock().unwrap().quitting = true;
        }
    }
    pub fn preview(&self, id: String, page: u16) -> u64 {
        let mut state = self.state.lock().unwrap();
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        state.preview = None;
        state.revision += 1;
        drop(state);
        self.send(Command::Preview(id, page, generation));
        generation
    }
    pub fn release_preview(&self, generation: u64) {
        let mut state = self.state.lock().unwrap();
        if self
            .preview_generation
            .compare_exchange(
                generation,
                generation + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            state.preview = None;
            state.revision += 1;
        }
    }
    pub fn search(&self, query: String) {
        let generation = self.search_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.send(Command::Search(query, generation));
    }
    pub fn error(&self, message: String) {
        let mut state = self.state.lock().unwrap();
        state.error = Some(message);
        state.error_expires_at = None;
        state.revision += 1;
    }
    fn paste_error(&self, message: String) {
        let mut state = self.state.lock().unwrap();
        state.error = Some(message);
        state.error_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
        state.revision += 1;
    }
    pub fn dismiss_error(&self) {
        let mut state = self.state.lock().unwrap();
        state.error = None;
        state.error_expires_at = None;
        state.revision += 1;
    }
    fn status(&self, message: &str) {
        let mut state = self.state.lock().unwrap();
        state.status = message.into();
        state.revision += 1;
    }
    fn refresh(&self, store: &HistoryStore) -> anyhow::Result<()> {
        let entries = if self.history_active.load(Ordering::Acquire) {
            Some(store.list()?)
        } else {
            None
        };
        let usage = store.usage()?;
        let settings = store.settings()?;
        let disk = store.disk_bytes();
        let query = self.snapshot().search_query;
        let matches = if query.is_empty() || !self.history_active.load(Ordering::Acquire) {
            None
        } else {
            Some(Arc::new(store.search(&query)?))
        };
        let mut state = self.state.lock().unwrap();
        let loaded = self.history_active.load(Ordering::Acquire) && entries.is_some();
        let mut entries = if loaded { entries.unwrap() } else { vec![] };
        for entry in &mut entries {
            if let Some((_, pinned, favorite)) = state.pending_protection.get(&entry.id) {
                entry.pinned = *pinned;
                entry.favorite = *favorite;
            }
        }
        entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.order.cmp(&a.order)));
        state.entries = Arc::new(entries);
        state.usage = usage;
        state.history_loaded = loaded;
        state.settings = settings;
        state.disk_bytes = disk;
        if let Some(ids) = matches {
            state.search_ids = ids;
        }
        state.ready = true;
        state.status = if state.settings.paused {
            "已暂停记录"
        } else {
            "正在记录 · 已保存"
        }
        .into();
        if usage.entries > state.settings.max_entries || usage.bytes > state.settings.max_bytes {
            state.status = "正在记录 · 置顶/收藏已超过容量限制".into();
        }
        state.revision += 1;
        Ok(())
    }
    fn run(self, rx: Receiver<Command>) {
        let mut store = HistoryStore::open(&self.root);
        if let Ok(ref db) = store {
            if let Err(e) = self
                .refresh(db)
                .and_then(|_| self.backend.shortcut(&self.snapshot().settings.shortcut))
            {
                self.error(e.to_string());
            }
        } else if let Err(ref e) = store {
            self.error(format!("本地数据未解锁：{e:#}"));
        }
        let (preview_tx, preview_rx) = bounded::<(String, ClipContent, u16, u64)>(1);
        let pending_preview = preview_rx.clone();
        let previews = self.clone();
        std::thread::spawn(move || {
            for (id, content, page, generation) in preview_rx {
                if previews.preview_generation.load(Ordering::Acquire) != generation
                    || !previews.history_active.load(Ordering::Acquire)
                {
                    continue;
                }
                let preview = preview::generate(id, content, page);
                let mut state = previews.state.lock().unwrap();
                if previews.preview_generation.load(Ordering::Acquire) == generation
                    && previews.history_active.load(Ordering::Acquire)
                {
                    state.preview = Some(Arc::new(preview));
                    state.revision += 1;
                }
            }
        });
        let (thumbnail_tx, thumbnail_rx) = bounded::<(String, ClipContent)>(1);
        let thumbnails = self.clone();
        std::thread::spawn(move || {
            for (id, content) in thumbnail_rx {
                let png = preview::thumbnail(&content)
                    .and_then(|image| preview::encode_thumbnail(&image))
                    .ok();
                drop(content);
                if thumbnails
                    .tx
                    .send(Command::ThumbnailReady(id, png))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut generating_thumbnails = std::collections::HashSet::new();
        let mut failed_capture = None;
        for command in rx {
            let protection = match &command {
                Command::PersistProtection(id, _, _, generation) => Some((id.clone(), *generation)),
                _ => None,
            };
            let paste_target = match &command {
                Command::Write(_, _, target) | Command::Text(_, target) => *target,
                _ => None,
            };
            let notice = match &command {
                Command::Settings(_) => Some("设置已保存"),
                Command::Write(_, _, None) | Command::Text(_, None) => Some("已复制到系统剪贴板"),
                Command::Write(_, _, Some(_)) | Command::Text(_, Some(_)) => {
                    Some("已向原窗口发送粘贴")
                }
                _ => None,
            };
            if matches!(command, Command::Shutdown) {
                self.backend.shutdown();
                if let Ok(db) = &store
                    && let Err(e) = db.checkpoint()
                {
                    self.error(format!("退出前检查点失败：{e}"));
                }
                self.state.lock().unwrap().shutdown_complete = true;
                break;
            }
            if matches!(command, Command::Retry) {
                if store.is_err() {
                    store = HistoryStore::open(&self.root);
                }
                if let Some(content) = failed_capture.take() {
                    self.send(Command::Capture(content));
                }
            }
            let db = match store.as_ref() {
                Ok(db) => db,
                Err(e) => {
                    if let Command::Capture(content) = command {
                        self.queued_bytes
                            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                                Some(n.saturating_sub(content.bytes() as usize))
                            })
                            .ok();
                        failed_capture = Some(content);
                    }
                    if paste_target.is_some() { self.paste_error(format!("密钥或存储不可用：{e:#}")); }
                    else { self.error(format!("密钥或存储不可用：{e:#}")); }
                    continue;
                }
            };
            let result = (|| -> anyhow::Result<()> {
                match command {
                    Command::HistoryActivity => {
                        self.refresh(db)?;
                        return Ok(());
                    }
                    Command::Capture(content) => {
                        self.queued_bytes
                            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                                Some(n.saturating_sub(content.bytes() as usize))
                            })
                            .ok();
                        let settings = db.settings()?;
                        if settings.paused {
                            return Ok(());
                        }
                        match db.capture(&content, &settings) {
                            Ok(entry) => {
                                if entry.kind == "image"
                                    && !generating_thumbnails.contains(&entry.id)
                                    && !db.thumbnail(&entry.id).is_ok_and(|cached| cached.is_some())
                                    && thumbnail_tx
                                        .try_send((entry.id.clone(), thumbnail_source(content)))
                                        .is_ok()
                                {
                                    generating_thumbnails.insert(entry.id);
                                }
                            }
                            Err(error) => {
                                failed_capture = Some(content);
                                return Err(error);
                            }
                        }
                    }
                    Command::Protect(..) => {
                        unreachable!("protection must enter through Service::send")
                    }
                    Command::PersistProtection(id, pin, fav, generation) => {
                        db.protect(&id, Some(pin), Some(fav))?;
                        {
                            let mut state = self.state.lock().unwrap();
                            if state
                                .pending_protection
                                .get(&id)
                                .is_some_and(|pending| pending.0 == generation)
                            {
                                state.pending_protection.remove(&id);
                            }
                        }
                        let snapshot = self.snapshot();
                        if snapshot.usage.entries > snapshot.settings.max_entries
                            || snapshot.usage.bytes > snapshot.settings.max_bytes
                        {
                            db.prune(&snapshot.settings)?;
                        } else {
                            let disk = db.disk_bytes();
                            let usage = db.usage()?;
                            let mut state = self.state.lock().unwrap();
                            state.usage = usage;
                            if state.entries.iter().any(|item| item.id == id) {
                                state.disk_bytes = disk;
                                state.revision += 1;
                                return Ok(());
                            }
                        }
                    }
                    Command::Delete(ids) => {
                        db.delete(&ids)?;
                        let mut state = self.state.lock().unwrap();
                        for id in &ids {
                            Arc::make_mut(&mut state.thumbnails).remove(id);
                            state.thumbnail_requests.remove(id);
                        }
                    }
                    Command::Clear(all) => {
                        db.clear(all)?;
                        let mut state = self.state.lock().unwrap();
                        state.thumbnails = Arc::new(Default::default());
                        state.thumbnail_requests.clear();
                    }
                    Command::Settings(settings) => {
                        settings.validate()?;
                        let old = db.settings()?;
                        if old.shortcut != settings.shortcut {
                            self.backend.shortcut(&settings.shortcut)?;
                        }
                        if old.start_at_login != settings.start_at_login
                            && let Err(error) = platform::set_startup(settings.start_at_login)
                        {
                            if old.shortcut != settings.shortcut {
                                let _ = self.backend.shortcut(&old.shortcut);
                            }
                            return Err(error);
                        }
                        if let Err(e) = db.save_settings(&settings) {
                            if old.shortcut != settings.shortcut {
                                let _ = self.backend.shortcut(&old.shortcut);
                            }
                            if old.start_at_login != settings.start_at_login {
                                let _ = platform::set_startup(old.start_at_login);
                            }
                            return Err(e);
                        }
                        db.prune(&settings)?;
                    }
                    Command::TogglePause => {
                        let mut settings = db.settings()?;
                        settings.paused = !settings.paused;
                        db.save_settings(&settings)?;
                        let disk = db.disk_bytes();
                        let mut state = self.state.lock().unwrap();
                        state.disk_bytes = disk;
                        state.settings = settings;
                        state.status = if state.settings.paused {
                            "已暂停记录"
                        } else if state.usage.entries > state.settings.max_entries
                            || state.usage.bytes > state.settings.max_bytes
                        {
                            "正在记录 · 置顶/收藏已超过容量限制"
                        } else {
                            "正在记录 · 已保存"
                        }
                        .into();
                        state.revision += 1;
                        return Ok(());
                    }
                    Command::Write(id, plain, target) => {
                        let content = db.content(&id)?;
                        self.backend.write(&content, plain)?;
                        if let Some(target) = target {
                            self.backend.paste(target)?;
                        }
                    }
                    Command::Text(text, target) => {
                        self.backend.write(&ClipContent::text(text), true)?;
                        if let Some(target) = target {
                            self.backend.paste(target)?;
                        }
                    }
                    Command::Preview(id, page, generation) => {
                        if self.preview_generation.load(Ordering::Acquire) != generation {
                            return Ok(());
                        }
                        let content = db.content(&id)?;
                        match preview_tx.try_send((id, content, page, generation)) {
                            Ok(()) => {}
                            Err(crossbeam_channel::TrySendError::Full(job)) => {
                                let _ = pending_preview.try_recv();
                                preview_tx
                                    .try_send(job)
                                    .map_err(|_| anyhow::anyhow!("预览服务不可用"))?;
                            }
                            Err(_) => anyhow::bail!("预览服务不可用"),
                        }
                        return Ok(());
                    }
                    Command::Thumbnail(id) => {
                        if !self.history_active.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        if let Some(png) = db.thumbnail(&id)?
                            && let Ok(image) = preview::decode_thumbnail(&png)
                        {
                            self.publish_thumbnail(id, Some(Arc::new(image)));
                            return Ok(());
                        }
                        if generating_thumbnails.contains(&id) {
                            return Ok(());
                        }
                        if thumbnail_tx.is_full() {
                            self.state.lock().unwrap().thumbnail_requests.remove(&id);
                            return Ok(());
                        }
                        match db.content(&id) {
                            Ok(content) => {
                                if thumbnail_tx
                                    .try_send((id.clone(), thumbnail_source(content)))
                                    .is_ok()
                                {
                                    generating_thumbnails.insert(id);
                                } else {
                                    self.state.lock().unwrap().thumbnail_requests.remove(&id);
                                }
                            }
                            Err(_) => self.publish_thumbnail(id, None),
                        }
                        return Ok(());
                    }
                    Command::ThumbnailReady(id, png) => {
                        generating_thumbnails.remove(&id);
                        let image = if let Some(png) = png {
                            let saved = match db.save_thumbnail(&id, &png) {
                                Ok(saved) => saved,
                                Err(error) => {
                                    self.publish_thumbnail(
                                        id,
                                        preview::decode_thumbnail(&png).ok().map(Arc::new),
                                    );
                                    return Err(error.context("缩略图缓存写入失败"));
                                }
                            };
                            if !saved {
                                self.state.lock().unwrap().thumbnail_requests.remove(&id);
                                return Ok(());
                            }
                            if self.history_active.load(Ordering::Acquire) {
                                preview::decode_thumbnail(&png).ok().map(Arc::new)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let disk = db.disk_bytes();
                        {
                            let mut state = self.state.lock().unwrap();
                            state.disk_bytes = disk;
                            state.revision += 1;
                        }
                        self.publish_thumbnail(id, image);
                        return Ok(());
                    }
                    Command::Search(query, generation) => {
                        if self.search_generation.load(Ordering::Acquire) == generation {
                            let ids = db.search(&query)?;
                            let mut state = self.state.lock().unwrap();
                            if self.search_generation.load(Ordering::Acquire) != generation
                                || !self.history_active.load(Ordering::Acquire)
                            {
                                return Ok(());
                            }
                            state.search_query = query.to_lowercase();
                            state.search_ids = Arc::new(ids);
                            state.revision += 1;
                        }
                        return Ok(());
                    }
                    Command::Retry => {}
                    Command::Shutdown => unreachable!(),
                };
                self.refresh(db)?;
                Ok(())
            })();
            if let Err(e) = result {
                if let Some((id, generation)) = protection {
                    let mut state = self.state.lock().unwrap();
                    if state
                        .pending_protection
                        .get(&id)
                        .is_some_and(|pending| pending.0 == generation)
                    {
                        state.pending_protection.remove(&id);
                    }
                    drop(state);
                    let _ = self.refresh(db);
                }
                if paste_target.is_some() { self.paste_error(format!("操作未完成：{e:#}")); }
                else { self.error(format!("操作未完成：{e:#}")); }
            } else if let Some(notice) = notice {
                self.status(notice);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Backend;
    impl ClipboardBackend for Backend {
        fn read(&self, _: u32, _: bool) -> anyhow::Result<Option<ClipContent>> {
            Ok(None)
        }
        fn write(&self, _: &ClipContent, _: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn shortcut(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn paste(&self, target: usize) -> anyhow::Result<()> {
            if target == usize::MAX - 1 { anyhow::bail!("synthetic invalid transient target"); }
            if target == usize::MAX {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Ok(())
        }
        fn capabilities(&self) -> &'static str {
            "test backend"
        }
    }
    fn wait(service: &Service, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
        for _ in 0..600 {
            let snapshot = service.snapshot();
            if condition(&snapshot) {
                return snapshot;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("history service did not reach the expected state");
    }
    #[test]
    fn dormant_history_is_unloaded_but_capture_and_reopen_work() {
        let root = tempfile::tempdir().unwrap();
        {
            let store = HistoryStore::open(root.path()).unwrap();
            store
                .capture(
                    &ClipContent::text("synthetic existing record"),
                    &Settings::default(),
                )
                .unwrap();
        }
        let (tx, rx) = bounded(64);
        let (_ui_tx, ui_events) = bounded(16);
        let service = Service {
            history_active: Arc::new(AtomicBool::new(false)),
            root: Arc::new(root.path().to_path_buf()),
            tx,
            state: Arc::new(Mutex::new(Snapshot::default())),
            backend: Arc::new(Backend),
            ui_events,
            preview_generation: Arc::new(AtomicU64::new(0)),
            queued_bytes: Arc::new(AtomicUsize::new(0)),
            search_generation: Arc::new(AtomicU64::new(0)),
        };
        let worker = service.clone();
        let thread = std::thread::spawn(move || worker.run(rx));
        let state = wait(&service, |state| state.ready);
        assert!(
            state.entries.is_empty(),
            "background startup must not load historical rows"
        );
        let revision = state.revision;
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(512, 256)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        service.send(Command::Capture(ClipContent {
            formats: vec![crate::model::FormatData {
                name: "image/png".into(),
                bytes: png.into_inner(),
            }],
            ..Default::default()
        }));
        let saved = wait(&service, |state| {
            state.usage.entries == 2 && state.revision >= revision + 2
        });
        assert!(saved.error.is_none() && saved.entries.is_empty() && saved.thumbnails.is_empty());
        service.set_history_active(true);
        let loaded = wait(&service, |state| state.entries.len() == 2);
        let image_id = loaded
            .entries
            .iter()
            .find(|entry| entry.kind == "image")
            .unwrap()
            .id
            .clone();
        service.thumbnail(&image_id);
        wait(&service, |state| {
            state
                .thumbnails
                .get(&image_id)
                .is_some_and(|image| image.is_some())
        });
        let id = loaded
            .entries
            .iter()
            .find(|entry| entry.kind == "text")
            .unwrap()
            .id
            .clone();
        drop(loaded);
        service.send(Command::Protect(id, Some(true), Some(true)));
        let protected = service.snapshot();
        let weak = Arc::downgrade(&protected.entries);
        drop(protected);
        service.set_history_active(false);
        wait(&service, |state| {
            state.entries.is_empty() && !state.history_loaded
        });
        assert!(
            weak.upgrade().is_none(),
            "dormancy must release the historical snapshot"
        );
        service.send(Command::Capture(ClipContent::text(
            "synthetic background capture",
        )));
        service.set_history_active(true);
        let reopened = wait(&service, |state| state.entries.len() == 3);
        assert!(
            reopened
                .entries
                .iter()
                .any(|entry| entry.pinned && entry.favorite)
        );
        service.send(Command::Shutdown);
        wait(&service, |state| state.shutdown_complete);
        thread.join().unwrap();
    }
    #[test]
    fn queued_pause_and_protection_commands_preserve_latest_state() {
        let root = tempfile::tempdir().unwrap();
        let (tx, rx) = bounded(64);
        let (_ui_tx, ui_events) = bounded(16);
        let service = Service {
            history_active: Arc::new(AtomicBool::new(true)),
            root: Arc::new(root.path().to_path_buf()),
            tx,
            state: Arc::new(Mutex::new(Snapshot::default())),
            backend: Arc::new(Backend),
            ui_events,
            preview_generation: Arc::new(AtomicU64::new(0)),
            queued_bytes: Arc::new(AtomicUsize::new(0)),
            search_generation: Arc::new(AtomicU64::new(0)),
        };
        let worker = service.clone();
        let thread = std::thread::spawn(move || worker.run(rx));
        wait(&service, |state| state.ready);
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(512, 256)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        service.send(Command::Capture(ClipContent {
            formats: vec![crate::model::FormatData {
                name: "image/png".into(),
                bytes: png.into_inner(),
            }],
            ..Default::default()
        }));
        let image_state = wait(&service, |state| state.entries.len() == 1);
        let image_id = image_state.entries[0].id.clone();
        service.thumbnail(&image_id);
        let image_state = wait(&service, |state| {
            state
                .thumbnails
                .get(&image_id)
                .is_some_and(|image| image.is_some())
        });
        let pixels = image_state.thumbnails[&image_id].as_ref().unwrap();
        assert_eq!((pixels.width(), pixels.height()), (96, 48));
        let expected_pixels = pixels.clone();
        service.set_history_active(false);
        wait(&service, |state| {
            !state.history_loaded && state.thumbnails.is_empty()
        });
        service.set_history_active(true);
        wait(&service, |state| state.history_loaded);
        service.thumbnail(&image_id);
        let reloaded = wait(&service, |state| {
            state
                .thumbnails
                .get(&image_id)
                .is_some_and(|image| image.is_some())
        });
        assert_eq!(
            reloaded.thumbnails[&image_id].as_ref().unwrap().as_ref(),
            expected_pixels.as_ref()
        );
        service.send(Command::Delete(vec![image_id]));
        wait(&service, |state| state.entries.is_empty());
        service.send(Command::Capture(ClipContent::text(
            "synthetic protected record",
        )));
        let state = wait(&service, |state| state.entries.len() == 1);
        let id = state.entries[0].id.clone();
        service.send(Command::Text(
            "synthetic slow target".into(),
            Some(usize::MAX),
        ));
        let started = std::time::Instant::now();
        service.send(Command::Protect(id.clone(), Some(true), None));
        assert!(
            service.snapshot().entries[0].pinned,
            "protection feedback must not wait for the worker queue"
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        service.send(Command::Protect(id.clone(), Some(false), Some(true)));
        assert!(!service.snapshot().entries[0].pinned && service.snapshot().entries[0].favorite);
        service.send(Command::Protect(id.clone(), None, Some(false)));
        assert!(!service.snapshot().entries[0].favorite);
        let generation = service.preview(id.clone(), 0);
        let preview_state = wait(&service, |state| state.preview.is_some());
        let weak = Arc::downgrade(preview_state.preview.as_ref().unwrap());
        drop(preview_state);
        service.release_preview(generation);
        assert!(service.snapshot().preview.is_none());
        assert!(weak.upgrade().is_none(), "closed preview must be freed");
        let next_generation = service.preview(id.clone(), 0);
        wait(&service, |state| state.preview.is_some());
        service.release_preview(generation);
        assert!(
            service.snapshot().preview.is_some(),
            "old window cannot release a newer preview"
        );
        service.release_preview(next_generation);
        let cancelled = service.preview(id.clone(), 0);
        service.release_preview(cancelled);
        service.send(Command::Protect(id.clone(), Some(true), None));
        service.send(Command::Protect(id.clone(), None, Some(true)));
        service.send(Command::Protect(id, Some(false), None));
        wait(&service, |state| {
            state.entries[0].favorite && !state.entries[0].pinned
        });
        assert!(
            service.snapshot().preview.is_none(),
            "cancelled work must not republish a preview"
        );
        service.send(Command::TogglePause);
        wait(&service, |state| state.settings.paused);
        service.send(Command::Capture(ClipContent::text(
            "must not be stored while paused",
        )));
        service.send(Command::TogglePause);
        let resumed = wait(&service, |state| !state.settings.paused);
        assert_eq!(resumed.entries.len(), 1);
        let revision = resumed.revision;
        for index in 0..50 {
            service.thumbnail(&format!("missing-thumbnail-{index}"));
        }
        let thumbnails = wait(&service, |state| {
            state.thumbnails.contains_key("missing-thumbnail-49")
        });
        assert_eq!(thumbnails.thumbnails.len(), 48);
        service.send(Command::Text("synthetic failed paste".into(), Some(usize::MAX - 1)));
        let failed = wait(&service, |state| state.error.is_some());
        assert!(service.ui_events.try_recv().is_err(), "paste failure must not reopen the quick panel");
        assert!(failed.error_expires_at.is_some());
        service.state.lock().unwrap().error_expires_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        assert!(service.snapshot().error.is_none(), "transient paste notice must expire");
        service.send(Command::TogglePause);
        service.send(Command::TogglePause);
        wait(&service, |state| {
            state.revision >= revision + 2 && !state.settings.paused
        });
        service.send(Command::Shutdown);
        wait(&service, |state| state.shutdown_complete);
        thread.join().unwrap();
        let store = HistoryStore::open(root.path()).unwrap();
        assert!(!store.settings().unwrap().paused);
        let entries = store.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].favorite && !entries[0].pinned);
    }
}
