//! Goal: the committed TypeScript bindings equal what the Rust wire types generate today.
//!
//! Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol bindings::`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mtg_protocol::bindings::{GENERATED_DIR, export_to};

use crate::common::generated;

fn committed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(GENERATED_DIR)
}

fn read_all(dir: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("directory entry").path();
            let name = path.file_name().expect("file name").to_string_lossy();
            (
                name.into_owned(),
                fs::read_to_string(&path).expect("utf8 file"),
            )
        })
        .collect()
}

#[test]
fn committed_bindings_match_generated() {
    let committed = committed_dir();
    if std::env::var_os("MTG_UPDATE_BINDINGS").is_some() {
        export_to(&committed).expect("regenerate bindings");
        return;
    }
    let fresh = generated();
    let old = read_all(&committed);

    let missing: Vec<_> = fresh.keys().filter(|k| !old.contains_key(*k)).collect();
    let stale: Vec<_> = old.keys().filter(|k| !fresh.contains_key(*k)).collect();
    let changed: Vec<_> = fresh
        .iter()
        .filter(|(name, text)| old.get(*name).is_some_and(|o| o != *text))
        .map(|(name, _)| name)
        .collect();
    assert!(
        missing.is_empty() && stale.is_empty() && changed.is_empty(),
        "bindings drifted (missing {missing:?}, stale {stale:?}, changed {changed:?}); \
         run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol bindings::"
    );
}

#[test]
fn bindings_cover_the_wire_surface() {
    let files = generated();
    for needed in [
        "ClientMessage.ts",
        "ServerMessage.ts",
        "StateView.ts",
        "EventBatch.ts",
        "Decision.ts",
        "Answer.ts",
        "Event.ts",
        "index.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
    }
    assert!(files["index.ts"].contains("PROTOCOL_VERSION"));
    assert!(
        files.values().all(|text| !text.contains("bigint")),
        "ids must be exported as number, not bigint"
    );
}
