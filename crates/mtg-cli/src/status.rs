//! `status`: what is cached, what the database holds and how the latest imports went.

use std::fmt::Write as _;
use std::str::FromStr;
use std::time::Duration;

use chrono::Utc;
use mtg_db::{Database, DbError, ImportRun, ImportSource, RunStatus, STALE_AFTER};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::cache::{Cache, FileRecord};
use crate::error::CliError;
use crate::files::{human_size, short};

/// How many import runs the report lists.
const RUNS_SHOWN: u32 = 8;

/// How long `status` waits for the database before reporting it unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// A report of the cache, and of the database when one is given.
pub async fn status(cache: &Cache, database_url: Option<&str>) -> Result<String, CliError> {
    let mut out = cache_section(cache)?;
    match database_url {
        None => out.push_str("database: DATABASE_URL is not set\n"),
        Some(url) => out.push_str(&database_section(url, CONNECT_TIMEOUT).await),
    }
    Ok(out)
}

fn cache_section(cache: &Cache) -> Result<String, CliError> {
    let manifest = cache.manifest()?;
    let mut out = format!("cache: {}\n", cache.root().display());
    let held = |record: &FileRecord| {
        if cache.holds(record) {
            String::new()
        } else {
            "  (file missing or changed)".to_owned()
        }
    };
    for (kind, record) in &manifest.scryfall {
        let _ = writeln!(
            out,
            "  scryfall {kind}: {}  {}  updated {}  fetched {}  sha256 {}{}",
            record.file.file,
            human_size(record.file.size),
            record.updated_at,
            record.file.fetched_at,
            short(&record.file.sha256),
            held(&record.file)
        );
    }
    if let Some(index) = &manifest.mtgjson.index {
        let _ = writeln!(
            out,
            "  mtgjson index: {}  build {}  fetched {}{}",
            index.file.file,
            index.build.as_deref().unwrap_or("unknown"),
            index.file.fetched_at,
            held(&index.file)
        );
    }
    if !manifest.mtgjson.decks.is_empty() {
        let size: u64 = manifest.mtgjson.decks.values().map(|d| d.file.size).sum();
        let gone = manifest
            .mtgjson
            .decks
            .values()
            .filter(|d| !cache.holds(&d.file))
            .count();
        let _ = writeln!(
            out,
            "  mtgjson decks: {} files, {}{}",
            manifest.mtgjson.decks.len(),
            human_size(size),
            if gone > 0 {
                format!("  ({gone} missing or changed)")
            } else {
                String::new()
            }
        );
    }
    if manifest.scryfall.is_empty() && manifest.mtgjson.index.is_none() {
        out.push_str("  nothing cached yet\n");
    }
    Ok(out)
}

/// Where a database URL points, without the user name and password it carries.
fn target(url: &str) -> String {
    PgConnectOptions::from_str(url).map_or_else(
        |_| "an unreadable DATABASE_URL".to_owned(),
        |options| {
            format!(
                "{}:{}/{}",
                options.get_host(),
                options.get_port(),
                options.get_database().unwrap_or("")
            )
        },
    )
}

async fn database_section(url: &str, wait: Duration) -> String {
    let target = target(url);
    let pool = PgPoolOptions::new()
        .acquire_timeout(wait)
        .connect(url)
        .await;
    match pool.map(Database::from_pool) {
        Ok(db) => database_report(&db, &target).await,
        Err(error) => format!("database: {target}: cannot connect: {error}\n"),
    }
}

/// The database part of the report: where it is, its row counts and its latest import runs.
pub async fn database_report(db: &Database, target: &str) -> String {
    let mut out = format!("database: {target}\n");
    match db.row_counts().await {
        Ok(rows) => {
            let _ = writeln!(
                out,
                "  rows: cards {}, printings {}, rulings {}, precon decks {} ({} lines)",
                rows.cards, rows.printings, rows.rulings, rows.precon_decks, rows.precon_entries
            );
        }
        Err(error) if missing_table(&error) => {
            out.push_str("  the schema is not applied yet; `mtg-cli ingest` applies it\n");
            return out;
        }
        Err(error) => {
            let _ = writeln!(out, "  rows: {error}");
            return out;
        }
    }
    match db.import_runs().recent(RUNS_SHOWN).await {
        Ok(runs) if runs.is_empty() => out.push_str("  no import runs yet\n"),
        Ok(runs) => {
            out.push_str("  latest import runs:\n");
            for run in &runs {
                let _ = writeln!(out, "    {}", run_line(run));
            }
        }
        Err(error) => {
            let _ = writeln!(out, "  import runs: {error}");
        }
    }
    out
}

/// Whether the database answered that a table does not exist.
fn missing_table(error: &DbError) -> bool {
    matches!(
        error,
        DbError::Query(sqlx::Error::Database(e)) if e.code().as_deref() == Some("42P01")
    )
}

fn source_name(source: ImportSource) -> &'static str {
    match source {
        ImportSource::ScryfallCards => "scryfall_cards",
        ImportSource::ScryfallRulings => "scryfall_rulings",
        ImportSource::MtgjsonDecks => "mtgjson_decks",
    }
}

fn run_line(run: &ImportRun) -> String {
    let started = run.started_at.format("%Y-%m-%d %H:%M:%S UTC");
    let how = match run.status {
        RunStatus::Succeeded => "succeeded".to_owned(),
        RunStatus::Failed => format!(
            "failed: {}",
            run.error.as_deref().unwrap_or("no reason given")
        ),
        RunStatus::Running => {
            let age = Utc::now() - run.started_at;
            if age.to_std().is_ok_and(|age| age > STALE_AFTER) {
                "running, but for so long that its process is probably gone".to_owned()
            } else {
                "running".to_owned()
            }
        }
    };
    let took = run
        .finished_at
        .map(|end| format!(", {:.1} s", (end - run.started_at).as_seconds_f64()))
        .unwrap_or_default();
    let skipped = run.counts["skipped"]["count"].as_u64().unwrap_or(0);
    let input = run
        .input
        .as_deref()
        .map(|input| format!(", input {}", short(input)))
        .unwrap_or_default();
    format!(
        "#{} {} {started} {how}{took}{}{input}",
        run.id,
        source_name(run.source),
        if skipped > 0 {
            format!(", {skipped} skipped")
        } else {
            String::new()
        }
    )
}

#[cfg(test)]
mod tests {
    //! Goal: the status report describes an empty and a populated cache, names where the database is without its password, and marks runs that look abandoned.
    use chrono::Duration;
    use mtg_db::RunId;
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::cache::{BulkRecord, Manifest};

    fn run(status: RunStatus, age: Duration, counts: serde_json::Value) -> ImportRun {
        let started_at = Utc::now() - age;
        ImportRun {
            id: RunId(7),
            source: ImportSource::ScryfallCards,
            started_at,
            finished_at: (status != RunStatus::Running)
                .then_some(started_at + Duration::seconds(3)),
            status,
            counts,
            error: (status == RunStatus::Failed).then(|| "boom".to_owned()),
            input: Some("abcdef0123456789abcdef".to_owned()),
        }
    }

    #[test]
    fn run_lines_say_how_the_run_ended() {
        let done = run_line(&run(RunStatus::Succeeded, Duration::hours(1), json!({})));
        assert!(done.starts_with("#7 scryfall_cards "), "{done}");
        assert!(
            done.contains("succeeded, 3.0 s, input abcdef012345"),
            "{done}"
        );
        let failed = run_line(&run(RunStatus::Failed, Duration::hours(1), json!({})));
        assert!(failed.contains("failed: boom"), "{failed}");
        let skipped = run_line(&run(
            RunStatus::Succeeded,
            Duration::hours(1),
            json!({"skipped": {"count": 4}}),
        ));
        assert!(skipped.contains("4 skipped"), "{skipped}");
    }

    #[test]
    fn a_run_that_stays_running_for_hours_is_marked_abandoned() {
        let fresh = run_line(&run(RunStatus::Running, Duration::minutes(5), json!({})));
        assert!(
            fresh.contains("running") && !fresh.contains("probably gone"),
            "{fresh}"
        );
        let old = run_line(&run(RunStatus::Running, Duration::hours(5), json!({})));
        assert!(old.contains("probably gone"), "{old}");
    }

    #[test]
    fn an_empty_cache_says_so_and_a_populated_one_lists_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path());
        assert!(
            cache_section(&cache)
                .unwrap()
                .contains("nothing cached yet")
        );

        let mut manifest = Manifest::default();
        manifest.scryfall.insert(
            "oracle_cards".into(),
            BulkRecord {
                file: FileRecord {
                    file: "scryfall/o.gz".into(),
                    url: "http://x/o.gz".into(),
                    size: 4,
                    sha256: "ab".repeat(32),
                    fetched_at: "2026-10-19T08:00:00Z".into(),
                },
                updated_at: "2026-10-18T21:02:00.694+00:00".into(),
            },
        );
        cache.save(&manifest).unwrap();
        let missing = cache_section(&cache).unwrap();
        assert!(
            missing.contains("scryfall oracle_cards: scryfall/o.gz"),
            "{missing}"
        );
        assert!(missing.contains("file missing or changed"), "{missing}");

        cache.prepare("scryfall").unwrap();
        std::fs::write(cache.path("scryfall/o.gz"), b"data").unwrap();
        assert!(!cache_section(&cache).unwrap().contains("missing"));
    }

    #[rstest]
    #[case(
        "postgres://user:hunter2@db.example:5433/magic",
        "db.example:5433/magic"
    )]
    #[case(
        "postgres://magic:magic@localhost/magic?sslmode=disable",
        "localhost:5432/magic"
    )]
    #[case("postgres://localhost:5432", "localhost:5432/")]
    #[case("not a url", "an unreadable DATABASE_URL")]
    fn the_database_is_named_without_its_password(#[case] url: &str, #[case] shown: &str) {
        assert_eq!(target(url), shown);
    }

    #[tokio::test]
    async fn an_unreachable_database_is_reported_with_its_address_only() {
        let text = database_section(
            "postgres://user:hunter2@127.0.0.1:1/magic",
            std::time::Duration::from_millis(200),
        )
        .await;
        assert!(
            text.starts_with("database: 127.0.0.1:1/magic: cannot connect"),
            "{text}"
        );
        assert!(!text.contains("hunter2"), "{text}");
    }
}
