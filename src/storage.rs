use crate::model::{ClipContent, Entry, Settings, now_ms};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Usage {
    pub entries: usize,
    pub bytes: u64,
    pub pinned: usize,
    pub favorites: usize,
}

pub struct HistoryStore {
    db: std::sync::Arc<turso::Database>,
    pub(crate) write_gate: std::sync::Arc<std::sync::Mutex<()>>,
    pub(crate) connection: turso::Connection,
    root: PathBuf,
}

impl HistoryStore {
    pub fn open(root: &Path) -> Result<Self> {
        let key = crate::vault::database_key(root)?;
        Self::open_with_key(root, &key)
    }
    pub fn open_with_key(root: &Path, key: &[u8]) -> Result<Self> {
        anyhow::ensure!(key.len() == 32, "数据库密钥长度无效");
        std::fs::create_dir_all(root)?;
        let db = pollster::block_on(
            turso::Builder::new_local(
                root.join("clipboard.db")
                    .to_str()
                    .context("数据目录路径无效")?,
            )
            .experimental_encryption(true)
            .with_encryption(turso::EncryptionOpts {
                cipher: "aes256gcm".into(),
                hexkey: key.iter().map(|b| format!("{b:02x}")).collect(),
            })
            .build(),
        )
        .context("无法打开加密数据库")?;
        let connection = db.connect()?;
        crate::sync_store::initialize(&connection)?;
        pollster::block_on(async {
            connection.execute("PRAGMA synchronous=FULL", ()).await?;
            connection.execute("CREATE TABLE IF NOT EXISTS clips(id TEXT PRIMARY KEY, hash TEXT NOT NULL UNIQUE, metadata BLOB NOT NULL, payload BLOB NOT NULL, ordinal INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, favorite INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL)",()).await?;
            connection
                .execute(
                    "CREATE INDEX IF NOT EXISTS clips_order ON clips(pinned DESC,ordinal DESC)",
                    (),
                )
                .await?;
            connection.execute("CREATE INDEX IF NOT EXISTS clips_retention ON clips(pinned,favorite,ordinal,bytes,id)", ()).await?;
            connection.execute("CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL)",()).await?;
            connection.execute("CREATE TABLE IF NOT EXISTS clip_search(id TEXT PRIMARY KEY, searchable TEXT NOT NULL)",()).await?;
            connection.execute("CREATE TABLE IF NOT EXISTS clip_thumbnails(id TEXT PRIMARY KEY, png BLOB NOT NULL)", ()).await?;
            let mut missing = connection
                .query(
                    "SELECT id,payload FROM clips WHERE id NOT IN (SELECT id FROM clip_search)",
                    (),
                )
                .await?;
            let mut backfill = vec![];
            while let Some(row) = missing.next().await? {
                backfill.push((
                    row.get::<String>(0)?,
                    postcard::from_bytes::<ClipContent>(&row.get::<Vec<u8>>(1)?)?,
                ));
            }
            drop(missing);
            for (id, content) in backfill {
                connection
                    .execute(
                        "INSERT INTO clip_search(id,searchable) VALUES(?1,?2)",
                        [id, searchable(&content)],
                    )
                    .await?;
            }
            anyhow::Ok(())
        })?;
        Ok(Self {
            db: std::sync::Arc::new(db),
            write_gate: Default::default(),
            connection,
            root: root.to_path_buf(),
        })
    }
    pub(crate) fn fork(&self) -> Result<Self> {
        Ok(Self {
            db: self.db.clone(),
            write_gate: self.write_gate.clone(),
            connection: self.db.connect()?,
            root: self.root.clone(),
        })
    }
    pub fn settings(&self) -> Result<Settings> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT value FROM settings WHERE id=1", ())
                .await?;
            match rows.next().await? {
                Some(r) => Ok(serde_json::from_str(&r.get::<String>(0)?)?),
                None => Ok(Settings::default()),
            }
        })
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        let _writer = self.write_gate.lock().unwrap();
        pollster::block_on(self.connection.execute("INSERT INTO settings(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[serde_json::to_string(settings)?]))?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Entry>> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query(
                    "SELECT metadata FROM clips ORDER BY pinned DESC,ordinal DESC",
                    (),
                )
                .await?;
            let mut entries = Vec::new();
            while let Some(row) = rows.next().await? {
                entries.push(postcard::from_bytes(&row.get::<Vec<u8>>(0)?)?);
            }
            Ok(entries)
        })
    }
    pub fn usage(&self) -> Result<Usage> {
        pollster::block_on(async {
            let mut rows = self.connection.query("SELECT COUNT(*),COALESCE(SUM(bytes),0),COALESCE(SUM(pinned),0),COALESCE(SUM(favorite),0) FROM clips INDEXED BY clips_retention", ()).await?;
            let row = rows.next().await?.context("无法读取历史统计")?;
            Ok(Usage {
                entries: row.get::<i64>(0)? as usize,
                bytes: row.get::<i64>(1)? as u64,
                pinned: row.get::<i64>(2)? as usize,
                favorites: row.get::<i64>(3)? as usize,
            })
        })
    }
    pub fn content(&self, id: &str) -> Result<ClipContent> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT payload FROM clips WHERE id=?1", [id])
                .await?;
            let row = rows.next().await?.context("记录已被删除")?;
            Ok(postcard::from_bytes(&row.get::<Vec<u8>>(0)?)?)
        })
    }
    pub fn thumbnail(&self, id: &str) -> Result<Option<Vec<u8>>> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query(
                    "SELECT png FROM clip_thumbnails WHERE id=?1 AND length(png)<=?2",
                    turso::params![id, crate::model::MAX_THUMBNAIL_BYTES as i64],
                )
                .await?;
            rows.next()
                .await?
                .map(|row| row.get::<Vec<u8>>(0))
                .transpose()
                .map_err(Into::into)
        })
    }
    pub fn save_thumbnail(&self, id: &str, png: &[u8]) -> Result<bool> {
        let _writer = self.write_gate.lock().unwrap();
        anyhow::ensure!(
            png.len() <= crate::model::MAX_THUMBNAIL_BYTES,
            "缩略图数据超出限制"
        );
        let changed = pollster::block_on(self.connection.execute(
            "INSERT INTO clip_thumbnails(id,png) SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM clips WHERE id=?1) ON CONFLICT(id) DO UPDATE SET png=excluded.png",
            turso::params![id, png.to_vec()],
        ))?;
        Ok(changed > 0)
    }
    pub fn search(&self, query: &str) -> Result<std::collections::HashSet<String>> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query(
                    "SELECT id FROM clip_search WHERE instr(searchable,?1)>0",
                    [query.to_lowercase()],
                )
                .await?;
            let mut ids = std::collections::HashSet::new();
            while let Some(row) = rows.next().await? {
                ids.insert(row.get::<String>(0)?);
            }
            Ok(ids)
        })
    }
    pub fn capture(&self, content: &ClipContent, settings: &Settings) -> Result<Entry> {
        content.validate()?;
        let hash = content.fingerprint();
        let entry = self.transaction(|| pollster::block_on(async {
                let mut rows=self.connection.query("SELECT metadata FROM clips WHERE hash=?1",[hash.clone()]).await?;
                let previous=rows.next().await?.map(|r|r.get::<Vec<u8>>(0)).transpose()?;drop(rows);
                let mut order=self.connection.query("SELECT COALESCE(MAX(ordinal),0)+1 FROM clips",()).await?;
                let ordinal=order.next().await?.context("无法读取排序序号")?.get::<i64>(0)?;drop(order);
                let mut entry=if let Some(bytes)=previous { postcard::from_bytes::<Entry>(&bytes)? } else {Entry{id:uuid::Uuid::new_v4().to_string(),fingerprint:hash.clone(),title:content.title(),text:content.text.chars().take(512).collect(),kind:content.kind().into(),source:content.source.clone(),copied_at:0,order:0,pinned:false,favorite:false,bytes:content.bytes()}};
                entry.copied_at=now_ms();entry.order=ordinal;entry.source=content.source.clone();
                self.connection.execute("INSERT INTO clips(id,hash,metadata,payload,ordinal,pinned,favorite,bytes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(hash) DO UPDATE SET metadata=excluded.metadata,ordinal=excluded.ordinal",turso::params![entry.id.clone(),hash,postcard::to_stdvec(&entry)?,postcard::to_stdvec(content)?,ordinal,entry.pinned as i64,entry.favorite as i64,entry.bytes as i64]).await?;
                self.connection.execute("INSERT INTO clip_search(id,searchable) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET searchable=excluded.searchable",[entry.id.clone(),searchable(content)]).await?;
                self.sync_capture(&entry).await?;
                anyhow::Ok(entry)
        }))?;
        self.prune(settings)?;
        Ok(entry)
    }
    pub fn protect(&self, id: &str, pinned: Option<bool>, favorite: Option<bool>) -> Result<Entry> {
        self.transaction(|| self.protect_inner(id, pinned, favorite))
    }
    fn protect_inner(
        &self,
        id: &str,
        pinned: Option<bool>,
        favorite: Option<bool>,
    ) -> Result<Entry> {
        let mut entry: Entry = pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT metadata FROM clips WHERE id=?1", [id])
                .await?;
            let row = rows.next().await?.context("记录已被删除")?;
            anyhow::Ok(postcard::from_bytes(&row.get::<Vec<u8>>(0)?)?)
        })?;
        let pin_changed = pinned.is_some_and(|value| value != entry.pinned);
        let favorite_changed = favorite.is_some_and(|value| value != entry.favorite);
        if let Some(value) = pinned {
            entry.pinned = value;
        }
        if let Some(value) = favorite {
            entry.favorite = value;
        }
        pollster::block_on(self.connection.execute(
            "UPDATE clips SET metadata=?1,pinned=?2,favorite=?3 WHERE id=?4",
            turso::params![
                postcard::to_stdvec(&entry)?,
                entry.pinned as i64,
                entry.favorite as i64,
                id
            ],
        ))?;
        pollster::block_on(self.sync_protect(&entry, pin_changed, favorite_changed))?;
        Ok(entry)
    }
    pub fn delete(&self, ids: &[String]) -> Result<()> {
        self.delete_inner(ids, true)
    }
    fn delete_inner(&self, ids: &[String], explicit: bool) -> Result<()> {
        let _writer = self.write_gate.lock().unwrap();
        pollster::block_on(async {
            self.connection.execute("BEGIN IMMEDIATE", ()).await?;
            if let Err(error) = self.sync_remove(ids, explicit).await {
                let _ = self.connection.execute("ROLLBACK", ()).await;
                return Err(error);
            }
            for id in ids {
                if let Err(e) = self
                    .connection
                    .execute("DELETE FROM clips WHERE id=?1", [id.as_str()])
                    .await
                {
                    let _ = self.connection.execute("ROLLBACK", ()).await;
                    return Err(e.into());
                }
            }
            for cleanup in [
                "DELETE FROM clip_search WHERE id NOT IN (SELECT id FROM clips)",
                "DELETE FROM clip_thumbnails WHERE id NOT IN (SELECT id FROM clips)",
            ] {
                if let Err(e) = self.connection.execute(cleanup, ()).await {
                    let _ = self.connection.execute("ROLLBACK", ()).await;
                    return Err(e.into());
                }
            }
            if let Err(e) = self.connection.execute("COMMIT", ()).await {
                let _ = self.connection.execute("ROLLBACK", ()).await;
                return Err(e.into());
            }
            Ok(())
        })
    }
    pub fn clear(&self, include_protected: bool) -> Result<()> {
        let ids = self
            .list()?
            .into_iter()
            .filter(|entry| include_protected || (!entry.pinned && !entry.favorite))
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        self.delete(&ids)
    }
    pub fn prune(&self, settings: &Settings) -> Result<usize> {
        let usage = self.usage()?;
        if usage.entries <= settings.max_entries && usage.bytes <= settings.max_bytes {
            return Ok(0);
        }
        let remove = pollster::block_on(async {
            let (mut count, mut bytes) = (usage.entries, usage.bytes);
            let mut rows = self
                .connection
                .query(
                    "SELECT id,bytes FROM clips INDEXED BY clips_retention WHERE pinned=0 AND favorite=0 ORDER BY ordinal ASC",
                    (),
                )
                .await?;
            let mut remove = vec![];
            while count > settings.max_entries || bytes > settings.max_bytes {
                let Some(row) = rows.next().await? else {
                    break;
                };
                remove.push(row.get::<String>(0)?);
                count -= 1;
                bytes = bytes.saturating_sub(row.get::<i64>(1)? as u64);
            }
            anyhow::Ok(remove)
        })?;
        if !remove.is_empty() {
            self.delete_inner(&remove, false)?;
        }
        Ok(remove.len())
    }
    pub fn disk_bytes(&self) -> u64 {
        ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"]
            .iter()
            .filter_map(|p| std::fs::metadata(self.root.join(p)).ok())
            .map(|m| m.len())
            .sum()
    }
    pub fn checkpoint(&self) -> Result<()> {
        let _writer = self.write_gate.lock().unwrap();
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
                .await?;
            while rows.next().await?.is_some() {}
            anyhow::Ok(())
        })
    }
}
pub(crate) fn searchable(content: &ClipContent) -> String {
    format!(
        "{}\n{}\n{}",
        content.text,
        content.files.join("\n"),
        content.source
    )
    .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image_content(color: u8) -> ClipContent {
        let image = image::RgbaImage::from_pixel(512, 256, image::Rgba([color, 25, 75, 127]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        ClipContent {
            formats: vec![crate::model::FormatData {
                name: "image/png".into(),
                bytes: bytes.into_inner(),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn thumbnail_cache_reopens_encrypted_and_is_independent_of_original_payload() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let key = [37; 32];
        let store = HistoryStore::open_with_key(dir.path(), &key)?;
        let content = image_content(50);
        let item = store.capture(&content, &Settings::default())?;
        assert!(store.thumbnail(&item.id)?.is_none());
        let png = crate::preview::encode_thumbnail(&crate::preview::thumbnail(&content)?)?;
        assert!(store.save_thumbnail(&item.id, &png)?);
        assert_eq!(
            store.content(&item.id)?.formats[0].bytes,
            content.formats[0].bytes
        );
        let duplicate = store.capture(&content, &Settings::default())?;
        assert_eq!(duplicate.id, item.id);
        assert_eq!(store.thumbnail(&item.id)?.as_ref(), Some(&png));
        store.protect(&item.id, Some(true), None)?;
        let ordinary = store.capture(&image_content(90), &Settings::default())?;
        assert!(store.save_thumbnail(&ordinary.id, &png)?);
        for file in ["clipboard.db", "clipboard.db-wal"] {
            if let Ok(bytes) = std::fs::read(dir.path().join(file)) {
                assert!(
                    !bytes.windows(png.len()).any(|part| part == png),
                    "thumbnail leaked into plaintext database or WAL"
                );
            }
        }
        store.checkpoint()?;
        drop(store);
        assert!(HistoryStore::open_with_key(dir.path(), &[38; 32]).is_err());
        let store = HistoryStore::open_with_key(dir.path(), &key)?;
        assert_eq!(store.thumbnail(&item.id)?, Some(png.clone()));
        // A cache hit must remain readable even when the large original payload cannot decode.
        pollster::block_on(store.connection.execute(
            "UPDATE clips SET payload=?1 WHERE id=?2",
            turso::params![vec![0u8], item.id.clone()],
        ))?;
        assert!(store.content(&item.id).is_err());
        let pixels = crate::preview::decode_thumbnail(&store.thumbnail(&item.id)?.unwrap())?;
        assert_eq!((pixels.width(), pixels.height()), (96, 48));
        assert_eq!(pixels.get_pixel(0, 0).0[3], 127);
        store.clear(false)?;
        assert!(store.thumbnail(&item.id)?.is_some());
        assert!(store.thumbnail(&ordinary.id)?.is_none());
        store.delete(std::slice::from_ref(&item.id))?;
        assert!(store.thumbnail(&item.id)?.is_none());
        assert!(
            !store.save_thumbnail(&item.id, &png)?,
            "late background result resurrected a deleted thumbnail"
        );
        assert!(store.thumbnail(&item.id)?.is_none());
        assert!(
            store
                .save_thumbnail("missing", &vec![0; crate::model::MAX_THUMBNAIL_BYTES + 1])
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn retention_removes_derived_thumbnail_with_original_record() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open_with_key(dir.path(), &[39; 32])?;
        let content = image_content(1);
        let item = store.capture(&content, &Settings::default())?;
        let png = crate::preview::encode_thumbnail(&crate::preview::thumbnail(&content)?)?;
        store.save_thumbnail(&item.id, &png)?;
        let settings = Settings {
            max_entries: 1,
            ..Default::default()
        };
        store.capture(&image_content(2), &settings)?;
        assert!(store.thumbnail(&item.id)?.is_none());
        Ok(())
    }
    #[test]
    #[ignore = "opt-in synthetic encrypted database latency probe"]
    fn protection_latency_probe() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open_with_key(dir.path(), &[31; 32])?;
        let settings = Settings::default();
        let content = ClipContent::text("synthetic protection probe ".repeat(50));
        let sample = store.capture(&content, &settings)?;
        let payload = postcard::to_stdvec(&content)?;
        pollster::block_on(async {
            store.connection.execute("BEGIN IMMEDIATE", ()).await?;
            for index in 0..2000 {
                let mut entry = sample.clone();
                entry.id = format!("probe-{index}");
                entry.fingerprint = format!("{index:064x}");
                entry.order = index + 10;
                store.connection.execute("INSERT INTO clips(id,hash,metadata,payload,ordinal,pinned,favorite,bytes) VALUES(?1,?2,?3,?4,?5,0,0,?6)",
                    turso::params![entry.id.clone(),entry.fingerprint.clone(),postcard::to_stdvec(&entry)?,payload.clone(),entry.order,entry.bytes as i64]).await?;
            }
            store.connection.execute("COMMIT", ()).await?;
            anyhow::Ok(())
        })?;
        let mut times = Vec::new();
        let mut full = Vec::new();
        for index in 0..20 {
            let start = std::time::Instant::now();
            store.protect(&sample.id, Some(index % 2 == 0), Some(index % 2 != 0))?;
            times.push(start.elapsed().as_micros());
            store.prune(&settings)?;
            let _ = store.list()?;
            full.push(start.elapsed().as_micros());
        }
        times.sort();
        full.sort();
        println!(
            "PROTECT 2001 entries: update p50={}us p95={}us; legacy update+prune+refresh p50={}us p95={}us",
            times[10], times[18], full[10], full[18]
        );
        Ok(())
    }
    #[test]
    fn search_includes_full_content_and_removes_deleted_entries() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open_with_key(dir.path(), &[9; 32])?;
        let content = ClipContent::text(format!("{}全文检索末尾", "前".repeat(17000)));
        let entry = store.capture(&content, &Settings::default())?;
        assert!(!entry.text.contains("全文检索末尾"));
        assert!(store.search("全文检索末尾")?.contains(&entry.id));
        store.delete(&[entry.id])?;
        assert!(store.search("全文检索末尾")?.is_empty());
        Ok(())
    }
    #[test]
    fn encrypted_reopen_wrong_key_and_payload() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let key = [42; 32];
        let text = "private-clipboard-payload-6b7f879ee348";
        let store = HistoryStore::open_with_key(dir.path(), &key)?;
        let item = store.capture(&ClipContent::text(text), &Settings::default())?;
        store.checkpoint()?;
        drop(store);
        let disk = std::fs::read(dir.path().join("clipboard.db"))?;
        assert!(!disk.windows(text.len()).any(|w| w == text.as_bytes()));
        assert!(HistoryStore::open_with_key(dir.path(), &[43; 32]).is_err());
        let reopened = HistoryStore::open_with_key(dir.path(), &key)?;
        assert_eq!(reopened.content(&item.id)?.text, text);
        Ok(())
    }
    #[test]
    fn duplicate_refresh_preserves_protection_and_clear_retains_it() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open_with_key(dir.path(), &[7; 32])?;
        let settings = Settings::default();
        let a = store.capture(&ClipContent::text("a"), &settings)?;
        store.protect(&a.id, Some(true), Some(true))?;
        let b = store.capture(&ClipContent::text("b"), &settings)?;
        let a2 = store.capture(&ClipContent::text("a"), &settings)?;
        assert_eq!(a.id, a2.id);
        assert!(a2.order > b.order);
        assert!(a2.pinned && a2.favorite);
        assert_eq!(store.list()?.len(), 2);
        store.clear(false)?;
        assert_eq!(store.list()?.len(), 1);
        store.clear(true)?;
        assert!(store.list()?.is_empty());
        Ok(())
    }
    #[test]
    fn language_survives_encrypted_reopen_without_changing_clipboard_content() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let key = [64; 32];
        let original = ClipContent::text("收藏 / Favorites / 日本語 / {value}");
        let id;
        {
            let store = HistoryStore::open_with_key(dir.path(), &key)?;
            id = store.capture(&original, &Settings::default())?.id;
        }
        for (language, _) in crate::i18n::LANGUAGES {
            let store = HistoryStore::open_with_key(dir.path(), &key)?;
            let mut settings = store.settings()?;
            settings.language = (*language).into();
            store.save_settings(&settings)?;
            drop(store);
            let store = HistoryStore::open_with_key(dir.path(), &key)?;
            assert_eq!(store.settings()?.language, *language);
            assert_eq!(store.content(&id)?.fingerprint(), original.fingerprint());
        }
        Ok(())
    }
    #[test]
    fn hidden_tray_setting_survives_encrypted_reopen() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let key = [65; 32];
        let store = HistoryStore::open_with_key(dir.path(), &key)?;
        let mut settings = store.settings()?;
        settings.show_tray_icon = false;
        settings.middle_click_activation = true;
        settings.paste_shortcut = "Shift+Insert".into();
        store.save_settings(&settings)?;
        drop(store);
        let restored = HistoryStore::open_with_key(dir.path(), &key)?.settings()?;
        assert!(!restored.show_tray_icon);
        assert!(restored.middle_click_activation);
        assert_eq!(restored.paste_shortcut, "Shift+Insert");
        Ok(())
    }
    #[test]
    fn cleanup_applies_both_limits_without_deleting_protected() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open_with_key(dir.path(), &[5; 32])?;
        let mut settings = Settings::default();
        let a = store.capture(&ClipContent::text("a"), &settings)?;
        store.protect(&a.id, None, Some(true))?;
        let b = store.capture(&ClipContent::text("bb"), &settings)?;
        let c = store.capture(&ClipContent::text("ccc"), &settings)?;
        settings.max_entries = 2;
        store.prune(&settings)?;
        assert!(store.content(&b.id).is_err());
        assert!(store.content(&c.id).is_ok());
        settings.max_bytes = 1;
        store.prune(&settings)?;
        let entries = store.list()?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, a.id);
        Ok(())
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_key_reopens_and_missing_key_never_replaces_database() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = HistoryStore::open(dir.path())?;
        store.capture(
            &ClipContent::text("key-protection-check"),
            &Settings::default(),
        )?;
        drop(store);
        let reopened = HistoryStore::open(dir.path())?;
        assert_eq!(reopened.list()?.len(), 1);
        drop(reopened);
        std::fs::remove_file(dir.path().join("clipboard-key.dat"))?;
        let before = std::fs::read(dir.path().join("clipboard.db"))?;
        assert!(HistoryStore::open(dir.path()).is_err());
        assert!(!dir.path().join("clipboard-key.dat").exists());
        assert_eq!(before, std::fs::read(dir.path().join("clipboard.db"))?);
        Ok(())
    }
}
