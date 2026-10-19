//! Goal: the MTGJSON fetch picks decks by type, name and age, asks about a cached deck only when the index build changed and then conditionally, decodes gzip transfers, skips decks the server lacks without losing the rest, keeps what it fetched when the server fails, and spaces its requests on a paused clock.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mtg_cli::cache::Cache;
use mtg_cli::error::{CliError, FetchError};
use mtg_cli::fetch::Endpoints;
use mtg_cli::fetch::decks::{DeckFetchReport, DeckFilter, fetch_decks};
use mtg_cli::files::sha256_hex;
use serde_json::json;

use common::server::{self, Reply, Request, Server};
use common::{client, gzip, progress};

const FAST: Duration = Duration::from_millis(1);

/// A fake MTGJSON: the deck index of one build and the deck files, each with a revision for its `ETag`.
#[derive(Clone, Default)]
struct Remote(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    build: u32,
    /// `(file name, deck name, type, release date)` in index order.
    index: Vec<(String, String, String, String)>,
    /// File name to `(revision, body)`.
    decks: BTreeMap<String, (u32, Vec<u8>)>,
    /// File names that answer 500.
    failing: Vec<String>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A deck file body that the importer would also accept.
fn deck_json(name: &str, revision: u32) -> Vec<u8> {
    json!({"meta": {"version": "5.3.0"}, "data": {
        "name": name, "code": "TST", "type": "Commander Deck", "releaseDate": "2020-01-01",
        "commander": [], "mainBoard": [{"count": revision, "uuid": "u", "name": "Sol Ring"}], "sideBoard": []
    }})
    .to_string()
    .into_bytes()
}

impl Remote {
    fn add(&self, file: &str, name: &str, kind: &str, released: &str) {
        let mut state = lock(&self.0);
        state
            .index
            .push((file.into(), name.into(), kind.into(), released.into()));
        state.decks.insert(file.into(), (1, deck_json(name, 1)));
    }

    /// A new MTGJSON build; the decks named change their content.
    fn rebuild(&self, changed: &[&str]) {
        let mut state = lock(&self.0);
        state.build += 1;
        for file in changed {
            let name = state
                .index
                .iter()
                .find(|entry| entry.0 == *file)
                .map(|entry| entry.1.clone())
                .unwrap_or_default();
            if let Some((revision, body)) = state.decks.get_mut(*file) {
                *revision += 1;
                *body = deck_json(&name, *revision);
            }
        }
    }

    fn answer(&self, request: &Request) -> Reply {
        let state = lock(&self.0);
        let (etag, body) = if request.path == "/api/v5/DeckList.json" {
            let entries: Vec<_> = state
                .index
                .iter()
                .map(|(file, name, kind, released)| {
                    json!({"code": "TST", "fileName": file, "name": name, "releaseDate": released, "type": kind})
                })
                .collect();
            let body = json!({"meta": {"date": "2026-10-18", "version": format!("5.3.0+b{}", state.build)}, "data": entries});
            (
                format!("W/\"index-{}\"", state.build),
                body.to_string().into_bytes(),
            )
        } else if let Some(file) = request
            .path
            .strip_prefix("/api/v5/decks/")
            .and_then(|rest| rest.strip_suffix(".json"))
        {
            if state.failing.iter().any(|failing| failing == file) {
                return Reply::status(500);
            }
            match state.decks.get(file) {
                Some((revision, body)) => (format!("W/\"{file}-{revision}\""), body.clone()),
                None => return Reply::status(404),
            }
        } else {
            return Reply::status(404);
        };
        if request.header("if-none-match") == Some(etag.as_str()) {
            return Reply::status(304).header("ETag", etag);
        }
        if request
            .header("accept-encoding")
            .is_some_and(|v| v.contains("gzip"))
        {
            return Reply::ok(gzip(&body))
                .header("ETag", etag)
                .header("Content-Encoding", "gzip");
        }
        Reply::ok(body).header("ETag", etag)
    }
}

fn endpoints(server: &Server) -> Endpoints {
    Endpoints {
        scryfall_listing: server.url("/bulk-data"),
        mtgjson_index: server.url("/api/v5/DeckList.json"),
        mtgjson_decks: server.url("/api/v5/decks/"),
    }
}

/// A fake MTGJSON with six decks: four Commander precons of different ages and two others.
async fn mtgjson() -> (Server, Remote) {
    let remote = Remote::default();
    remote.add("Old_TST", "Old Guard", "Commander Deck", "2012-06-15");
    remote.add("New_TST", "New Guard", "Commander Deck", "2026-10-02");
    remote.add("Mid_TST", "Middle Guard", "Commander Deck", "2020-01-01");
    remote.add("Alpha_TST", "Alpha Guard", "Commander Deck", "2016-11-11");
    remote.add("Theme_TST", "Guard Theme", "Theme Deck", "2007-07-13");
    remote.add("Intro_TST", "Guard Intro", "Intro Pack", "2010-01-01");
    let handler = remote.clone();
    (
        server::start(move |request| handler.answer(request)).await,
        remote,
    )
}

fn deck_hits(server: &Server) -> Vec<String> {
    server
        .hits("/api/v5/decks/")
        .iter()
        .map(|request| request.path.trim_start_matches("/api/v5/decks/").to_owned())
        .collect()
}

fn run_counts(report: &DeckFetchReport) -> (usize, usize, usize, usize, usize) {
    (
        report.selected,
        report.downloaded,
        report.not_modified,
        report.current,
        report.missing.len(),
    )
}

#[tokio::test]
async fn the_newest_commander_decks_are_fetched_up_to_the_limit_and_the_rest_later() {
    let (server, remote) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, log) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter {
        limit: Some(2),
        ..DeckFilter::default()
    };

    let report = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&report), (2, 2, 0, 0, 0));
    assert_eq!(deck_hits(&server), ["New_TST.json", "Mid_TST.json"]);
    let manifest = cache.manifest().unwrap();
    let cached: Vec<_> = manifest.mtgjson.decks.keys().cloned().collect();
    assert_eq!(cached, ["Mid_TST", "New_TST"]);
    let record = &manifest.mtgjson.decks["New_TST"];
    let body = lock(&remote.0).decks["New_TST"].1.clone();
    assert_eq!(fs::read(cache.path(&record.file.file)).unwrap(), body);
    assert_eq!(record.file.sha256, sha256_hex(&body));
    assert_eq!(record.build.as_deref(), Some("5.3.0+b0"));
    assert_eq!(record.validators.etag.as_deref(), Some("W/\"New_TST-1\""));
    let index = manifest.mtgjson.index.unwrap();
    assert_eq!(index.build.as_deref(), Some("5.3.0+b0"));
    assert!(cache.path(&index.file.file).exists());
    for request in server.log() {
        assert_eq!(request.header("user-agent"), Some("mtg-cli-test/1.0"));
        assert!(
            request
                .header("accept")
                .unwrap()
                .contains("application/json")
        );
    }
    assert!(
        log.text().contains("mtgjson: 2 of 6 decks selected"),
        "{}",
        log.text()
    );

    let all = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&all), (4, 2, 0, 2, 0));
    assert_eq!(cache.manifest().unwrap().mtgjson.decks.len(), 4);
}

#[tokio::test]
async fn a_second_run_on_the_same_build_asks_only_for_the_index() {
    let (server, _) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter::default();
    fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();
    let before = cache.manifest().unwrap();

    let report = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&report), (4, 0, 0, 4, 0));
    assert_eq!(
        deck_hits(&server).len(),
        4,
        "the decks were requested only once"
    );
    let index_requests = server.hits("/api/v5/DeckList.json");
    assert_eq!(index_requests.len(), 2);
    assert_eq!(
        index_requests[1].header("if-none-match"),
        Some("W/\"index-0\"")
    );
    assert_eq!(cache.manifest().unwrap(), before);
}

#[tokio::test]
async fn a_new_build_asks_about_each_deck_and_downloads_only_those_that_changed() {
    let (server, remote) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter::default();
    fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();
    remote.rebuild(&["Mid_TST"]);

    let report = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&report), (4, 1, 3, 0, 0));
    let second_round: Vec<Request> = server.hits("/api/v5/decks/").split_off(4);
    assert_eq!(second_round.len(), 4);
    assert!(
        second_round
            .iter()
            .all(|r| r.header("if-none-match").is_some())
    );
    let manifest = cache.manifest().unwrap();
    assert_eq!(
        manifest.mtgjson.decks["Mid_TST"].validators.etag.as_deref(),
        Some("W/\"Mid_TST-2\"")
    );
    assert!(
        manifest
            .mtgjson
            .decks
            .values()
            .all(|d| d.build.as_deref() == Some("5.3.0+b1"))
    );
    let body = fs::read(cache.path("mtgjson/decks/Mid_TST.json")).unwrap();
    assert_eq!(body, lock(&remote.0).decks["Mid_TST"].1);

    let third = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();
    assert_eq!(run_counts(&third), (4, 0, 0, 4, 0));
}

#[tokio::test]
async fn forcing_downloads_every_deck_without_asking_whether_it_changed() {
    let (server, _) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter {
        only: vec!["Middle".into()],
        ..DeckFilter::default()
    };
    fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    let forced = fetch_decks(&http, &cache, &endpoints(&server), &filter, true, &progress)
        .await
        .unwrap();

    assert_eq!(run_counts(&forced), (1, 1, 0, 0, 0));
    for request in server.log() {
        assert_eq!(request.header("if-none-match"), None, "{}", request.path);
    }
}

#[tokio::test]
async fn name_and_type_filters_decide_which_files_are_requested() {
    let (server, _) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter {
        types: vec!["theme deck".into(), "Intro Pack".into()],
        only: vec!["guard".into()],
        limit: None,
    };

    let report = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&report), (2, 2, 0, 0, 0));
    assert_eq!(deck_hits(&server), ["Intro_TST.json", "Theme_TST.json"]);
}

#[tokio::test]
async fn a_deck_the_server_lacks_or_garbles_is_reported_and_the_others_still_arrive() {
    let (server, remote) = mtgjson().await;
    {
        let mut state = lock(&remote.0);
        state.decks.remove("Mid_TST");
        state
            .decks
            .insert("Old_TST".into(), (1, b"{ this is not json".to_vec()));
    }
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, log) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let report = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(run_counts(&report), (4, 2, 0, 0, 2));
    let mut missing = report.missing.clone();
    missing.sort();
    assert_eq!(missing, ["Mid_TST", "Old_TST"]);
    let cached: Vec<_> = cache
        .manifest()
        .unwrap()
        .mtgjson
        .decks
        .keys()
        .cloned()
        .collect();
    assert_eq!(cached, ["Alpha_TST", "New_TST"]);
    assert!(!cache.path("mtgjson/decks/Old_TST.json").exists());
    assert!(log.text().contains("skipping Mid_TST"), "{}", log.text());
    assert_eq!(
        server.hits("/api/v5/decks/Mid_TST").len(),
        1,
        "a missing deck is not retried"
    );
}

#[tokio::test]
async fn gzip_transfers_are_decoded_before_they_are_cached() {
    let (server, remote) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let filter = DeckFilter {
        only: vec!["New Guard".into()],
        ..DeckFilter::default()
    };

    fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &filter,
        false,
        &progress,
    )
    .await
    .unwrap();

    let sent = server.hits("/api/v5/decks/New_TST").remove(0);
    assert!(sent.header("accept-encoding").unwrap().contains("gzip"));
    let stored = fs::read(cache.path("mtgjson/decks/New_TST.json")).unwrap();
    assert_eq!(stored, lock(&remote.0).decks["New_TST"].1);
    assert_eq!(
        mtg_import::mtgjson::parse_deck(stored.as_slice())
            .unwrap()
            .name,
        "New Guard"
    );
}

#[tokio::test(start_paused = true)]
async fn requests_are_a_gap_apart_on_the_server_clock() {
    let (server, _) = mtgjson().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let gap = Duration::from_millis(100);
    let http = client(&progress, gap, 3, FAST);

    fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap();

    let times: Vec<_> = server.log().iter().map(|request| request.at).collect();
    assert_eq!(times.len(), 5, "the index and four decks");
    for pair in times.windows(2) {
        assert!(
            pair[1] - pair[0] >= gap,
            "requests came {:?} apart",
            pair[1] - pair[0]
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_failing_server_stops_the_run_but_keeps_the_decks_fetched_so_far() {
    let (server, remote) = mtgjson().await;
    lock(&remote.0).failing.push("Mid_TST".into());
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(
        &progress,
        Duration::from_millis(100),
        3,
        Duration::from_secs(1),
    );

    let error = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(
            error,
            CliError::Fetch(FetchError::Status { status: 500, .. })
        ),
        "{error}"
    );
    assert_eq!(server.hits("/api/v5/decks/Mid_TST").len(), 3);
    let cached: Vec<_> = cache
        .manifest()
        .unwrap()
        .mtgjson
        .decks
        .keys()
        .cloned()
        .collect();
    assert_eq!(cached, ["New_TST"], "the deck before the failure is kept");

    lock(&remote.0).failing.clear();
    let rest = fetch_decks(
        &http,
        &cache,
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap();

    assert_eq!(
        run_counts(&rest),
        (4, 3, 0, 1, 0),
        "only the rest is fetched"
    );
}

#[tokio::test]
async fn an_index_that_is_not_an_index_is_refused() {
    let server = server::start(|request| {
        if request.path.ends_with("DeckList.json") {
            Reply::ok("<html>maintenance</html>")
        } else {
            Reply::status(500)
        }
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_decks(
        &http,
        &Cache::new(dir.path()),
        &endpoints(&server),
        &DeckFilter::default(),
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("not a deck index"), "{error}");
}
