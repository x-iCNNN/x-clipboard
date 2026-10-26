//! GitHub stable releases, verified downloads, and an out-of-process Windows installer.
mod sources;

use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const PROJECT_URL: &str = "https://github.com/x-iCNNN/x-clipboard";
pub const RELEASES_URL: &str = "https://github.com/x-iCNNN/x-clipboard/releases";
const API: &str = "https://api.github.com/repos/x-iCNNN/x-clipboard/releases/latest";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
pub const SUPPORTED: bool = cfg!(all(windows, target_arch = "x86_64"));

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}
impl Release {
    fn asset(&self) -> Result<&Asset> {
        let name = format!("x-clipboard-{}-windows-x64.zip", self.tag_name);
        self.assets
            .iter()
            .find(|a| a.name == name)
            .context("此版本尚未提供 Windows x64 在线更新包")
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Prepared {
    pub version: String,
    stage: PathBuf,
    target: PathBuf,
    executable: PathBuf,
    files: Vec<String>,
    pid: u32,
    restart: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Probing,
    Downloading,
    Verifying,
}
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub received: u64,
    pub total: u64,
    pub phase: Phase,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            received: 0,
            total: 0,
            phase: Phase::Probing,
        }
    }
}
impl Progress {
    pub fn fraction(self) -> f32 {
        if self.total == 0 {
            0.
        } else {
            (self.received as f64 / self.total as f64).clamp(0., 1.) as f32
        }
    }
}

#[derive(Clone, Debug)]
pub enum State {
    Idle,
    Checking,
    Latest,
    Available(Release),
    Downloading(Progress),
    Ready(Prepared),
    Error(String),
}
impl State {
    pub fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading(_))
    }
    pub fn message(&self) -> String {
        match self {
            Self::Idle => "从 GitHub 获取正式版本。".into(),
            Self::Checking => "正在检查更新…".into(),
            Self::Latest => "当前已是最新版本。".into(),
            Self::Available(r) => format!("发现新版本 {}", r.tag_name),
            Self::Downloading(p) => match p.phase {
                Phase::Probing => crate::i18n::tr("正在准备下载…").into(),
                Phase::Downloading => crate::i18n::message(
                    "正在下载更新… {percent}%",
                    &[("percent", format!("{:.0}", p.fraction() * 100.))],
                ),
                Phase::Verifying => crate::i18n::tr("正在校验更新…").into(),
            },
            Self::Ready(p) => format!(
                "{} 已就绪，可立即重启更新；开启自动更新时将在退出后安装。",
                p.version
            ),
            Self::Error(e) => format!("更新失败：{e}；可重试检查更新。"),
        }
    }
}

pub struct Updater {
    state: Arc<Mutex<(u64, State)>>,
    root: PathBuf,
    next_check: Instant,
}
impl Updater {
    pub fn new(root: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new((0, State::Idle))),
            root,
            next_check: Instant::now() + Duration::from_secs(20),
        }
    }
    #[cfg(feature = "ui-capture")]
    pub fn review_progress(&self, percent: u64) {
        set(
            &self.state,
            State::Downloading(Progress {
                received: percent.min(100),
                total: 100,
                phase: Phase::Downloading,
            }),
        );
    }
    pub fn snapshot(&self) -> (u64, State) {
        self.state.lock().unwrap().clone()
    }
    pub fn tick(&mut self, enabled: bool) {
        if !enabled {
            self.next_check = Instant::now() + Duration::from_secs(20);
        } else if SUPPORTED && Instant::now() >= self.next_check {
            self.next_check = Instant::now() + Duration::from_secs(6 * 60 * 60);
            self.check(true);
        }
    }
    pub fn check(&self, automatic: bool) {
        if !SUPPORTED {
            return;
        }
        {
            let mut state = self.state.lock().unwrap();
            if state.1.busy() || matches!(state.1, State::Ready(_)) {
                return;
            }
            *state = (state.0 + 1, State::Checking);
        }
        let state = self.state.clone();
        let root = self.root.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let client = client()?;
                let release = check(&client, API, env!("CARGO_PKG_VERSION"))?;
                match release {
                    None => Ok(State::Latest),
                    Some(release) if automatic => {
                        Ok(State::Ready(prepare(&client, &release, &root, |p| {
                            set(&state, State::Downloading(p))
                        })?))
                    }
                    Some(release) => Ok(State::Available(release)),
                }
            })();
            set(
                &state,
                result.unwrap_or_else(|e: anyhow::Error| State::Error(format!("{e:#}"))),
            );
        });
    }
    pub fn download(&self) {
        let release = {
            let mut state = self.state.lock().unwrap();
            let State::Available(release) = &state.1 else {
                return;
            };
            let release = release.clone();
            *state = (state.0 + 1, State::Downloading(Progress::default()));
            release
        };
        let state = self.state.clone();
        let root = self.root.clone();
        std::thread::spawn(move || {
            let result = client()
                .and_then(|c| prepare(&c, &release, &root, |p| set(&state, State::Downloading(p))));
            set(
                &state,
                match result {
                    Ok(p) => State::Ready(p),
                    Err(e) => State::Error(format!("{e:#}")),
                },
            );
        });
    }
    /// Arm the helper before asking the service to flush and quit. It waits for this PID.
    pub fn install(&self, restart: bool) -> Result<bool> {
        let State::Ready(mut prepared) = self.snapshot().1 else {
            return Ok(false);
        };
        prepared.restart = restart;
        launch(&prepared)?;
        Ok(true)
    }
}
fn set(state: &Mutex<(u64, State)>, value: State) {
    let mut state = state.lock().unwrap();
    *state = (state.0 + 1, value);
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .user_agent(concat!("x-clipboard/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()?)
}
fn select_release(release: Release, current: &str) -> Result<Option<Release>> {
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .context("GitHub 版本号无效")?;
    if release.draft
        || release.prerelease
        || !version.pre.is_empty()
        || version <= Version::parse(current)?
    {
        return Ok(None);
    }
    release.asset()?;
    Ok(Some(release))
}
fn check(client: &Client, api: &str, current: &str) -> Result<Option<Release>> {
    let response = client
        .get(api)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("GitHub 尚无正式发布版本");
    }
    let bytes = bounded_bytes(response.error_for_status()?, 2 * 1024 * 1024)?;
    select_release(serde_json::from_slice(&bytes)?, current)
}
fn bounded_bytes(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "下载内容超过大小限制");
    Ok(bytes)
}
fn digest(asset: &Asset) -> Result<&str> {
    let digest = asset
        .digest
        .as_deref()
        .and_then(|s| s.strip_prefix("sha256:"))
        .context("GitHub 未提供更新包 SHA-256，已拒绝安装")?;
    ensure!(
        digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()),
        "无效的 SHA-256"
    );
    Ok(digest)
}
fn verify(bytes: &[u8], asset: &Asset) -> Result<()> {
    ensure!(bytes.len() as u64 == asset.size, "更新包大小不匹配");
    let digest = digest(asset)?;
    ensure!(
        format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(digest),
        "更新包 SHA-256 校验失败"
    );
    Ok(())
}
fn prepare(
    client: &Client,
    release: &Release,
    root: &Path,
    mut progress: impl FnMut(Progress),
) -> Result<Prepared> {
    let asset = release.asset()?;
    ensure!(
        asset.size > 0 && asset.size <= MAX_DOWNLOAD,
        "更新包大小超出限制"
    );
    let expected = format!(
        "{RELEASES_URL}/download/{}/{}",
        release.tag_name, asset.name
    );
    ensure!(
        asset.browser_download_url == expected,
        "更新包地址不属于此 GitHub 仓库"
    );
    let bytes = sources::download(client, asset, &sources::urls(&expected), &mut progress)?;
    let executable = std::env::current_exe()?;
    ensure!(
        executable.file_name().and_then(|n| n.to_str()) == Some("x-clipboard.exe"),
        "请从正式版 x-clipboard.exe 运行更新"
    );
    let target = executable
        .parent()
        .context("无法确定安装目录")?
        .to_path_buf();
    // Refuse read-only installations before leaving the app, not halfway through replacement.
    let probe = target.join(format!(".update-write-{}", uuid::Uuid::new_v4()));
    File::create(&probe).context("应用目录不可写，请将便携版移至有写入权限的目录")?;
    fs::remove_file(probe)?;
    let stage = root.join("updates").join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&stage)?;
    let result = (|| {
        let files = extract(&bytes, &stage.join("payload"))?;
        Ok(Prepared {
            version: release.tag_name.clone(),
            stage: stage.clone(),
            target,
            executable,
            files,
            pid: std::process::id(),
            restart: false,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
pub(crate) fn extract(bytes: &[u8], destination: &Path) -> Result<Vec<String>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    ensure!(archive.len() <= 256, "更新包文件过多");
    let mut files = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        ensure!(
            !name.contains('\\') && !name.contains(':') && entry.enclosed_name().is_some(),
            "更新包包含不安全路径"
        );
        ensure!(
            entry.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
            "更新包包含符号链接"
        );
        if entry.is_dir() {
            continue;
        }
        ensure!(allowed_file(&name), "更新包包含非应用文件：{name}");
        ensure!(
            !files.iter().any(|f: &String| f.eq_ignore_ascii_case(&name)),
            "更新包包含重复文件"
        );
        total = total.checked_add(entry.size()).context("解压大小溢出")?;
        ensure!(total <= 512 * 1024 * 1024, "更新包解压后过大");
        let path = destination.join(&name);
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = File::create(path)?;
        let size = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)?;
        ensure!(copied == entry.size(), "更新包文件大小不匹配");
        file.flush()?;
        files.push(name);
    }
    ensure!(
        files.iter().any(|f| f == "x-clipboard.exe") && files.iter().any(|f| f == "pdfium.dll"),
        "更新包缺少 EXE 或 PDFium"
    );
    Ok(files)
}
fn allowed_file(name: &str) -> bool {
    matches!(
        name,
        "x-clipboard.exe"
            | "pdfium.dll"
            | "README.txt"
            | "THIRD-PARTY.md"
            | "SHA256SUMS.txt"
            | "build-info.json"
    ) || name.strip_prefix("licenses/").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .split('/')
                .all(|s| !s.is_empty() && s != "." && s != ".." && !s.ends_with(['.', ' ']))
    })
}
#[cfg(windows)]
fn launch(prepared: &Prepared) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let plan = prepared.stage.join("plan.json");
    fs::write(&plan, serde_json::to_vec(prepared)?)?;
    let script = prepared.stage.join("install.ps1");
    fs::write(&script, include_str!("../scripts/install-update.ps1"))?;
    // Use the OS copy rather than a program from PATH or the app's working directory.
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot 缺失")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut child = std::process::Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-Plan")
        .arg(plan)
        .creation_flags(0x08000000)
        .spawn()
        .context("无法启动更新安装程序")?;
    for _ in 0..100 {
        if prepared.stage.join("ready").exists() {
            return Ok(());
        }
        if child.try_wait()?.is_some() {
            bail!(
                "安装程序启动失败，请查看 {}",
                prepared.stage.join("result.txt").display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    bail!("安装程序未就绪，应用将保持运行")
}
#[cfg(not(windows))]
fn launch(_: &Prepared) -> Result<()> {
    bail!("此平台暂不支持在线安装，请从 GitHub 下载");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: format!("x-clipboard-{tag}-windows-x64.zip"),
                browser_download_url: String::new(),
                size: 3,
                digest: Some(format!("sha256:{:x}", Sha256::digest(b"zip"))),
            }],
        }
    }
    #[test]
    fn stable_semver_only_and_platform_asset_required() {
        assert!(
            select_release(release("v0.10.0"), "0.2.0")
                .unwrap()
                .is_some()
        );
        for tag in ["v0.1.0", "v0.2.0", "v0.3.0-beta.1"] {
            assert!(select_release(release(tag), "0.2.0").unwrap().is_none());
        }
        let mut r = release("v0.3.0");
        r.draft = true;
        assert!(select_release(r, "0.2.0").unwrap().is_none());
        let mut r = release("v0.3.0");
        r.prerelease = true;
        assert!(select_release(r, "0.2.0").unwrap().is_none());
        let mut r = release("v0.3.0");
        r.assets.clear();
        assert!(select_release(r, "0.2.0").is_err());
    }
    #[test]
    fn rejects_corruption_missing_digest_and_oversized_responses() {
        let mut a = release("v0.3.0").assets.remove(0);
        verify(b"zip", &a).unwrap();
        assert!(verify(b"bad", &a).is_err());
        assert!(verify(b"short", &a).is_err());
        a.digest = None;
        assert!(verify(b"zip", &a).is_err());
        assert!(bounded_bytes(&b"1234"[..], 3).is_err());
    }
    fn archive(names: &[&str]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for name in names {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"fixture").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn extracts_only_application_files_and_requires_runtime() {
        let temp = tempfile::tempdir().unwrap();
        extract(
            &archive(&["x-clipboard.exe", "pdfium.dll", "licenses/LICENSE.txt"]),
            temp.path(),
        )
        .unwrap();
        for bad in [
            "../outside",
            "C:/evil",
            "licenses/../../evil",
            "data/client.db",
            "licenses/evil.",
            "pdfium.dll:evil",
        ] {
            assert!(
                extract(
                    &archive(&["x-clipboard.exe", "pdfium.dll", bad]),
                    temp.path()
                )
                .is_err(),
                "{bad}"
            );
        }
        assert!(extract(&archive(&["x-clipboard.exe"]), temp.path()).is_err());
        assert!(
            extract(
                &archive(&["x-clipboard.exe", "pdfium.dll", "PDFIUM.DLL"]),
                temp.path()
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "accesses public GitHub API; run explicitly for release validation"]
    fn github_live_check() {
        let client = client().unwrap();
        let r = check(&client, API, "0.0.0").unwrap().unwrap();
        assert!(r.tag_name.starts_with('v'));
        let a = r.asset().unwrap();
        let bytes = bounded_bytes(
            client
                .get(&a.browser_download_url)
                .send()
                .unwrap()
                .error_for_status()
                .unwrap(),
            MAX_DOWNLOAD,
        )
        .unwrap();
        verify(&bytes, a).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let files = extract(&bytes, dir.path()).unwrap();
        assert!(files.iter().any(|name| name.starts_with("licenses/")));
        assert!(
            check(&client, API, env!("CARGO_PKG_VERSION"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn handles_github_http_errors_and_malformed_metadata() {
        use std::net::TcpListener;
        for (status, body) in [
            (403, "rate limited"),
            (404, "no release"),
            (200, "invalid json"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/release", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                assert!(stream.read(&mut request).unwrap() > 0);
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            });
            assert!(check(&Client::new(), &url, "0.2.0").is_err());
            server.join().unwrap();
        }
    }
}
