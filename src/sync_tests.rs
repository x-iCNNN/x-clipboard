use super::*;
use crate::model::{FormatData, Settings};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
struct ServerState {
    files: BTreeMap<String, (Vec<u8>, u64)>,
    writes: usize,
    conflicts: usize,
    fail_blob: bool,
    corrupt_blob: bool,
}
struct Server {
    address: String,
    state: Arc<Mutex<ServerState>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}/dav/", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(ServerState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let server = state.clone();
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &server),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("server accept failed: {e}"),
                }
            }
        });
        Self {
            address,
            state,
            stop,
            thread: Some(thread),
        }
    }
    fn config(&self) -> Config {
        Config {
            endpoint: self.address.clone(),
            username: "test".into(),
            password: "secret".into(),
            secret: "shared synthetic passphrase".into(),
            enabled: true,
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn serve(mut stream: TcpStream, state: &Mutex<ServerState>) {
    // Winsock accepts inherit the listener's nonblocking mode; read WouldBlock is not EOF.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        bytes.push(byte[0]);
        assert!(bytes.len() < 65536);
    }
    let headers = String::from_utf8(bytes).unwrap();
    let mut lines = headers.lines();
    let mut first = lines.next().unwrap().split_whitespace();
    let method = first.next().unwrap();
    let path = first.next().unwrap();
    let headers: BTreeMap<_, _> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    assert!(length < 200 * 1024 * 1024);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    let mut state = state.lock().unwrap();
    let mut code = 200;
    let mut response = Vec::new();
    let mut etag = None;
    if headers.get("authorization").map(String::as_str) != Some("Basic dGVzdDpzZWNyZXQ=") {
        code = 401;
    } else {
        match method {
            "MKCOL" => code = 405,
            "GET" | "HEAD" => {
                if let Some((bytes, version)) = state.files.get(path) {
                    response = bytes.clone();
                    etag = Some(format!("\"{version}\""));
                    if state.corrupt_blob && path.contains("/blobs/") && !response.is_empty() {
                        response[60] ^= 1;
                    }
                } else {
                    code = 404;
                }
            }
            "PUT" => {
                let previous = state.files.get(path);
                let conflict = headers
                    .get("if-none-match")
                    .is_some_and(|v| v == "*" && previous.is_some())
                    || headers
                        .get("if-match")
                        .is_some_and(|v| previous.is_none_or(|(_, n)| v != &format!("\"{n}\"")));
                if state.fail_blob && path.contains("/blobs/") {
                    code = 500;
                } else if conflict {
                    code = 412;
                } else if path.ends_with("index.bin") && state.conflicts > 0 {
                    state.conflicts -= 1;
                    code = 412;
                } else {
                    let version = previous.map(|(_, v)| v + 1).unwrap_or(1);
                    state.files.insert(path.into(), (body, version));
                    state.writes += 1;
                    code = 201;
                }
            }
            _ => code = 405,
        }
    }
    let etag = etag.map(|e| format!("ETag: {e}\r\n")).unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {code} Test\r\n{etag}Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.len()
    );
    stream.write_all(header.as_bytes()).unwrap();
    if method != "HEAD" {
        let _ = stream.write_all(&response);
    }
}
fn device() -> (tempfile::TempDir, HistoryStore) {
    let root = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(root.path()).unwrap();
    (root, store)
}
fn capture(store: &HistoryStore, text: &str) -> Entry {
    store
        .capture(&ClipContent::text(text), &Settings::default())
        .unwrap()
}
fn sync(store: &HistoryStore, server: &Server) {
    synchronize(store, &server.config()).unwrap();
}

#[test]
fn two_devices_rich_payload_metadata_deletion_and_offline_reconnect() {
    let server = Server::new();
    let (_a, a) = device();
    let (_b, b) = device();
    let (_c, c) = device();
    let mut content = ClipContent::text("private synthetic clipboard Ω");
    content.formats.push(FormatData {
        name: "text/html".into(),
        bytes: b"<b>synthetic</b>".to_vec(),
    });
    let entry = a.capture(&content, &Settings::default()).unwrap();
    a.protect(&entry.id, Some(true), None).unwrap();
    sync(&a, &server);
    sync(&b, &server);
    sync(&c, &server);
    let remote = b.list().unwrap().remove(0);
    assert!(remote.pinned);
    assert!(b.content(&remote.id).unwrap().formats == content.formats);
    assert_eq!(b.search("synthetic").unwrap().len(), 1);
    // Independent concurrent flag edits survive instead of overwriting the other flag.
    a.protect(&entry.id, Some(false), None).unwrap();
    b.protect(&remote.id, None, Some(true)).unwrap();
    sync(&a, &server);
    sync(&b, &server);
    sync(&a, &server);
    let merged = a.list().unwrap().remove(0);
    assert!(!merged.pinned && merged.favorite);
    a.delete(&[entry.id]).unwrap();
    sync(&a, &server);
    sync(&b, &server);
    sync(&c, &server);
    assert!(
        a.list().unwrap().is_empty()
            && b.list().unwrap().is_empty()
            && c.list().unwrap().is_empty()
    );
    // An explicit new copy revives a deleted item; stale offline snapshots do not.
    c.capture(&content, &Settings::default()).unwrap();
    sync(&c, &server);
    sync(&a, &server);
    assert_eq!(a.list().unwrap().len(), 1);
    assert!(!a.list().unwrap()[0].pinned && !a.list().unwrap()[0].favorite);
    let state = server.state.lock().unwrap();
    for (path, (bytes, _)) in &state.files {
        assert!(!path.contains(&content.fingerprint()));
        assert!(!bytes.windows(9).any(|w| w == b"synthetic"));
    }
}
#[test]
fn password_auth_corruption_upload_failure_and_cas_retry_are_safe() {
    let server = Server::new();
    let (_a, a) = device();
    let (_b, b) = device();
    capture(&a, "retained private fixture");
    server.state.lock().unwrap().fail_blob = true;
    assert!(synchronize(&a, &server.config()).is_err());
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .files
            .keys()
            .all(|p| !p.ends_with("index.bin"))
    );
    server.state.lock().unwrap().fail_blob = false;
    server.state.lock().unwrap().conflicts = 2;
    sync(&a, &server);
    let previous = server.state.lock().unwrap().files.clone();
    let mut wrong = server.config();
    wrong.secret = "different wrong shared secret".into();
    assert!(synchronize(&b, &wrong).is_err());
    assert!(b.list().unwrap().is_empty());
    assert_eq!(server.state.lock().unwrap().files, previous);
    wrong = server.config();
    wrong.password = "bad".into();
    assert!(synchronize(&b, &wrong).is_err());
    server.state.lock().unwrap().corrupt_blob = true;
    assert!(synchronize(&b, &server.config()).is_err());
    assert!(b.list().unwrap().is_empty());
    server.state.lock().unwrap().corrupt_blob = false;
    sync(&b, &server);
    assert_eq!(b.list().unwrap().len(), 1);
}
#[test]
fn local_retention_does_not_delete_remote_or_redownload_unchanged_items() {
    let server = Server::new();
    let (_a, a) = device();
    let (_b, b) = device();
    capture(&a, "old");
    capture(&a, "new");
    sync(&a, &server);
    let settings = Settings {
        max_entries: 1,
        ..Settings::default()
    };
    b.save_settings(&settings).unwrap();
    sync(&b, &server);
    assert_eq!(b.list().unwrap().len(), 1);
    assert_eq!(b.list().unwrap()[0].title, "new");
    let writes = server.state.lock().unwrap().writes;
    sync(&b, &server);
    assert_eq!(server.state.lock().unwrap().writes, writes);
    sync(&a, &server);
    assert_eq!(a.list().unwrap().len(), 2);
    let old = a
        .list()
        .unwrap()
        .into_iter()
        .find(|e| e.title == "old")
        .unwrap();
    a.protect(&old.id, Some(true), None).unwrap();
    sync(&a, &server);
    sync(&b, &server);
    assert!(
        b.list()
            .unwrap()
            .iter()
            .any(|entry| entry.title == "old" && entry.pinned)
    );
}
#[test]
fn concurrent_first_writers_converge_without_overwriting_each_other() {
    let server = Server::new();
    let (_a, a) = device();
    let (_b, b) = device();
    capture(&a, "device-a");
    capture(&b, "device-b");
    let config = server.config();
    let other = config.clone();
    let one = std::thread::spawn(move || {
        synchronize(&a, &config).unwrap();
        a
    });
    let two = std::thread::spawn(move || {
        synchronize(&b, &other).unwrap();
        b
    });
    let a = one.join().unwrap();
    let b = two.join().unwrap();
    sync(&a, &server);
    sync(&b, &server);
    assert_eq!(a.list().unwrap().len(), 2);
    assert_eq!(b.list().unwrap().len(), 2);
}
#[test]
fn credentials_survive_encrypted_reopen_without_plaintext_and_config_rejects_unsafe_urls() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    {
        let store = HistoryStore::open(root.path()).unwrap();
        store.sync_save_config(&server.config()).unwrap();
        store.checkpoint().unwrap();
    }
    let store = HistoryStore::open(root.path()).unwrap();
    assert_eq!(
        store.sync_config().unwrap().secret,
        "shared synthetic passphrase"
    );
    drop(store);
    let bytes = std::fs::read(root.path().join("clipboard.db")).unwrap();
    assert!(!bytes.windows(9).any(|w| w == b"synthetic"));
    for endpoint in [
        "http://example.com/dav/",
        "https://user:secret@example.com/",
        "https://example.com/?token=abc",
        "file:///test",
    ] {
        let mut config = server.config();
        config.endpoint = endpoint.into();
        assert!(config.validate().is_err());
    }
    let key = derive("same password", &[1; 16]).unwrap();
    let plain = b"fixture";
    let first = seal(&key, &[1; 16], plain, "index").unwrap();
    let second = seal(&key, &[1; 16], plain, "index").unwrap();
    assert_ne!(first, second);
    assert!(unseal(&key, &first, "other-context").is_err());
}

#[test]
fn repeated_webdav_directory_requests() {
    let server = Server::new();
    for round in 0..30 {
        let dav = Dav::new(&server.config()).unwrap();
        dav.ensure_folder("x-clipboard-v1/")
            .unwrap_or_else(|e| panic!("directory request {round}: {e:#}"));
    }
}

#[test]
fn clipboard_capture_waits_for_a_background_sync_transaction() {
    let (_root, store) = device();
    let background = store.fork().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        background
            .transaction(|| {
                tx.send(()).unwrap();
                std::thread::sleep(Duration::from_millis(50));
                Ok(())
            })
            .unwrap()
    });
    rx.recv().unwrap();
    let result = store.capture(
        &ClipContent::text("capture while sync holds its transaction"),
        &Settings::default(),
    );
    worker.join().unwrap();
    assert!(
        result.is_ok(),
        "background sync caused capture failure: {}",
        result.err().unwrap()
    );
}
