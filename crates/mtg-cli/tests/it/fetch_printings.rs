//! Goal: Default Cards (every printing) is fetched only on request, as the other bulk files are: checked against the listing before any download, resumed from a partial file, recorded with its checksum and skipped while current.

use crate::common;
use crate::fetch_scryfall::{FAST, partial, scryfall};

use std::fs;

use mtg_cli::cache::{Cache, PRINTINGS_TYPE, SCRYFALL_TYPES};
use mtg_cli::error::{CliError, FetchError};
use mtg_cli::fetch::scryfall::{BulkAction, fetch_scryfall, fetch_scryfall_kinds};
use mtg_cli::files::sha256_hex;

use common::{client, gzip_file_of, progress};

const ALL: [&str; 3] = [SCRYFALL_TYPES[0], SCRYFALL_TYPES[1], PRINTINGS_TYPE];

#[tokio::test]
async fn default_cards_come_only_when_asked_for_and_are_current_afterwards() {
    let (cards, rulings, printings) = (
        gzip_file_of(20_000, 31),
        gzip_file_of(5_000, 32),
        gzip_file_of(90_000, 33),
    );
    let (server, remote) = scryfall(1, &cards, &rulings, crate::fetch_scryfall::standard).await;
    remote.publish_printings(1, &printings);
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let listing = server.url("/bulk-data");

    let plain = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();
    assert_eq!(plain.len(), 2);
    assert!(
        !cache
            .manifest()
            .unwrap()
            .scryfall
            .contains_key(PRINTINGS_TYPE)
    );
    assert!(server.hits("/files/default").is_empty());

    let all = fetch_scryfall_kinds(&http, &cache, &listing, &ALL, false, &progress)
        .await
        .unwrap();
    let actions: Vec<_> = all
        .iter()
        .map(|outcome| (outcome.kind, outcome.action))
        .collect();
    assert_eq!(
        actions,
        [
            ("oracle_cards", BulkAction::UpToDate),
            ("rulings", BulkAction::UpToDate),
            ("default_cards", BulkAction::Downloaded { resumed_from: 0 }),
        ]
    );
    let record = &cache.manifest().unwrap().scryfall[PRINTINGS_TYPE];
    assert_eq!(record.file.sha256, sha256_hex(&printings));
    assert_eq!(fs::read(cache.path(&record.file.file)).unwrap(), printings);

    let again = fetch_scryfall_kinds(&http, &cache, &listing, &ALL, false, &progress)
        .await
        .unwrap();
    assert!(
        again
            .iter()
            .all(|outcome| outcome.action == BulkAction::UpToDate)
    );
    assert_eq!(server.hits("/files/default").len(), 1);
}

#[tokio::test]
async fn a_partial_default_cards_file_is_resumed() {
    let (cards, rulings, printings) = (
        gzip_file_of(20_000, 34),
        gzip_file_of(5_000, 35),
        gzip_file_of(120_000, 36),
    );
    let (server, remote) = scryfall(1, &cards, &rulings, crate::fetch_scryfall::standard).await;
    remote.publish_printings(1, &printings);
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    cache.prepare("scryfall").unwrap();
    fs::write(
        partial(&cache, "default-cards-1.jsonl.gz"),
        &printings[..70_000],
    )
    .unwrap();
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let all = fetch_scryfall_kinds(
        &http,
        &cache,
        &server.url("/bulk-data"),
        &ALL,
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(
        all[2].action,
        BulkAction::Downloaded {
            resumed_from: 70_000
        }
    );
    assert_eq!(all[2].record.file.sha256, sha256_hex(&printings));
    let ranges: Vec<_> = server
        .hits("/files/default")
        .iter()
        .map(common::server::Request::range_start)
        .collect();
    assert_eq!(ranges, [Some(70_000)]);
    assert!(!partial(&cache, "default-cards-1.jsonl.gz").exists());
}

#[tokio::test]
async fn a_listing_without_default_cards_stops_the_request_before_any_download() {
    let (cards, rulings) = (gzip_file_of(20_000, 37), gzip_file_of(5_000, 38));
    let (server, _) = scryfall(1, &cards, &rulings, crate::fetch_scryfall::standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall_kinds(
        &http,
        &cache,
        &server.url("/bulk-data"),
        &ALL,
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&error, CliError::Fetch(FetchError::Invalid { reason, .. }) if reason.contains("default_cards")),
        "{error}"
    );
    assert!(server.hits("/files/").is_empty());
    assert!(cache.manifest().unwrap().scryfall.is_empty());
}
