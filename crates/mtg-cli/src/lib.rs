//! The `mtg-cli` tool: downloads Scryfall and MTGJSON data into a cache and loads it into Postgres.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mtg_db::Database;

pub mod args;
pub mod audit;
pub mod bulk;
pub mod cache;
pub mod check_rules;
pub mod compile;
pub mod coverage;
pub mod coverage_all;
pub mod coverage_deck;
pub mod drafts;
pub mod error;
pub mod fetch;
pub mod files;
pub mod fix_marks;
pub mod forge;
pub mod gaps;
pub mod http;
pub mod ingest;
pub mod pacer;
pub mod popularity;
pub mod progress;
pub mod propose;
pub mod reads;
pub mod status;
pub mod w19b_pairs;
pub mod w2a_promote;

use args::{Cli, Command, Fetch};
use cache::{Cache, PRINTINGS_TYPE, SCRYFALL_TYPES, default_dir};
use error::CliError;
use fetch::Endpoints;
use fetch::decks::{DeckFetchReport, DeckFilter, fetch_decks};
use fetch::scryfall::{BulkAction, BulkOutcome, fetch_scryfall_kinds};
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
            let mut kinds = SCRYFALL_TYPES.to_vec();
            if args.printings {
                kinds.push(PRINTINGS_TYPE);
            }
            let outcomes = fetch_scryfall_kinds(
                &http,
                &cache,
                &endpoints.scryfall_listing,
                &kinds,
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
            let db = connect(cli.database_url).await?;
            let options = IngestOptions {
                force: args.force,
                batch_size: args.batch_size,
            };
            let report = ingest(&cache, &db, &options, &progress).await?;
            Ok(report.summary(started.elapsed()))
        }
        Command::Migrate => migrate_summary(&connect(cli.database_url).await?).await,
        Command::Status => status::status(&cache, cli.database_url.as_deref())
            .await
            .map(|text| text.trim_end().to_owned()),
        Command::Audit(args) => audit_command(cli.database_url, &args).await,
        Command::Compile(args) => {
            let db = connect(cli.database_url).await?;
            Ok(compile::compile(&db, &args).await?.render())
        }
        Command::CheckRules(args) => check_rules_command(cli.database_url, &cache, &args).await,
        Command::FixMarks(args) => {
            fix_marks::fix_marks(&connect(cli.database_url).await?, &args).await
        }
        Command::Reads(args) => reads::reads(&connect(cli.database_url).await?, &args).await,
        Command::ForgeCheck(args) => {
            let db = connect(cli.database_url).await?;
            forge::forge_check(&db, &args).await
        }
        Command::Coverage(args) => {
            let db = connect(cli.database_url).await?;
            coverage::report(&db, &cache, &args).await
        }
        Command::Promote(args) => {
            let db = connect(cli.database_url).await?;
            Ok(w2a_promote::w2a_promote(&db, &cache, &args).await?.render())
        }
        Command::Propose(args) => propose::command(cli.database_url, &cache, &args, progress).await,
        Command::DslReference => Ok(mtg_cards::reference::dsl_reference().trim_end().to_owned()),
        Command::DslSchema => Ok(mtg_cards::reference::w3a_card_schema().to_string()),
        Command::ExportPairs(args) => {
            let db = connect(cli.database_url).await?;
            w19b_pairs::w19b_export_pairs(&db, &args).await
        }
    }
}

/// `audit`: the sample as text, or written to the file `--file` names.
async fn audit_command(
    database_url: Option<String>,
    args: &args::AuditArgs,
) -> Result<String, CliError> {
    let db = connect(database_url).await?;
    let text = audit::audit(&db, args).await?;
    let Some(path) = &args.file else {
        return Ok(text);
    };
    tokio::fs::write(path, format!("{text}\n"))
        .await
        .map_err(|source| CliError::io(path, source))?;
    Ok(format!("audit: written to {}", path.display()))
}

/// `check-rules`: the reports, or with `--gaps` the ledger (the ranks come from the cache, read before the database is).
async fn check_rules_command(
    database_url: Option<String>,
    cache: &Cache,
    args: &args::CheckRulesArgs,
) -> Result<String, CliError> {
    let ranks = match &args.gaps {
        Some(_) => Some(popularity::Popularity::from_cache(cache)?),
        None => None,
    };
    let db = connect(database_url).await?;
    let checked = check_rules::check_rules(&db, args).await?;
    let (Some(path), Some(ranks)) = (&args.gaps, &ranks) else {
        return Ok(checked.render(args.verbose));
    };
    let marks = if args.marks {
        Some(fix_mark_counts(&db).await?)
    } else {
        None
    };
    gaps::write_ledger(&checked, ranks, marks.as_ref(), path).await
}

/// The fix marks players made, by oracle id (`check-rules --gaps --marks`).
async fn fix_mark_counts(db: &Database) -> Result<gaps::Marks, CliError> {
    let ranked = db.fix_marks().ranked(None).await?;
    Ok(ranked
        .into_iter()
        .map(|mark| (mark.oracle_id.to_string(), mark.marks))
        .collect())
}

/// The database the URL names, or the error that asks for one.
async fn connect(url: Option<String>) -> Result<Database, CliError> {
    let url = url.ok_or_else(missing_database_url)?;
    Ok(Database::connect(&url).await?)
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

/// Applies the pending migrations of the card tables; safe to repeat and to run beside a server.
pub async fn migrate_summary(db: &Database) -> Result<String, CliError> {
    db.migrate().await?;
    Ok("migrate: the card tables are current".to_owned())
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
