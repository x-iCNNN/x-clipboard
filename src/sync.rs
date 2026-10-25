//! Encrypted WebDAV sync. Only ciphertext is sent to the server.
use crate::{
    model::{ClipContent, Entry, now_ms},
    storage::HistoryStore,
};
use anyhow::{Context, Result, bail, ensure};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{ETAG, IF_MATCH, IF_NONE_MATCH},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, time::Duration};
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 8] = b"XCDAV001";
const MAX_INDEX: u64 = 64 * 1024 * 1024;
const MAX_BLOB: u64 = 192 * 1024 * 1024;
pub(crate) const MAX_RECORDS: usize = 100_000;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub secret: String,
    pub enabled: bool,
}
impl Drop for Config {
    fn drop(&mut self) {
        self.password.zeroize();
        self.secret.zeroize();
    }
}
impl Config {
    pub fn validate(&self) -> Result<url::Url> {
        let mut url = url::Url::parse(self.endpoint.trim()).context("WebDAV 地址无效")?;
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "请分别填写地址和账号，地址不能包含凭据、查询参数或片段"
        );
        ensure!(
            url.scheme() == "https"
                || (url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))),
            "WebDAV 需要 HTTPS（仅本机测试允许 HTTP）"
        );
        ensure!(url.host_str().is_some(), "WebDAV 地址缺少服务器");
        ensure!(
            self.secret.chars().count() >= 12 && self.secret.len() <= 1024,
            "同步密码至少 12 个字符，最多 1024 字节；所有设备需使用相同密码"
        );
        ensure!(
            self.username.len() <= 4096 && self.password.len() <= 4096,
            "登录信息过长"
        );
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(url)
    }
}
#[derive(Clone, Default)]
pub struct Status {
    pub endpoint: String,
    pub username: String,
    pub enabled: bool,
    pub configured: bool,
    pub busy: bool,
    pub last_success: Option<i64>,
    pub message: String,
}
impl Status {
    fn from_config(config: &Config) -> Self {
        Self {
            endpoint: config.endpoint.clone(),
            username: config.username.clone(),
            enabled: config.enabled,
            configured: !config.endpoint.is_empty(),
            message: "尚未同步".into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Stamp {
    pub clock: u64,
    pub device: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Register<T> {
    pub stamp: Stamp,
    pub value: T,
}
impl<T: Clone> Register<T> {
    fn merge(&mut self, other: &Self) {
        if other.stamp > self.stamp {
            *self = other.clone();
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CopyInfo {
    pub copied_at: i64,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    pub copy: Register<CopyInfo>,
    pub pinned: Register<bool>,
    pub favorite: Register<bool>,
    pub deleted: Register<bool>,
}
impl Record {
    pub fn new(entry: &Entry, stamp: Stamp) -> Self {
        Self {
            copy: Register {
                stamp: stamp.clone(),
                value: CopyInfo {
                    copied_at: entry.copied_at,
                    source: entry.source.clone(),
                },
            },
            pinned: Register {
                stamp: stamp.clone(),
                value: entry.pinned,
            },
            favorite: Register {
                stamp: stamp.clone(),
                value: entry.favorite,
            },
            deleted: Register {
                stamp,
                value: false,
            },
        }
    }
    pub fn merge(&mut self, other: &Self) {
        self.copy.merge(&other.copy);
        self.pinned.merge(&other.pinned);
        self.favorite.merge(&other.favorite);
        self.deleted.merge(&other.deleted);
    }
    pub fn clock(&self) -> u64 {
        [
            &self.copy.stamp,
            &self.pinned.stamp,
            &self.favorite.stamp,
            &self.deleted.stamp,
        ]
        .into_iter()
        .map(|s| s.clock)
        .max()
        .unwrap_or(0)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Index {
    pub schema: u32,
    pub records: BTreeMap<String, Record>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            schema: 1,
            records: BTreeMap::new(),
        }
    }
}
impl Index {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == 1 && self.records.len() <= MAX_RECORDS,
            "远端同步索引版本或条数超出支持范围"
        );
        for (hash, record) in &self.records {
            ensure!(
                hash.len() == 64
                    && hash.bytes().all(|b| b.is_ascii_hexdigit())
                    && record.copy.value.source.len() <= 4096,
                "远端同步记录无效"
            );
            for stamp in [
                &record.copy.stamp,
                &record.pinned.stamp,
                &record.favorite.stamp,
                &record.deleted.stamp,
            ] {
                ensure!(
                    stamp.clock > 0
                        && stamp.clock < i64::MAX as u64
                        && uuid::Uuid::parse_str(&stamp.device).is_ok(),
                    "远端同步版本无效"
                );
            }
        }
        Ok(())
    }
    pub fn merge(&mut self, other: &Self) {
        for (hash, record) in &other.records {
            self.records
                .entry(hash.clone())
                .and_modify(|r| r.merge(record))
                .or_insert_with(|| record.clone());
        }
    }
}
fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| anyhow::anyhow!("无法生成同步随机数"))?;
    Ok(bytes)
}
fn derive(secret: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0; 32]);
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 2, 1, Some(32)).map_err(|_| anyhow::anyhow!("同步加密参数无效"))?,
    )
    .hash_password_into(secret.as_bytes(), salt, &mut *key)
    .map_err(|_| anyhow::anyhow!("同步密钥派生失败"))?;
    Ok(key)
}
fn seal(key: &[u8; 32], salt: &[u8; 16], plaintext: &[u8], context: &str) -> Result<Vec<u8>> {
    let nonce = random::<24>()?;
    let cipher = XChaCha20Poly1305::new(key.into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: context.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("同步加密失败"))?;
    let mut bytes = Vec::with_capacity(48 + cipher.len());
    bytes.extend(MAGIC);
    bytes.extend(salt);
    bytes.extend(nonce);
    bytes.extend(cipher);
    Ok(bytes)
}
fn salt(bytes: &[u8]) -> Result<[u8; 16]> {
    ensure!(
        bytes.len() >= 64 && &bytes[..8] == MAGIC,
        "远端不是有效的加密同步文件；未覆盖任何数据"
    );
    Ok(bytes[8..24].try_into()?)
}
fn unseal(key: &[u8; 32], bytes: &[u8], context: &str) -> Result<Zeroizing<Vec<u8>>> {
    salt(bytes)?;
    Ok(Zeroizing::new(
        XChaCha20Poly1305::new(key.into())
            .decrypt(
                XNonce::from_slice(&bytes[24..48]),
                Payload {
                    msg: &bytes[48..],
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("同步密码不匹配或远端数据已损坏；未覆盖任何数据"))?,
    ))
}
fn blob_name(key: &[u8; 32], hash: &str) -> String {
    let mut h = Sha256::new();
    h.update(key);
    h.update(hash);
    format!("{:x}.bin", h.finalize())
}
fn bounded(response: Response, maximum: u64) -> Result<Vec<u8>> {
    ensure!(
        response.content_length().is_none_or(|n| n <= maximum),
        "远端同步文件过大"
    );
    let mut bytes = Vec::new();
    response.take(maximum + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= maximum, "远端同步文件超过大小限制");
    Ok(bytes)
}
fn network_reason(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut cause: &dyn std::error::Error = &error;
    while let Some(source) = cause.source() {
        cause = source;
    }
    cause.to_string()
}
struct Dav {
    client: Client,
    base: url::Url,
    config: Config,
}
impl Dav {
    fn new(config: &Config) -> Result<Self> {
        let base = config.validate()?;
        let mut builder = Client::builder();
        if matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
            builder = builder.no_proxy();
        }
        let client = builder
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("x-clipboard/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            base,
            config: config.clone(),
        })
    }
    fn request(&self, method: &str, path: &str) -> Result<reqwest::blocking::RequestBuilder> {
        let mut request = self.client.request(method.parse()?, self.base.join(path)?);
        if !self.config.username.is_empty() {
            request = request.basic_auth(&self.config.username, Some(&self.config.password));
        }
        Ok(request)
    }
    fn ensure_folder(&self, path: &str) -> Result<()> {
        let response = self.request("MKCOL", path)?.send().map_err(|e| {
            anyhow::anyhow!(
                "无法连接 WebDAV；请检查地址、证书与网络：{}",
                network_reason(e)
            )
        })?;
        ensure!(
            response.status().is_success() || response.status() == StatusCode::METHOD_NOT_ALLOWED,
            "WebDAV 创建目录失败（HTTP {}）",
            response.status().as_u16()
        );
        Ok(())
    }
    fn get_index(&self) -> Result<Option<(Vec<u8>, String)>> {
        let response = self
            .request("GET", "x-clipboard-v1/index.bin")?
            .send()
            .map_err(|e| anyhow::anyhow!("WebDAV 下载失败，请检查网络：{}", network_reason(e)))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        ensure!(
            response.status().is_success(),
            "WebDAV 读取失败（HTTP {}），请检查账号和目录权限",
            response.status().as_u16()
        );
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|e| e.to_str().ok())
            .context("WebDAV 必须支持 ETag 与条件写入")?
            .to_owned();
        ensure!(
            etag.starts_with('"') && etag.ends_with('"'),
            "WebDAV 返回弱或无效 ETag，已停止以免覆盖其他设备"
        );
        Ok(Some((bounded(response, MAX_INDEX)?, etag)))
    }
    fn put(&self, path: &str, bytes: Vec<u8>, etag: Option<&str>) -> Result<bool> {
        let request = self
            .request("PUT", path)?
            .header("Content-Type", "application/octet-stream")
            .body(bytes);
        let request = if let Some(etag) = etag {
            request.header(IF_MATCH, etag)
        } else {
            request.header(IF_NONE_MATCH, "*")
        };
        let response = request
            .send()
            .map_err(|e| anyhow::anyhow!("WebDAV 上传中断；可安全重试：{}", network_reason(e)))?;
        if response.status() == StatusCode::PRECONDITION_FAILED {
            return Ok(false);
        }
        ensure!(
            response.status().is_success(),
            "WebDAV 写入失败（HTTP {}）",
            response.status().as_u16()
        );
        Ok(true)
    }
}

pub fn synchronize(store: &HistoryStore, config: &Config) -> Result<()> {
    let dav = Dav::new(config)?;
    dav.ensure_folder("x-clipboard-v1/")?;
    dav.ensure_folder("x-clipboard-v1/blobs/")?;
    for _ in 0..4 {
        let previous = dav.get_index()?;
        let salt = if let Some((bytes, _)) = &previous {
            salt(bytes)?
        } else {
            random()?
        };
        let key = derive(&config.secret, &salt)?;
        let mut remote: Index = if let Some((bytes, _)) = &previous {
            serde_json::from_slice(&unseal(&key, bytes, "index")?).context("同步索引无效")?
        } else {
            Index::default()
        };
        remote.validate()?;
        let known_blobs: std::collections::HashSet<_> = remote
            .records
            .iter()
            .filter(|(_, record)| !record.deleted.value)
            .map(|(hash, _)| hash.clone())
            .collect();
        store.sync_observe(
            remote
                .records
                .values()
                .map(Record::clock)
                .max()
                .unwrap_or(0),
        )?;
        let mut local = store.sync_export()?;
        let original = serde_json::to_vec(&remote)?;
        let mut unavailable = Vec::new();
        // Upload immutable blobs before publishing their references. Failed uploads never expose an incomplete index.
        for (hash, record) in &local.records {
            if record.deleted.value || known_blobs.contains(hash) {
                continue;
            }
            if let Some(content) = store.sync_content(hash)? {
                let name = blob_name(&key, hash);
                let path = format!("x-clipboard-v1/blobs/{name}");
                // HEAD avoids uploading unchanged image payloads on every periodic sync.
                let response = dav.request("HEAD", &path)?.send().map_err(|e| {
                    anyhow::anyhow!("WebDAV 无法检查内容文件：{}", network_reason(e))
                })?;
                if response.status() == StatusCode::NOT_FOUND {
                    let plain = Zeroizing::new(postcard::to_stdvec(&content)?);
                    ensure!(plain.len() as u64 <= MAX_BLOB - 64, "同步内容过大");
                    dav.put(&path, seal(&key, &salt, &plain, &name)?, None)?;
                } else {
                    ensure!(
                        response.status().is_success(),
                        "WebDAV 内容检查失败（HTTP {}）",
                        response.status().as_u16()
                    );
                }
            } else {
                // A user deletion or local retention may have removed this payload since export.
                // Do not publish a new reference unless its immutable blob was actually available.
                unavailable.push(hash.clone());
            }
        }
        for hash in unavailable {
            local.records.remove(&hash);
        }
        remote.merge(&local);
        remote.validate()?;
        // Download to memory one record at a time and import transactionally. No plaintext staging files.
        for (hash, record) in &remote.records {
            if !record.deleted.value && store.sync_needs_content(hash, record)? {
                let name = blob_name(&key, hash);
                let response = dav
                    .request("GET", &format!("x-clipboard-v1/blobs/{name}"))?
                    .send()
                    .map_err(|e| anyhow::anyhow!("同步内容下载中断：{}", network_reason(e)))?;
                ensure!(
                    response.status().is_success(),
                    "同步内容缺失或不可读取（HTTP {}）；请在来源设备重试同步",
                    response.status().as_u16()
                );
                let bytes = bounded(response, MAX_BLOB)?;
                ensure!(
                    crate::sync::salt(&bytes)? == salt,
                    "同步内容不属于当前加密空间"
                );
                let content: ClipContent = postcard::from_bytes(&unseal(&key, &bytes, &name)?)?;
                content.validate()?;
                ensure!(
                    content.fingerprint() == *hash
                        && content.text.len()
                            + content.files.iter().map(String::len).sum::<usize>()
                            <= MAX_BLOB as usize,
                    "同步内容校验失败"
                );
                store.sync_import(hash, record, Some(&content))?;
            } else {
                store.sync_import(hash, record, None)?;
            }
        }
        let bytes = Zeroizing::new(serde_json::to_vec(&remote)?);
        ensure!(bytes.len() as u64 <= MAX_INDEX - 64, "同步索引超过大小上限");
        if previous.is_some() && *bytes == original {
            return Ok(());
        }
        if dav.put(
            "x-clipboard-v1/index.bin",
            seal(&key, &salt, &bytes, "index")?,
            previous.as_ref().map(|p| p.1.as_str()),
        )? {
            return Ok(());
        }
    }
    bail!("其他设备正在同时修改，请稍后重试；本机改动已保留")
}

pub enum Request {
    Configure(Config),
    Enabled(bool),
    Now,
    Stop,
}
pub struct Handle {
    tx: crossbeam_channel::Sender<Request>,
}
impl Handle {
    pub fn send(&self, request: Request) -> Result<()> {
        self.tx
            .try_send(request)
            .map_err(|_| anyhow::anyhow!("同步任务繁忙，请稍后重试"))
    }
}
pub fn start(store: &HistoryStore, notify: impl Fn(Status) + Send + 'static) -> Result<Handle> {
    let store = store.fork()?;
    let (tx, rx) = crossbeam_channel::bounded(4);
    std::thread::spawn(move || {
        let mut config = match store.sync_config() {
            Ok(config) => config,
            Err(_) => {
                notify(Status {
                    message: "同步配置读取失败".into(),
                    ..Default::default()
                });
                return;
            }
        };
        let mut status = Status::from_config(&config);
        notify(status.clone());
        let mut first = config.enabled;
        loop {
            let request = if first {
                first = false;
                Ok(Request::Now)
            } else {
                rx.recv_timeout(Duration::from_secs(60))
            };
            let run = match request {
                Ok(Request::Stop) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                Ok(Request::Enabled(on)) => {
                    let mut next = config.clone();
                    next.enabled = on;
                    match store.sync_save_config(&next) {
                        Ok(()) => {
                            config = next;
                            status.enabled = on;
                            status.message = if on {
                                "已开启自动同步"
                            } else {
                                "已关闭自动同步"
                            }
                            .into();
                            notify(status.clone());
                            on
                        }
                        Err(_) => {
                            status.message = "同步设置保存失败".into();
                            notify(status.clone());
                            false
                        }
                    }
                }
                Ok(Request::Configure(mut next)) => {
                    if next.endpoint == config.endpoint && next.username == config.username {
                        if next.password.is_empty() {
                            next.password = config.password.clone();
                        }
                        if next.secret.is_empty() {
                            next.secret = config.secret.clone();
                        }
                    }
                    match next.validate().and_then(|_| store.sync_save_config(&next)) {
                        Ok(()) => {
                            config = next;
                            status = Status::from_config(&config);
                            true
                        }
                        Err(e) => {
                            status.message = format!("{e:#}");
                            notify(status.clone());
                            false
                        }
                    }
                }
                Ok(Request::Now) => true,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => config.enabled,
            };
            if run && !config.endpoint.is_empty() {
                status.busy = true;
                status.message = "正在加密并同步…".into();
                notify(status.clone());
                match synchronize(&store, &config) {
                    Ok(()) => {
                        status.last_success = Some(now_ms());
                        status.message = "同步完成".into();
                    }
                    Err(e) => status.message = format!("同步失败：{e:#}"),
                }
                status.busy = false;
                notify(status.clone());
            }
        }
    });
    Ok(Handle { tx })
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
