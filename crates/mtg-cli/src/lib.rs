//! The `mtg-cli` tool: downloads Scryfall and MTGJSON data into a cache and loads it into Postgres.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mtg_db::Database;

pub mod args;
pub mod audit;
pub mod bulk;
pub mod cache;
pub mod coverage;
pub mod error;
pub mod fetch;
pub mod files;
pub mod http;
pub mod ingest;
pub mod pacer;
pub mod progress;
pub mod status;

use args::{Cli, Command, Fetch};
use cache::{Cache, default_dir};
use error::CliError;
use fetch::Endpoints;
use fetch::decks::{DeckFetchReport, DeckFilter, fetch_decks};
use fetch::scryfall::{BulkAction, BulkOutcome, fetch_scryfall};
use http::{Http, HttpConfig};
use ingest::{IngestOptions, dry_run, ingest};
use progress::Progress;

/// Runs a parsed command line and returns the one-line summary (or report) to print.
pub async fn run(cli: Cli, progress: Progress) -> Result<String, CliError> {
    let cache = Cache::new(cache_dir(cli.cache)?);
    let started = Instant::now();
    match cli.command {
        Command::Fetch {
            what: Fetch::Scryfall(args),
        } => {
            let http = Http::new(&HttpConfig::default(), progress.clone())?;
            let endpoints = Endpoints::real()?;
            let outcomes = fetch_scryfall(
                &http,
                &cache,
                &endpoints.scryfall_listing,
                args.force,
                &progress,
            )
            .await?;
            Ok(scryfall_summary(&outcomes, started.elapsed()))
        }
        Command::Fetch {
            what: Fetch::Decks(args),
        } => {
            let http = Http::new(&HttpConfig::default(), progress.clone())?;
            let filter = DeckFilter {
                types: args.deck_type,
                only: args.only,
                limit: args.limit,
            };
            let report = fetch_decks(
                &http,
                &cache,
                &Endpoints::real()?,
                &filter,
                args.force,
                &progress,
            )
            .await?;
            Ok(decks_summary(&report, started.elapsed()))
        }
        Command::Ingest(args) if args.dry_run => {
            Ok(dry_run(&cache, &progress)?.summary(started.elapsed()))
        }
        Command::Ingest(args) => {
            let url = cli.database_url.ok_or_else(missing_database_url)?;
            let db = Database::connect(&url).await?;
            let options = IngestOptions {
                force: args.force,
                batch_size: args.batch_size,
            };
            let report = ingest(&cache, &db, &options, &progress).await?;
            Ok(report.summary(started.elapsed()))
        }
        Command::Status => status::status(&cache, cli.database_url.as_deref())
            .await
            .map(|text| text.trim_end().to_owned()),
        Command::Audit(args) => {
            let url = cli.database_url.ok_or_else(missing_database_url)?;
            let db = Database::connect(&url).await?;
            let text = audit::audit(&db, &args).await?;
            let Some(path) = &args.file else {
                return Ok(text);
            };
            tokio::fs::write(path, format!("{text}\n"))
                .await
                .map_err(|source| CliError::io(path, source))?;
            Ok(format!("audit: written to {}", path.display()))
        }
        Command::Coverage(args) => {
            let url = cli.database_url.ok_or_else(missing_database_url)?;
            let db = Database::connect(&url).await?;
            coverage::coverage(&db, &args).await
        }
    }
}

fn missing_database_url() -> CliError {
    CliError::Config("no database: set DATABASE_URL or pass --database-url".into())
}

/// The cache directory: the flag or `MTG_CACHE_DIR` (clap reads both), else the user's cache directory.
fn cache_dir(flag: Option<PathBuf>) -> Result<PathBuf, CliError> {
    flag.or_else(|| default_dir(|name| std::env::var_os(name)))
        .ok_or_else(|| {
            CliError::Config(
                "no cache directory: pass --cache <dir> or set MTG_CACHE_DIR (HOME is not set)"
                    .into(),
            )
        })
}

/// The one-line summary of `fetch scryfall`.
pub fn scryfall_summary(outcomes: &[BulkOutcome], elapsed: Duration) -> String {
    let parts: Vec<String> = outcomes
        .iter()
        .map(|outcome| match outcome.action {
            BulkAction::UpToDate => format!("{} up to date", outcome.kind),
            BulkAction::Downloaded { .. } => format!(
                "{} downloaded ({})",
                outcome.kind,
                files::human_size(outcome.record.file.size)
            ),
        })
        .collect();
    format!(
        "fetch scryfall: {} ({:.1} s)",
        parts.join(", "),
        elapsed.as_secs_f64()
    )
}

/// The one-line summary of `fetch decks`.
pub fn decks_summary(report: &DeckFetchReport, elapsed: Duration) -> String {
    format!(
        "fetch decks: {} selected, {} downloaded, {} unchanged, {} current, {} missing ({:.1} s)",
        report.selected,
        report.downloaded,
        report.not_modified,
        report.current,
        report.missing.len(),
        elapsed.as_secs_f64()
    )
}
