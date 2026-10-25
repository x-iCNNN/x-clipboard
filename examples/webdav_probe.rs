//! Real WebDAV integration probe, using only synthetic clipboard contents.
use anyhow::{Context, Result, ensure};
use std::{sync::mpsc, time::Duration};
use x_clipboard::{
    model::{ClipContent, FormatData, Settings},
    storage::HistoryStore,
    sync::{self, Config, Request, Status},
};

fn success(rx: &mpsc::Receiver<Status>) -> Result<Status> {
    loop {
        let status = rx
            .recv_timeout(Duration::from_secs(90))
            .context("同步超时")?;
        if status.message.starts_with("同步失败") {
            anyhow::bail!(status.message);
        }
        if !status.busy && status.last_success.is_some() {
            return Ok(status);
        }
    }
}
fn main() -> Result<()> {
    let endpoint = std::env::args()
        .nth(1)
        .context("Specify the isolated WebDAV fixture URL")?;
    let base = url::Url::parse(&endpoint)?;
    ensure!(
        matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
        "The synthetic probe requires a loopback fixture"
    );
    let endpoint = base
        .join(&format!("probe-{}/", uuid::Uuid::new_v4()))?
        .to_string();
    let response = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()?
        .request("MKCOL".parse()?, &endpoint)
        .basic_auth("sync-test", Some("fixture-login"))
        .send()?;
    ensure!(
        response.status().is_success(),
        "Cannot create isolated DAV fixture collection"
    );
    let config = Config {
        endpoint,
        username: "sync-test".into(),
        password: "fixture-login".into(),
        secret: "synthetic shared sync secret".into(),
        enabled: true,
    };
    let roots = [tempfile::tempdir()?, tempfile::tempdir()?];
    let a = HistoryStore::open(roots[0].path())?;
    let b = HistoryStore::open(roots[1].path())?;
    let mut rich = ClipContent::text("synthetic real WebDAV rich text Ω");
    rich.formats.push(FormatData {
        name: "text/html".into(),
        bytes: b"<b>synthetic rich</b>".to_vec(),
    });
    a.capture(&rich, &Settings::default())?;
    let image = ClipContent {
        formats: vec![FormatData {
            name: "image/png".into(),
            bytes: include_bytes!("../assets/branding/app-icon-32.png").to_vec(),
        }],
        ..Default::default()
    };
    a.capture(&image, &Settings::default())?;
    let file = ClipContent {
        formats: vec![FormatData {
            name: "text/uri-list".into(),
            bytes: b"file:///synthetic/reference-only.txt".to_vec(),
        }],
        files: vec!["/synthetic/reference-only.txt".into()],
        ..Default::default()
    };
    a.capture(&file, &Settings::default())?;
    a.sync_save_config(&config)?;
    let (tx, rx) = mpsc::channel();
    let worker = sync::start(&a, move |s| {
        let _ = tx.send(s);
    })?;
    for _ in 0..20 {
        a.capture(&rich, &Settings::default())?;
        std::thread::sleep(Duration::from_millis(1));
    }
    success(&rx)?;
    sync::synchronize(&b, &config)?;
    ensure!(b.list()?.len() == 3, "文本/图片/文件引用未完整同步");
    for entry in b.list()? {
        let copied = b.content(&entry.id)?;
        ensure!(copied.fingerprint() == entry.fingerprint, "内容指纹不匹配");
        if entry.kind == "file" {
            ensure!(copied.files == file.files, "文件引用发生变化");
        }
    }
    println!(
        "PASS: WsgiDAV Basic auth, encrypted rich text/image/file-reference sync, background startup sync"
    );
    // Real 60-second periodic schedule: no timer override or manual request.
    let periodic = b.capture(
        &ClipContent::text("synthetic periodic sync"),
        &Settings::default(),
    )?;
    sync::synchronize(&b, &config)?;
    success(&rx)?;
    ensure!(
        a.list()?
            .iter()
            .any(|e| e.fingerprint == periodic.fingerprint),
        "自动周期未拉取远端改动"
    );
    println!("PASS: real 60-second automatic pull");
    worker.send(Request::Enabled(false))?;
    loop {
        let status = rx.recv_timeout(Duration::from_secs(5))?;
        if !status.enabled {
            break;
        }
    }
    a.capture(
        &ClipContent::text("synthetic manual while automatic is disabled"),
        &Settings::default(),
    )?;
    worker.send(Request::Now)?;
    success(&rx)?;
    sync::synchronize(&b, &config)?;
    ensure!(b.list()?.len() == 5, "手动同步未上传记录");
    worker.send(Request::Stop)?;
    println!("PASS: automatic off, manual push, preserved data");
    Ok(())
}
