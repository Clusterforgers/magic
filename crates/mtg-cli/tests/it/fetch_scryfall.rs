//! Goal: the Scryfall download sends the headers Scryfall requires, skips files whose `updated_at` is current, resumes partial files with Range requests whatever the server does with them, spaces its requests on a paused clock, records checksums, and ends in a clear error without a half-written file when the server fails.

use crate::common;

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mtg_cli::cache::Cache;
use mtg_cli::error::{CliError, FetchError};
use mtg_cli::fetch::scryfall::{BulkAction, fetch_scryfall};
use mtg_cli::files::sha256_hex;
use rstest::rstest;
use serde_json::json;

use common::server::{self, Reply, Request, Server, serve_ranges};
use common::{client, gzip_file_of, progress};

pub(crate) const FAST: Duration = Duration::from_millis(1);

/// What the fake Scryfall serves now: a listing and the files it points to.
#[derive(Clone, Default)]
pub(crate) struct Remote(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    base: String,
    listing: String,
    files: HashMap<String, Vec<u8>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Remote {
    /// Publishes version `version` of both bulk files and a listing that points to them.
    pub(crate) fn publish(&self, version: u32, cards: &[u8], rulings: &[u8]) {
        self.publish_as(version, version, cards, rulings);
    }

    /// Like `publish`, but the file names carry `files` while `updated_at` carries `version`.
    pub(crate) fn publish_as(&self, version: u32, files: u32, cards: &[u8], rulings: &[u8]) {
        let mut state = lock(&self.0);
        let base = state.base.clone();
        let entry = |kind: &str, file: String, bytes: &[u8]| {
            json!({
                "object": "bulk_data",
                "type": kind,
                "updated_at": format!("2026-10-18T21:02:00.{version:03}+00:00"),
                "jsonl_download_uri": format!("{base}/files/{file}"),
                "compressed_size": bytes.len(),
            })
        };
        let cards_file = format!("oracle-cards-{files}.jsonl.gz");
        let rulings_file = format!("rulings-{files}.jsonl.gz");
        state.listing = json!({
            "object": "list",
            "has_more": false,
            "data": [entry("oracle_cards", cards_file.clone(), cards), entry("rulings", rulings_file.clone(), rulings)],
        })
        .to_string();
        state
            .files
            .insert(format!("/files/{cards_file}"), cards.to_vec());
        state
            .files
            .insert(format!("/files/{rulings_file}"), rulings.to_vec());
    }

    /// Adds the Default Cards entry, and the file it points to, to the listing.
    pub(crate) fn publish_printings(&self, version: u32, bytes: &[u8]) {
        let mut state = lock(&self.0);
        let file = format!("default-cards-{version}.jsonl.gz");
        let mut listing: serde_json::Value = serde_json::from_str(&state.listing)
            .unwrap_or_else(|e| panic!("the listing parses: {e}"));
        let entries = listing["data"].as_array_mut();
        entries
            .unwrap_or_else(|| panic!("the listing has data"))
            .push(json!({
                "object": "bulk_data",
                "type": "default_cards",
                "updated_at": format!("2026-10-18T21:02:00.{version:03}+00:00"),
                "jsonl_download_uri": format!("{}/files/{file}", state.base),
                "compressed_size": bytes.len(),
            }));
        state.listing = listing.to_string();
        state.files.insert(format!("/files/{file}"), bytes.to_vec());
    }

    /// The default behaviour: the listing, files with range support, 404 for the rest.
    pub(crate) fn answer(&self, request: &Request) -> Reply {
        let state = lock(&self.0);
        if request.path == "/bulk-data" {
            return Reply::ok(state.listing.clone()).header("Content-Type", "application/json");
        }
        match state.files.get(&request.path) {
            Some(body) => serve_ranges(request, body),
            None => Reply::status(404),
        }
    }

    pub(crate) fn file(&self, path: &str) -> Vec<u8> {
        lock(&self.0).files.get(path).cloned().unwrap_or_default()
    }
}

/// A fake Scryfall that answers with `behave`, which may delegate to [`Remote::answer`].
pub(crate) async fn scryfall(
    version: u32,
    cards: &[u8],
    rulings: &[u8],
    behave: impl Fn(&Remote, &Request) -> Reply + Send + Sync + 'static,
) -> (Server, Remote) {
    let bound = server::bind().await;
    let remote = Remote::default();
    lock(&remote.0).base = bound.base();
    remote.publish(version, cards, rulings);
    let handler = remote.clone();
    let server = bound.serve(move |request| behave(&handler, request));
    (server, remote)
}

pub(crate) fn standard(remote: &Remote, request: &Request) -> Reply {
    remote.answer(request)
}

pub(crate) fn partial(cache: &Cache, name: &str) -> std::path::PathBuf {
    cache.path(&format!("scryfall/{name}.part"))
}

#[tokio::test]
async fn the_bulk_files_arrive_with_the_headers_scryfall_requires_and_a_checksum() {
    let cards = gzip_file_of(150_000, 1);
    let rulings = gzip_file_of(40_000, 2);
    let (server, _) = scryfall(1, &cards, &rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, log) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let outcomes = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    let kinds: Vec<_> = outcomes.iter().map(|o| o.kind).collect();
    assert_eq!(kinds, ["oracle_cards", "rulings"]);
    for (outcome, bytes) in outcomes.iter().zip([&cards, &rulings]) {
        assert_eq!(outcome.action, BulkAction::Downloaded { resumed_from: 0 });
        assert_eq!(
            &fs::read(cache.path(&outcome.record.file.file)).unwrap(),
            bytes
        );
        assert_eq!(outcome.record.file.sha256, sha256_hex(bytes));
        assert_eq!(outcome.record.file.size, bytes.len() as u64);
        assert_eq!(outcome.record.updated_at, "2026-10-18T21:02:00.001+00:00");
    }
    let manifest = cache.manifest().unwrap();
    assert_eq!(manifest.scryfall["oracle_cards"], outcomes[0].record);
    assert_eq!(manifest.scryfall["rulings"], outcomes[1].record);

    let requests = server.log();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_eq!(request.method, "GET");
        assert_eq!(request.header("user-agent"), Some("mtg-cli-test/1.0"));
        assert!(
            request
                .header("accept")
                .unwrap()
                .contains("application/json")
        );
    }
    for request in server.hits("/files/") {
        assert_eq!(request.header("accept-encoding"), Some("identity"));
        assert_eq!(request.header("range"), None);
    }
    let names: Vec<_> = fs::read_dir(cache.path("scryfall"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        names.iter().all(|name| !name.contains(".part")),
        "{names:?}"
    );
    assert!(log.text().contains("oracle_cards saved"), "{}", log.text());
}

#[tokio::test]
async fn files_whose_updated_at_is_current_are_not_downloaded_again() {
    let (cards, rulings) = (gzip_file_of(60_000, 3), gzip_file_of(20_000, 4));
    let (server, _) = scryfall(1, &cards, &rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let listing = server.url("/bulk-data");

    let first = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();
    let before = cache.manifest().unwrap();
    let second = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();

    assert!(second.iter().all(|o| o.action == BulkAction::UpToDate));
    assert_eq!(first[0].record, second[0].record);
    assert_eq!(cache.manifest().unwrap(), before);
    assert_eq!(server.hits("/bulk-data").len(), 2);
    assert_eq!(server.hits("/files/").len(), 2, "no second download");

    let forced = fetch_scryfall(&http, &cache, &listing, true, &progress)
        .await
        .unwrap();
    assert!(
        forced
            .iter()
            .all(|o| matches!(o.action, BulkAction::Downloaded { .. }))
    );
    assert_eq!(server.hits("/files/").len(), 4);
}

#[tokio::test]
async fn a_damaged_cache_file_is_fetched_again_even_when_updated_at_matches() {
    let (cards, rulings) = (gzip_file_of(60_000, 5), gzip_file_of(20_000, 6));
    let (server, _) = scryfall(1, &cards, &rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let listing = server.url("/bulk-data");
    let first = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();
    let path = cache.path(&first[1].record.file.file);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();

    let again = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();

    assert_eq!(again[0].action, BulkAction::UpToDate);
    assert!(matches!(again[1].action, BulkAction::Downloaded { .. }));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[tokio::test]
async fn a_new_updated_at_downloads_the_new_file_and_removes_the_old_one() {
    let (old_cards, old_rulings) = (gzip_file_of(50_000, 7), gzip_file_of(10_000, 8));
    let (server, remote) = scryfall(1, &old_cards, &old_rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let listing = server.url("/bulk-data");
    let first = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();
    let old_path = cache.path(&first[0].record.file.file);

    let (new_cards, new_rulings) = (gzip_file_of(55_000, 9), gzip_file_of(12_000, 10));
    remote.publish(2, &new_cards, &new_rulings);
    let second = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();

    assert!(
        second
            .iter()
            .all(|o| matches!(o.action, BulkAction::Downloaded { .. }))
    );
    assert_eq!(
        second[0].record.file.file,
        "scryfall/oracle-cards-2.jsonl.gz"
    );
    assert_eq!(
        fs::read(cache.path(&second[0].record.file.file)).unwrap(),
        new_cards
    );
    assert!(!old_path.exists(), "the superseded file is removed");
    assert_eq!(
        cache.manifest().unwrap().scryfall["rulings"].file.sha256,
        sha256_hex(&new_rulings)
    );
}

#[tokio::test]
async fn a_newer_updated_at_downloads_again_even_when_the_file_name_stays_the_same() {
    let (old_cards, old_rulings) = (gzip_file_of(50_000, 31), gzip_file_of(10_000, 32));
    let (server, remote) = scryfall(1, &old_cards, &old_rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);
    let listing = server.url("/bulk-data");
    fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();

    let (new_cards, new_rulings) = (gzip_file_of(52_000, 33), gzip_file_of(11_000, 34));
    remote.publish_as(2, 1, &new_cards, &new_rulings);
    let second = fetch_scryfall(&http, &cache, &listing, false, &progress)
        .await
        .unwrap();

    assert!(
        second
            .iter()
            .all(|o| matches!(o.action, BulkAction::Downloaded { .. }))
    );
    let path = cache.path("scryfall/oracle-cards-1.jsonl.gz");
    assert_eq!(
        fs::read(&path).unwrap(),
        new_cards,
        "the file was replaced, not removed"
    );
    assert_eq!(
        cache.manifest().unwrap().scryfall["oracle_cards"].updated_at,
        "2026-10-18T21:02:00.002+00:00"
    );
}

/// How the fake server treats a `Range` request.
#[derive(Clone, Copy, Debug)]
enum Mode {
    Honours,
    Ignores,
    WrongOffset,
}

/// What is already in the partial file.
#[derive(Clone, Copy, Debug)]
enum Part {
    Prefix(usize),
    Whole,
    WholePlusJunk(usize),
}

impl Part {
    fn bytes(self, whole: &[u8]) -> Vec<u8> {
        match self {
            Part::Prefix(length) => whole[..length].to_vec(),
            Part::Whole => whole.to_vec(),
            Part::WholePlusJunk(extra) => [whole, &vec![7; extra]].concat(),
        }
    }
}

/// What a correct client does about a partial file: the bytes it keeps and the `Range` offsets it sends.
fn expected(mode: Mode, part: Part, whole: usize) -> (usize, Vec<Option<usize>>) {
    match (mode, part) {
        (Mode::Honours, Part::Prefix(kept)) => (kept, vec![Some(kept)]),
        (Mode::Ignores, Part::Prefix(offered)) => (0, vec![Some(offered)]),
        (Mode::Honours, Part::Whole) => (whole, vec![Some(whole)]),
        (Mode::Honours, Part::WholePlusJunk(extra)) => (0, vec![Some(whole + extra), None]),
        (Mode::WrongOffset, Part::Prefix(offered)) => (0, vec![Some(offered), None]),
        other => panic!("no expectation for {other:?}"),
    }
}

#[rstest]
#[case::resumes(Mode::Honours, Part::Prefix(70_000))]
#[case::server_ignores_the_range(Mode::Ignores, Part::Prefix(70_000))]
#[case::part_is_already_the_whole_file(Mode::Honours, Part::Whole)]
#[case::part_is_longer_than_the_file(Mode::Honours, Part::WholePlusJunk(500))]
#[case::server_resumes_at_another_offset(Mode::WrongOffset, Part::Prefix(70_000))]
#[tokio::test]
async fn a_partial_file_is_resumed_or_replaced_never_spliced_wrongly(
    #[case] mode: Mode,
    #[case] part: Part,
) {
    let (cards, rulings) = (gzip_file_of(120_000, 11), gzip_file_of(10_000, 12));
    let (server, _) = scryfall(1, &cards, &rulings, move |remote, request| {
        let ranged = request.range_start().is_some() && request.path.contains("oracle");
        match (mode, ranged) {
            (Mode::Ignores, true) => Reply::ok(remote.file(&request.path)),
            (Mode::WrongOffset, true) => {
                let body = remote.file(&request.path);
                Reply::status(206)
                    .body(&body[..1000])
                    .header("Content-Range", format!("bytes 0-999/{}", body.len()))
            }
            _ => remote.answer(request),
        }
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    cache.prepare("scryfall").unwrap();
    fs::write(
        partial(&cache, "oracle-cards-1.jsonl.gz"),
        part.bytes(&cards),
    )
    .unwrap();
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let outcomes = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    let (kept, ranges) = expected(mode, part, cards.len());
    assert_eq!(
        outcomes[0].action,
        BulkAction::Downloaded {
            resumed_from: kept as u64
        },
        "{mode:?} {part:?}"
    );
    assert_eq!(
        fs::read(cache.path(&outcomes[0].record.file.file)).unwrap(),
        cards
    );
    assert_eq!(outcomes[0].record.file.sha256, sha256_hex(&cards));
    let seen: Vec<Option<usize>> = server
        .hits("/files/oracle")
        .iter()
        .map(Request::range_start)
        .collect();
    assert_eq!(seen, ranges, "{mode:?} {part:?}");
    assert!(!partial(&cache, "oracle-cards-1.jsonl.gz").exists());
}

#[tokio::test]
async fn a_connection_that_breaks_mid_download_is_resumed_by_the_retry() {
    let (cards, rulings) = (gzip_file_of(200_000, 13), gzip_file_of(10_000, 14));
    let broke = Arc::new(AtomicUsize::new(0));
    let flag = broke.clone();
    let (server, remote) = scryfall(1, &cards, &rulings, move |remote, request| {
        if request.path.contains("oracle") && flag.fetch_add(1, Ordering::SeqCst) == 0 {
            let body = remote.file(&request.path);
            return Reply::ok(body.clone()).cut_after(body.len() / 2);
        }
        remote.answer(request)
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, log) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let outcomes = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    let hits = server.hits("/files/oracle");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].range_start(), None);
    let resumed = hits[1].range_start().unwrap();
    assert!(
        resumed > 0 && resumed <= cards.len() / 2,
        "resumed at {resumed}"
    );
    assert_eq!(
        outcomes[0].action,
        BulkAction::Downloaded {
            resumed_from: resumed as u64
        }
    );
    assert_eq!(
        fs::read(cache.path(&outcomes[0].record.file.file)).unwrap(),
        remote.file("/files/oracle-cards-1.jsonl.gz")
    );
    assert!(log.text().contains("retrying"), "{}", log.text());
}

#[tokio::test(start_paused = true)]
async fn requests_are_a_gap_apart_on_the_server_clock() {
    let (cards, rulings) = (gzip_file_of(30_000, 15), gzip_file_of(10_000, 16));
    let (server, _) = scryfall(1, &cards, &rulings, standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let gap = Duration::from_millis(100);
    let http = client(&progress, gap, 3, FAST);

    fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    let times: Vec<_> = server.log().iter().map(|request| request.at).collect();
    assert_eq!(times.len(), 3);
    for pair in times.windows(2) {
        assert!(
            pair[1] - pair[0] >= gap,
            "requests came {:?} apart",
            pair[1] - pair[0]
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_server_error_is_retried_with_growing_waits_and_then_reported() {
    let (cards, rulings) = (gzip_file_of(30_000, 17), gzip_file_of(10_000, 18));
    let (server, _) = scryfall(1, &cards, &rulings, |remote, request| {
        if request.path.starts_with("/files/") {
            Reply::status(500)
        } else {
            remote.answer(request)
        }
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, log) = progress();
    let backoff = Duration::from_secs(2);
    let http = client(&progress, Duration::from_millis(100), 3, backoff);

    let error = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            CliError::Fetch(FetchError::Status { status: 500, .. })
        ),
        "{error}"
    );
    assert!(error.to_string().contains("500"));
    let tries = server.hits("/files/");
    assert_eq!(tries.len(), 3);
    assert!(tries[1].at - tries[0].at >= backoff);
    assert!(tries[2].at - tries[1].at >= backoff * 2);
    assert_eq!(cache.manifest().unwrap().scryfall.len(), 0);
    assert!(!cache.path("scryfall/oracle-cards-1.jsonl.gz").exists());
    assert!(!partial(&cache, "oracle-cards-1.jsonl.gz").exists());
    assert_eq!(log.text().matches("retrying").count(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_retry_after_header_sets_the_wait() {
    let (cards, rulings) = (gzip_file_of(30_000, 19), gzip_file_of(10_000, 20));
    let throttled = Arc::new(AtomicUsize::new(0));
    let counter = throttled.clone();
    let (server, _) = scryfall(1, &cards, &rulings, move |remote, request| {
        if request.path.contains("oracle") && counter.fetch_add(1, Ordering::SeqCst) == 0 {
            return Reply::status(429).header("Retry-After", "7");
        }
        remote.answer(request)
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(
        &progress,
        Duration::from_millis(100),
        3,
        Duration::from_secs(1),
    );

    fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    let tries = server.hits("/files/oracle");
    assert_eq!(tries.len(), 2);
    assert!(tries[1].at - tries[0].at >= Duration::from_secs(7));
}

#[rstest]
#[case::not_found(404, 1)]
#[case::forbidden(403, 1)]
#[case::overloaded(503, 3)]
#[tokio::test]
async fn only_failures_that_can_pass_are_retried(#[case] status: u16, #[case] tries: usize) {
    let (cards, rulings) = (gzip_file_of(10_000, 21), gzip_file_of(10_000, 22));
    let (server, _) = scryfall(1, &cards, &rulings, move |remote, request| {
        if request.path.starts_with("/files/") {
            Reply::status(status)
        } else {
            remote.answer(request)
        }
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall(
        &http,
        &Cache::new(dir.path()),
        &server.url("/bulk-data"),
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert_eq!(server.hits("/files/").len(), tries, "{error}");
    assert!(matches!(error, CliError::Fetch(_)), "{error}");
}

#[tokio::test]
async fn a_server_that_is_not_there_ends_in_a_transport_error_after_the_retries() {
    let port_holder = server::bind().await;
    let url = format!("{}/bulk-data", port_holder.base());
    drop(port_holder);
    let dir = tempfile::tempdir().unwrap();
    let (progress, log) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall(
        &http,
        &Cache::new(dir.path()),
        &url.parse().unwrap(),
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, CliError::Fetch(FetchError::Transport { .. })),
        "{error}"
    );
    assert_eq!(log.text().matches("retrying").count(), 2);
}

#[tokio::test]
async fn a_download_that_always_breaks_keeps_its_partial_file_for_the_next_run() {
    let (cards, rulings) = (gzip_file_of(100_000, 23), gzip_file_of(10_000, 24));
    let (server, _) = scryfall(1, &cards, &rulings, |remote, request| {
        if request.path.contains("oracle") {
            // Whatever is asked, 10 kB arrive and the connection drops.
            return Reply::ok(remote.file(&request.path)).cut_after(10_000);
        }
        remote.answer(request)
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 2, FAST);

    let error = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap_err();

    assert!(
        matches!(error, CliError::Fetch(FetchError::Transport { .. })),
        "{error}"
    );
    let kept = fs::metadata(partial(&cache, "oracle-cards-1.jsonl.gz"))
        .unwrap()
        .len();
    assert_eq!(kept, 10_000);
    assert_eq!(cache.manifest().unwrap().scryfall.len(), 0);
    assert!(!cache.path("scryfall/oracle-cards-1.jsonl.gz").exists());
}

#[tokio::test]
async fn a_corrupt_gzip_file_is_rejected_and_deleted() {
    let mut cards = gzip_file_of(80_000, 25);
    let middle = cards.len() / 2;
    cards[middle] ^= 0xff;
    let (server, _) = scryfall(1, &cards, &gzip_file_of(5_000, 26), standard).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap_err();

    assert!(
        matches!(error, CliError::Fetch(FetchError::Corrupt { .. })),
        "{error}"
    );
    assert!(!partial(&cache, "oracle-cards-1.jsonl.gz").exists());
    assert!(!cache.path("scryfall/oracle-cards-1.jsonl.gz").exists());
    assert_eq!(cache.manifest().unwrap().scryfall.len(), 0);
}

#[tokio::test]
async fn a_file_of_the_wrong_length_is_downloaded_again_and_then_refused() {
    let (cards, rulings) = (gzip_file_of(40_000, 27), gzip_file_of(5_000, 28));
    let (server, remote) = scryfall(1, &cards, &rulings, standard).await;
    {
        // The listing announces 10 bytes more than the server has.
        let mut state = lock(&remote.0);
        let bigger = state.listing.replace(
            &format!("{}", cards.len()),
            &format!("{}", cards.len() + 10),
        );
        state.listing = bigger;
    }
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("were announced"), "{error}");
    assert_eq!(server.hits("/files/oracle").len(), 3);
    assert!(!partial(&cache, "oracle-cards-1.jsonl.gz").exists());
}

#[rstest]
#[case::no_rulings_entry(r#"{"data":[{"type":"oracle_cards","updated_at":"u","jsonl_download_uri":"http://h/files/a.gz"}]}"#, "rulings")]
#[case::not_json("this is not json", "bulk-data listing")]
#[case::no_address(
    r#"{"data":[{"type":"oracle_cards","updated_at":"u"},{"type":"rulings","updated_at":"u"}]}"#,
    "no download address"
)]
#[case::path_traversal(r#"{"data":[{"type":"oracle_cards","updated_at":"u","jsonl_download_uri":"http://h/files/.."},{"type":"rulings","updated_at":"u","jsonl_download_uri":"http://h/r.gz"}]}"#, "names no file")]
#[tokio::test]
async fn a_listing_that_cannot_be_used_is_refused_before_any_download(
    #[case] listing: &'static str,
    #[case] mentions: &str,
) {
    let server = server::start(move |request| {
        if request.path == "/bulk-data" {
            Reply::ok(listing)
        } else {
            Reply::status(500)
        }
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let error = fetch_scryfall(
        &http,
        &Cache::new(dir.path()),
        &server.url("/bulk-data"),
        false,
        &progress,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, CliError::Fetch(FetchError::Invalid { .. })),
        "{error}"
    );
    assert!(error.to_string().contains(mentions), "{error}");
    assert_eq!(server.log().len(), 1, "only the listing was requested");
}

#[tokio::test]
async fn the_older_listing_shape_with_a_plain_json_download_is_understood() {
    let array = br#"[{"id":"p","oracle_id":"o","name":"N","cmc":0}]"#.to_vec();
    let bound = server::bind().await;
    let base = bound.base();
    let (file, listed) = (array.clone(), base.clone());
    let server = bound.serve(move |request| match request.path.as_str() {
        "/bulk-data" => Reply::ok(
            json!({"data": [
                {"type": "oracle_cards", "updated_at": "2026-01-01T00:00:00+00:00", "download_uri": format!("{listed}/files/oracle-cards-1.json")},
                {"type": "rulings", "updated_at": "2026-01-01T00:00:00+00:00", "download_uri": format!("{listed}/files/rulings-1.json")},
            ]})
            .to_string(),
        ),
        _ => Reply::ok(file.clone()),
    });
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let (progress, _) = progress();
    let http = client(&progress, FAST, 3, FAST);

    let outcomes = fetch_scryfall(&http, &cache, &server.url("/bulk-data"), false, &progress)
        .await
        .unwrap();

    assert_eq!(
        fs::read(cache.path("scryfall/oracle-cards-1.json")).unwrap(),
        array
    );
    assert_eq!(outcomes[0].record.file.sha256, sha256_hex(&array));
}
