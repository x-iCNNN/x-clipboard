//! Bounded, concurrent sample probes and checksum-verified source fallback.
use super::*;
use reqwest::{StatusCode, header};

const SAMPLE_BYTES: u64 = 64 * 1024;
const MIRRORS: [&str; 3] = [
    "https://ghfast.top/",
    "https://ghproxy.net/",
    "https://gh-proxy.com/",
];

pub(super) fn urls(original: &str) -> Vec<String> {
    std::iter::once(original.to_owned())
        .chain(MIRRORS.iter().map(|prefix| format!("{prefix}{original}")))
        .collect()
}

fn probe(client: &Client, url: &str, size: u64) -> Result<f64> {
    let started = Instant::now();
    let count = size.min(SAMPLE_BYTES);
    let mut response = client
        .get(url)
        .header(header::RANGE, format!("bytes=0-{}", count - 1))
        .header(header::ACCEPT_ENCODING, "identity")
        .timeout(Duration::from_secs(5))
        .send()?
        .error_for_status()?;
    ensure!(
        matches!(
            response.status(),
            StatusCode::OK | StatusCode::PARTIAL_CONTENT
        ),
        "无效的测速响应"
    );
    if response.status() == StatusCode::PARTIAL_CONTENT {
        let range = response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok());
        ensure!(
            range == Some(format!("bytes 0-{}/{size}", count - 1).as_str()),
            "测速范围不匹配"
        );
    } else if let Some(length) = response.content_length() {
        ensure!(length == size, "测速文件大小不匹配");
    }
    let mut bytes = vec![0; count as usize];
    response.read_exact(&mut bytes)?;
    ensure!(bytes.starts_with(b"PK"), "测速响应不是更新包");
    Ok(count as f64 / started.elapsed().as_secs_f64().max(0.001))
}

pub(super) fn rank(client: &Client, sources: &[String], size: u64) -> Vec<(String, Option<f64>)> {
    let mut ranked = std::thread::scope(|scope| {
        let tasks: Vec<_> = sources
            .iter()
            .map(|url| {
                let client = client.clone();
                scope.spawn(move || (url.clone(), probe(&client, url, size).ok()))
            })
            .collect();
        tasks
            .into_iter()
            .filter_map(|task| task.join().ok())
            .collect::<Vec<_>>()
    });
    ranked.sort_by(|a, b| b.1.unwrap_or(0.).total_cmp(&a.1.unwrap_or(0.)));
    ranked
}

pub(super) fn download(
    client: &Client,
    asset: &Asset,
    sources: &[String],
    progress: &mut impl FnMut(Progress),
) -> Result<Vec<u8>> {
    ensure!(
        asset.size >= 2 && asset.size <= MAX_DOWNLOAD,
        "更新包大小超出限制"
    );
    digest(asset)?;
    progress(Progress {
        received: 0,
        total: asset.size,
        phase: Phase::Probing,
    });
    let ranked = rank(client, sources, asset.size);
    for (url, _) in ranked {
        progress(Progress {
            received: 0,
            total: asset.size,
            phase: Phase::Downloading,
        });
        let attempt = (|| -> Result<Vec<u8>> {
            let mut response = client
                .get(&url)
                .header(header::ACCEPT_ENCODING, "identity")
                .timeout(Duration::from_secs(120))
                .send()?
                .error_for_status()?;
            ensure!(response.status() == StatusCode::OK, "下载响应不完整");
            if let Some(length) = response.content_length() {
                ensure!(length == asset.size, "下载文件大小不匹配");
            }
            let mut bytes = Vec::with_capacity(asset.size as usize);
            let mut chunk = [0; 64 * 1024];
            let mut last = Instant::now();
            loop {
                let count = response.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                ensure!(
                    bytes.len() as u64 + count as u64 <= asset.size,
                    "下载内容超过大小限制"
                );
                bytes.extend_from_slice(&chunk[..count]);
                if last.elapsed() >= Duration::from_millis(100) || bytes.len() as u64 == asset.size
                {
                    progress(Progress {
                        received: bytes.len() as u64,
                        total: asset.size,
                        phase: Phase::Downloading,
                    });
                    last = Instant::now();
                }
            }
            progress(Progress {
                received: bytes.len() as u64,
                total: asset.size,
                phase: Phase::Verifying,
            });
            verify(&bytes, asset)?;
            Ok(bytes)
        })();
        if let Ok(bytes) = attempt {
            return Ok(bytes);
        }
    }
    bail!("所有下载线路均未完成更新包下载与校验，请稍后重试")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        sync::{Arc, Mutex},
    };
    #[derive(Clone, Copy, PartialEq)]
    enum Reply {
        Valid,
        Corrupt,
        Truncated,
        Overlong,
    }
    fn server(
        id: usize,
        reply: Reply,
        delay: u64,
        order: Arc<Mutex<Vec<usize>>>,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/asset.zip", listener.local_addr().unwrap());
        let task = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buf[..count]);
                }
                let ranged = String::from_utf8_lossy(&request)
                    .to_ascii_lowercase()
                    .contains("range: bytes=0-");
                if ranged {
                    std::thread::sleep(Duration::from_millis(delay));
                } else {
                    order.lock().unwrap().push(id);
                }
                let mut body = vec![b'a'; 4096];
                body[..2].copy_from_slice(b"PK");
                if !ranged {
                    match reply {
                        Reply::Corrupt => body[100] = b'b',
                        Reply::Truncated => body.truncate(1024),
                        Reply::Overlong => body.extend_from_slice(&[0; 1024]),
                        Reply::Valid => {}
                    }
                }
                if !ranged && reply == Reply::Overlong {
                    write!(stream, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n").unwrap();
                } else {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n"
                    )
                    .unwrap();
                }
                let _ = stream.write_all(&body);
            }
        });
        (url, task)
    }
    fn asset() -> Asset {
        let mut bytes = vec![b'a'; 4096];
        bytes[..2].copy_from_slice(b"PK");
        Asset {
            name: "fixture.zip".into(),
            browser_download_url: String::new(),
            size: 4096,
            digest: Some(format!("sha256:{:x}", Sha256::digest(&bytes))),
        }
    }
    #[test]
    fn faster_corrupt_source_falls_back_and_reports_real_progress() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let (slow, a) = server(0, Reply::Valid, 150, order.clone());
        let (fast, b) = server(1, Reply::Corrupt, 0, order.clone());
        let mut events = Vec::new();
        let bytes = download(&Client::new(), &asset(), &[slow, fast], &mut |p| {
            events.push(p)
        })
        .unwrap();
        verify(&bytes, &asset()).unwrap();
        a.join().unwrap();
        b.join().unwrap();
        assert_eq!(*order.lock().unwrap(), vec![1, 0]);
        assert_eq!(events.first().unwrap().phase, Phase::Probing);
        assert_eq!(
            events
                .iter()
                .filter(|p| p.received == 0 && p.phase == Phase::Downloading)
                .count(),
            2
        );
        assert_eq!(events.last().unwrap().fraction(), 1.);
        assert!(
            events
                .iter()
                .any(|p| p.received > 0 && p.phase == Phase::Downloading)
        );
    }
    #[test]
    fn all_corrupt_sources_fail_closed_and_missing_digest_never_fetches() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let (url, a) = server(0, Reply::Corrupt, 0, order);
        assert!(download(&Client::new(), &asset(), &[url], &mut |_| {}).is_err());
        a.join().unwrap();
        let mut invalid = asset();
        invalid.digest = None;
        assert!(
            download(
                &Client::new(),
                &invalid,
                &["http://127.0.0.1:9/no-request".into()],
                &mut |_| panic!("invalid metadata reached network phase")
            )
            .is_err()
        );
    }
    #[test]
    fn incomplete_and_unbounded_oversize_bodies_are_rejected() {
        for reply in [Reply::Truncated, Reply::Overlong] {
            let (url, server) = server(0, reply, 0, Arc::new(Mutex::new(Vec::new())));
            assert!(download(&Client::new(), &asset(), &[url], &mut |_| {}).is_err());
            server.join().unwrap();
        }
    }
    #[test]
    fn unavailable_probe_does_not_block_a_working_source() {
        let (url, a) = server(0, Reply::Valid, 0, Arc::new(Mutex::new(Vec::new())));
        assert!(
            download(
                &Client::new(),
                &asset(),
                &["http://127.0.0.1:9/unavailable".into(), url],
                &mut |_| {}
            )
            .is_ok()
        );
        a.join().unwrap();
    }
    #[test]
    #[ignore = "public GitHub metadata, mirrors and real verified download"]
    fn live_accelerated_download() {
        let client = super::super::client().unwrap();
        let release = check(&client, API, "0.0.0").unwrap().unwrap();
        let asset = release.asset().unwrap();
        let sources = urls(&asset.browser_download_url);
        for (url, speed) in rank(&client, &sources, asset.size) {
            eprintln!(
                "PROBE {} {:?} KiB/s",
                reqwest::Url::parse(&url).unwrap().host_str().unwrap(),
                speed.map(|s| s / 1024.)
            );
        }
        std::thread::scope(|scope| {
            for url in sources.iter().skip(1) {
                let client = client.clone();
                scope.spawn(move || {
                    eprintln!(
                        "MIRROR_DETAIL {} {:?}",
                        reqwest::Url::parse(url).unwrap().host_str().unwrap(),
                        probe(&client, url, asset.size)
                    )
                });
            }
        });
        let available_mirrors: Vec<_> = rank(&client, &sources[1..], asset.size)
            .into_iter()
            .filter(|(_, speed)| speed.is_some())
            .take(2)
            .collect();
        let mut verified_mirrors = 0;
        for (url, _) in available_mirrors {
            match download(&client, asset, std::slice::from_ref(&url), &mut |_| {}) {
                Ok(bytes) => {
                    verify(&bytes, asset).unwrap();
                    verified_mirrors += 1;
                    eprintln!(
                        "MIRROR_FULL_VERIFIED {} {} bytes",
                        reqwest::Url::parse(&url).unwrap().host_str().unwrap(),
                        bytes.len()
                    );
                }
                Err(error) => eprintln!(
                    "MIRROR_FULL_UNAVAILABLE {} {error}",
                    reqwest::Url::parse(&url).unwrap().host_str().unwrap()
                ),
            }
        }
        assert!(
            verified_mirrors > 0,
            "no mirror completed a real checksum-verified payload"
        );
        let mut updates = 0;
        let bytes = download(&client, asset, &sources, &mut |_| updates += 1).unwrap();
        verify(&bytes, asset).unwrap();
        assert!(updates > 2);
        eprintln!(
            "PASS verified {} bytes across {} candidate sources",
            bytes.len(),
            sources.len()
        );
    }
}
