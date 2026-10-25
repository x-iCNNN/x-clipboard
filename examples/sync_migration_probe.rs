//! Upgrade a COPY of an older synthetic review fixture and verify unchanged history.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};
use x_clipboard::{
    storage::HistoryStore,
    sync::{self, Config},
};
#[derive(Deserialize)]
struct Marker {
    synthetic: bool,
    schema: u32,
    ids: BTreeMap<String, String>,
}
fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("Specify a COPY of an old synthetic fixture")?,
    );
    let endpoint = std::env::args()
        .nth(2)
        .context("Specify the loopback WebDAV fixture")?;
    let marker: Marker =
        serde_json::from_slice(&std::fs::read(root.join("UI-REVIEW-FIXTURE.json"))?)?;
    ensure!(
        marker.synthetic && marker.schema == 1,
        "Refusing a real user database"
    );
    let base = url::Url::parse(&endpoint)?;
    ensure!(
        matches!(base.host_str(), Some("127.0.0.1" | "localhost")),
        "Requires a loopback fixture"
    );
    let endpoint = base
        .join(&format!("migration-{}/", uuid::Uuid::new_v4()))?
        .to_string();
    reqwest::blocking::Client::builder()
        .no_proxy()
        .build()?
        .request("MKCOL".parse()?, &endpoint)
        .basic_auth("sync-test", Some("fixture-login"))
        .send()?
        .error_for_status()?;
    let store = HistoryStore::open(&root)?;
    let before = store.list()?;
    ensure!(
        before.len() == marker.ids.len()
            && before
                .iter()
                .all(|entry| marker.ids.values().any(|id| *id == entry.id)),
        "Old fixture did not reopen intact"
    );
    ensure!(
        store.sync_config()?.endpoint.is_empty(),
        "Fixture already contains sync configuration"
    );
    let expected = serde_json::to_vec(&before)?;
    let payloads = before
        .iter()
        .map(|e| store.content(&e.id).map(|c| c.fingerprint()))
        .collect::<Result<Vec<_>>>()?;
    sync::synchronize(
        &store,
        &Config {
            endpoint,
            username: "sync-test".into(),
            password: "fixture-login".into(),
            secret: "migration synthetic shared secret".into(),
            enabled: false,
        },
    )?;
    ensure!(
        serde_json::to_vec(&store.list()?)? == expected,
        "Upgrade changed existing IDs, timestamps, flags or order"
    );
    for (entry, hash) in before.iter().zip(payloads) {
        ensure!(
            store.content(&entry.id)?.fingerprint() == hash,
            "Upgrade changed a payload"
        );
    }
    store.checkpoint()?;
    println!(
        "PASS: v0.2 fixture upgrade, unchanged IDs/timestamps/flags/order/payloads, first encrypted sync"
    );
    Ok(())
}
