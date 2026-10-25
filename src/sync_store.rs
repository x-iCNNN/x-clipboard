//! Sync metadata shares the encrypted database and the same transaction as local edits.
use crate::{
    model::{ClipContent, Entry},
    storage::{HistoryStore, searchable},
    sync::{Config, CopyInfo, Index, MAX_RECORDS, Record, Register, Stamp},
};
use anyhow::{Context, Result, ensure};

pub(crate) fn initialize(connection: &turso::Connection) -> Result<()> {
    pollster::block_on(async {
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS sync_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL)",
                (),
            )
            .await?;
        connection.execute("CREATE TABLE IF NOT EXISTS sync_records(hash TEXT PRIMARY KEY,value TEXT NOT NULL,evicted INTEGER NOT NULL DEFAULT 0)", ()).await?;
        connection
            .execute(
                "INSERT OR IGNORE INTO sync_meta(key,value) VALUES('clock','0')",
                (),
            )
            .await?;
        connection
            .execute(
                "INSERT OR IGNORE INTO sync_meta(key,value) VALUES('device',?1)",
                [uuid::Uuid::new_v4().to_string()],
            )
            .await?;
        anyhow::Ok(())
    })
}
impl HistoryStore {
    pub(crate) fn transaction<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        let _writer = self.write_gate.lock().unwrap();
        pollster::block_on(self.connection.execute("BEGIN IMMEDIATE", ()))?;
        match operation() {
            Ok(value) => match pollster::block_on(self.connection.execute("COMMIT", ())) {
                Ok(_) => Ok(value),
                Err(error) => {
                    let _ = pollster::block_on(self.connection.execute("ROLLBACK", ()));
                    Err(error.into())
                }
            },
            Err(error) => {
                let _ = pollster::block_on(self.connection.execute("ROLLBACK", ()));
                Err(error)
            }
        }
    }
    async fn stamp(&self) -> Result<Stamp> {
        let mut rows = self
            .connection
            .query("SELECT value FROM sync_meta WHERE key='clock'", ())
            .await?;
        let clock = rows
            .next()
            .await?
            .context("同步时钟缺失")?
            .get::<String>(0)?
            .parse::<u64>()?
            .checked_add(1)
            .context("同步时钟已满")?;
        drop(rows);
        ensure!(clock < i64::MAX as u64, "同步时钟超出范围");
        self.connection
            .execute(
                "UPDATE sync_meta SET value=?1 WHERE key='clock'",
                [clock.to_string()],
            )
            .await?;
        let mut rows = self
            .connection
            .query("SELECT value FROM sync_meta WHERE key='device'", ())
            .await?;
        Ok(Stamp {
            clock,
            device: rows
                .next()
                .await?
                .context("同步设备标识缺失")?
                .get::<String>(0)?,
        })
    }
    async fn record(&self, hash: &str) -> Result<Option<(Record, bool)>> {
        let mut rows = self
            .connection
            .query(
                "SELECT value,evicted FROM sync_records WHERE hash=?1",
                [hash],
            )
            .await?;
        rows.next()
            .await?
            .map(|row| {
                anyhow::Ok((
                    serde_json::from_str(&row.get::<String>(0)?)?,
                    row.get::<i64>(1)? != 0,
                ))
            })
            .transpose()
    }
    async fn save_record(&self, hash: &str, record: &Record, evicted: bool) -> Result<()> {
        self.connection.execute("INSERT INTO sync_records(hash,value,evicted) VALUES(?1,?2,?3) ON CONFLICT(hash) DO UPDATE SET value=excluded.value,evicted=excluded.evicted",
            turso::params![hash, serde_json::to_string(record)?, evicted as i64]).await?;
        Ok(())
    }
    pub(crate) async fn sync_capture(&self, entry: &Entry) -> Result<()> {
        let stamp = self.stamp().await?;
        let mut record = self
            .record(&entry.fingerprint)
            .await?
            .map(|p| p.0)
            .unwrap_or_else(|| Record::new(entry, stamp.clone()));
        if record.deleted.value {
            record.pinned = Register {
                stamp: stamp.clone(),
                value: entry.pinned,
            };
            record.favorite = Register {
                stamp: stamp.clone(),
                value: entry.favorite,
            };
        }
        record.copy = Register {
            stamp: stamp.clone(),
            value: CopyInfo {
                copied_at: entry.copied_at,
                source: entry.source.clone(),
            },
        };
        record.deleted = Register {
            stamp,
            value: false,
        };
        self.save_record(&entry.fingerprint, &record, false).await
    }
    pub(crate) async fn sync_protect(
        &self,
        entry: &Entry,
        pin: bool,
        favorite: bool,
    ) -> Result<()> {
        let stamp = self.stamp().await?;
        let mut record = self
            .record(&entry.fingerprint)
            .await?
            .map(|p| p.0)
            .unwrap_or_else(|| Record::new(entry, stamp.clone()));
        if pin {
            record.pinned = Register {
                stamp: stamp.clone(),
                value: entry.pinned,
            };
        }
        if favorite {
            record.favorite = Register {
                stamp,
                value: entry.favorite,
            };
        }
        self.save_record(&entry.fingerprint, &record, false).await
    }
    pub(crate) async fn sync_remove(&self, ids: &[String], explicit: bool) -> Result<()> {
        for id in ids {
            let mut rows = self
                .connection
                .query("SELECT metadata FROM clips WHERE id=?1", [id.as_str()])
                .await?;
            let Some(row) = rows.next().await? else {
                continue;
            };
            let entry: Entry = postcard::from_bytes(&row.get::<Vec<u8>>(0)?)?;
            drop(rows);
            if explicit {
                let stamp = self.stamp().await?;
                let mut record = self
                    .record(&entry.fingerprint)
                    .await?
                    .map(|p| p.0)
                    .unwrap_or_else(|| Record::new(&entry, stamp.clone()));
                record.deleted = Register { stamp, value: true };
                self.save_record(&entry.fingerprint, &record, false).await?;
            } else {
                if self.record(&entry.fingerprint).await?.is_none() {
                    let stamp = self.stamp().await?;
                    self.save_record(&entry.fingerprint, &Record::new(&entry, stamp), true)
                        .await?;
                }
                self.connection
                    .execute(
                        "UPDATE sync_records SET evicted=1 WHERE hash=?1",
                        [entry.fingerprint],
                    )
                    .await?;
            }
        }
        Ok(())
    }
    pub fn sync_config(&self) -> Result<Config> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT value FROM sync_meta WHERE key='config'", ())
                .await?;
            match rows.next().await? {
                Some(row) => Ok(serde_json::from_str(&row.get::<String>(0)?)?),
                None => Ok(Config::default()),
            }
        })
    }
    pub fn sync_save_config(&self, config: &Config) -> Result<()> {
        config.validate()?;
        let _writer = self.write_gate.lock().unwrap();
        let json = zeroize::Zeroizing::new(serde_json::to_string(config)?);
        pollster::block_on(self.connection.execute("INSERT INTO sync_meta(key,value) VALUES('config',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json.as_str()]))?;
        Ok(())
    }
    pub(crate) fn sync_observe(&self, clock: u64) -> Result<()> {
        self.transaction(|| {
            pollster::block_on(self.connection.execute("UPDATE sync_meta SET value=CAST(MAX(CAST(value AS INTEGER),?1) AS TEXT) WHERE key='clock'", [clock as i64]))?;
            Ok(())
        })
    }
    pub(crate) fn sync_export(&self) -> Result<Index> {
        // Backfill old installations once, without changing existing IDs or payloads.
        self.transaction(|| pollster::block_on(async {
            let mut rows = self.connection.query("SELECT metadata FROM clips WHERE hash NOT IN (SELECT hash FROM sync_records) ORDER BY ordinal ASC", ()).await?;
            let mut missing = Vec::new();
            while let Some(row) = rows.next().await? { missing.push(postcard::from_bytes::<Entry>(&row.get::<Vec<u8>>(0)?)?); }
            drop(rows);
            for entry in missing { let stamp = self.stamp().await?; self.save_record(&entry.fingerprint, &Record::new(&entry, stamp), false).await?; }
            let mut rows = self.connection.query("SELECT hash,value FROM sync_records WHERE evicted=0", ()).await?;
            let mut index = Index::default();
            while let Some(row) = rows.next().await? {
                ensure!(index.records.len() < MAX_RECORDS, "本机同步记录超过条数上限");
                index.records.insert(row.get::<String>(0)?, serde_json::from_str(&row.get::<String>(1)?)?);
            }
            Ok(index)
        }))
    }
    pub(crate) fn sync_content(&self, hash: &str) -> Result<Option<ClipContent>> {
        pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT payload FROM clips WHERE hash=?1", [hash])
                .await?;
            rows.next()
                .await?
                .map(|row| anyhow::Ok(postcard::from_bytes(&row.get::<Vec<u8>>(0)?)?))
                .transpose()
        })
    }
    pub(crate) fn sync_needs_content(&self, hash: &str, remote: &Record) -> Result<bool> {
        let local = pollster::block_on(self.record(hash))?;
        let mut merged = remote.clone();
        if let Some((record, _)) = &local {
            merged.merge(record);
        }
        let present = pollster::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT EXISTS(SELECT 1 FROM clips WHERE hash=?1)", [hash])
                .await?;
            anyhow::Ok(
                rows.next()
                    .await?
                    .context("无法检查同步内容")?
                    .get::<i64>(0)?
                    != 0,
            )
        })?;
        if merged.deleted.value || present {
            return Ok(false);
        }
        if let Some((record, true)) = local {
            return Ok(merged.copy.stamp > record.copy.stamp
                || (merged.pinned.value && merged.pinned.stamp > record.pinned.stamp)
                || (merged.favorite.value && merged.favorite.stamp > record.favorite.stamp));
        }
        Ok(true)
    }
    pub(crate) fn sync_import(
        &self,
        hash: &str,
        remote: &Record,
        content: Option<&ClipContent>,
    ) -> Result<()> {
        if content.is_none()
            && let Some((local, _)) = pollster::block_on(self.record(hash))?
        {
            let mut merged = local.clone();
            merged.merge(remote);
            if merged == local {
                return Ok(());
            }
        }
        self.transaction(|| pollster::block_on(async {
            let local = self.record(hash).await?;
            let mut record = remote.clone();
            if let Some((local, _)) = &local { record.merge(local); }
            let mut rows = self.connection.query("SELECT metadata FROM clips WHERE hash=?1", [hash]).await?;
            let existing = rows.next().await?.map(|row| anyhow::Ok(postcard::from_bytes::<Entry>(&row.get::<Vec<u8>>(0)?)?)).transpose()?;
            drop(rows);
            if record.deleted.value {
                if let Some(entry) = existing { self.connection.execute("DELETE FROM clips WHERE id=?1", [entry.id.clone()]).await?; self.connection.execute("DELETE FROM clip_search WHERE id=?1", [entry.id.clone()]).await?; self.connection.execute("DELETE FROM clip_thumbnails WHERE id=?1", [entry.id]).await?; }
                self.save_record(hash, &record, false).await?;
                return Ok(());
            }
            let evicted = existing.is_none() && content.is_none() && local.is_some_and(|p| p.1);
            if let Some(content) = content {
                content.validate()?;
                ensure!(content.fingerprint() == hash, "同步记录指纹不匹配");
            }
            if existing.is_none() && content.is_none() && !evicted { anyhow::bail!("同步记录缺少内容，未导入"); }
            if let Some(mut entry) = existing.or_else(|| content.map(|content| Entry { id: uuid::Uuid::new_v4().to_string(), fingerprint: hash.into(), title: content.title(), text: content.text.chars().take(512).collect(), kind: content.kind().into(), source: String::new(), copied_at: 0, order: 0, pinned: false, favorite: false, bytes: content.bytes() })) {
                entry.copied_at = record.copy.value.copied_at; entry.source = record.copy.value.source.clone(); entry.pinned = record.pinned.value; entry.favorite = record.favorite.value;
                // Logical order does not depend on devices having synchronized wall clocks.
                entry.order = record.copy.stamp.clock as i64;
                if let Some(content) = content {
                    self.connection.execute("INSERT INTO clips(id,hash,metadata,payload,ordinal,pinned,favorite,bytes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(hash) DO UPDATE SET metadata=excluded.metadata,ordinal=excluded.ordinal,pinned=excluded.pinned,favorite=excluded.favorite",
                        turso::params![entry.id.clone(),hash,postcard::to_stdvec(&entry)?,postcard::to_stdvec(content)?,entry.order,entry.pinned as i64,entry.favorite as i64,entry.bytes as i64]).await?;
                    self.connection.execute("INSERT INTO clip_search(id,searchable) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET searchable=excluded.searchable", [entry.id.clone(),searchable(content)]).await?;
                } else {
                    self.connection.execute("UPDATE clips SET metadata=?1,ordinal=?2,pinned=?3,favorite=?4 WHERE id=?5",turso::params![postcard::to_stdvec(&entry)?,entry.order,entry.pinned as i64,entry.favorite as i64,entry.id]).await?;
                }
            }
            self.save_record(hash, &record, evicted).await?;
            Ok(())
        }))?;
        self.prune(&self.settings()?)?;
        Ok(())
    }
}
