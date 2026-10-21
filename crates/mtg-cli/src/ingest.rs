//! `ingest`: reads the cached files through `mtg-import` and loads them into Postgres, once per input.

use std::fmt::{Display, Write as _};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mtg_db::import::{
    CardImportReport, CheckReport, PreconImportReport, PrintingImportReport, RulingImportReport,
    SkipLog, check_cards, check_precons, check_printings, check_rulings,
};
use mtg_db::{Database, ImportRuns, ImportSource};
use mtg_import::mtgjson::{MtgjsonError, PreconDeck, parse_deck};
use mtg_import::scryfall::ScryfallError;

use crate::bulk::{Counted, ReadAhead, open_cards, open_rulings};
use crate::cache::{Cache, Manifest, PRINTINGS_TYPE};
use crate::error::CliError;
use crate::files::{sha256_file, sha256_hex, short};
use crate::progress::Progress;

/// How many records the reader thread may run ahead of the database.
const READ_AHEAD: usize = 1024;

/// A progress line after this many cards or rulings.
const RECORDS_PER_LINE: u64 = 10_000;

/// A progress line after this many decks.
const DECKS_PER_LINE: u64 = 25;

/// Skipped records described in detail on the terminal; the rest are only counted.
const WARNINGS_SHOWN: usize = 5;

/// How an ingest behaves.
#[derive(Debug, Clone, Copy)]
pub struct IngestOptions {
    /// Import inputs that were already imported.
    pub force: bool,
    /// Records per database transaction.
    pub batch_size: usize,
}

/// What one input came to.
#[derive(Debug)]
pub enum Step<R> {
    /// Nothing of this kind is cached.
    Missing,
    /// The same input was imported before, against the same cards.
    Unchanged,
    /// A dry run read the input and counted what an import would do.
    Checked(CheckReport),
    /// The input was imported.
    Imported(R),
}

/// What an ingest did with each kind of input.
#[derive(Debug)]
pub struct IngestReport {
    /// The Oracle Cards file.
    pub cards: Step<CardImportReport>,
    /// The Default Cards file, every printing.
    pub printings: Step<PrintingImportReport>,
    /// The rulings file.
    pub rulings: Step<RulingImportReport>,
    /// The MTGJSON deck files.
    pub decks: Step<PreconImportReport>,
}

impl IngestReport {
    /// The one-line summary printed when the command ends.
    pub fn summary(&self, elapsed: Duration) -> String {
        let cards = match &self.cards {
            Step::Missing => "cards not cached".to_owned(),
            Step::Unchanged => "cards unchanged".to_owned(),
            Step::Checked(check) => format!(
                "cards {} usable, {} skipped",
                check.usable, check.skipped.count
            ),
            Step::Imported(r) => format!(
                "cards +{} ~{} ={}, printings +{}, skipped {}",
                r.cards.inserted,
                r.cards.updated,
                r.cards.unchanged,
                r.printings.inserted,
                r.skipped.count
            ),
        };
        let printings = match &self.printings {
            Step::Missing => "printings not cached".to_owned(),
            Step::Unchanged => "printings unchanged".to_owned(),
            Step::Checked(check) => format!(
                "printings {} usable, {} skipped",
                check.usable, check.skipped.count
            ),
            Step::Imported(r) => format!(
                "printings +{} ~{} ={}, orphaned {}, skipped {}",
                r.printings.inserted,
                r.printings.updated,
                r.printings.unchanged,
                r.orphaned,
                r.skipped.count
            ),
        };
        let rulings = match &self.rulings {
            Step::Missing => "rulings not cached".to_owned(),
            Step::Unchanged => "rulings unchanged".to_owned(),
            Step::Checked(check) => format!(
                "rulings {} usable, {} skipped",
                check.usable, check.skipped.count
            ),
            Step::Imported(r) => format!(
                "rulings +{} ={}, orphaned {}, skipped {}",
                r.inserted, r.unchanged, r.orphaned, r.skipped.count
            ),
        };
        let decks = match &self.decks {
            Step::Missing => "decks not cached".to_owned(),
            Step::Unchanged => "decks unchanged".to_owned(),
            Step::Checked(check) => format!(
                "decks {} usable, {} skipped",
                check.usable, check.skipped.count
            ),
            Step::Imported(r) => format!(
                "decks {} stored, {} unresolved entries, skipped {}",
                r.decks.len(),
                r.decks.iter().map(|d| d.unresolved.len()).sum::<usize>(),
                r.skipped.count
            ),
        };
        let dry =
            if matches!(self.cards, Step::Checked(_)) || matches!(self.decks, Step::Checked(_)) {
                "dry run, nothing written: "
            } else {
                ""
            };
        format!(
            "ingest: {dry}{cards}; {printings}; {rulings}; {decks} ({:.1} s)",
            elapsed.as_secs_f64()
        )
    }
}

/// A cached file and its checksum.
struct Input {
    path: PathBuf,
    sha256: String,
}

/// A cached deck file and its checksum.
struct DeckInput {
    name: String,
    input: Input,
}

/// The cached files an ingest reads.
struct Inputs {
    cards: Option<Input>,
    printings: Option<Input>,
    rulings: Option<Input>,
    decks: Vec<DeckInput>,
}

impl Inputs {
    fn nothing(&self) -> bool {
        self.cards.is_none()
            && self.printings.is_none()
            && self.rulings.is_none()
            && self.decks.is_empty()
    }

    /// One checksum for the whole set of deck files, in the order of their names.
    fn deck_checksum(&self) -> String {
        let listing = self.decks.iter().fold(String::new(), |mut text, deck| {
            let _ = writeln!(text, "{}:{}", deck.name, deck.input.sha256);
            text
        });
        sha256_hex(listing.as_bytes())
    }
}

/// Finds the complete files the manifest lists and takes their checksums from the bytes on disk.
async fn gather(cache: &Cache, manifest: &Manifest) -> Result<Inputs, CliError> {
    let mut wanted: Vec<(String, PathBuf)> = Vec::new();
    for kind in ["oracle_cards", PRINTINGS_TYPE, "rulings"] {
        if let Some(record) = manifest.scryfall.get(kind).filter(|r| cache.holds(&r.file)) {
            wanted.push((kind.to_owned(), cache.path(&record.file.file)));
        }
    }
    let scryfall_count = wanted.len();
    for (name, record) in &manifest.mtgjson.decks {
        if cache.holds(&record.file) {
            wanted.push((name.clone(), cache.path(&record.file.file)));
        }
    }
    let sums = tokio::task::spawn_blocking(move || {
        wanted
            .into_iter()
            .map(|(name, path)| {
                let sum = sha256_file(&path).map(|(sum, _)| sum);
                (name, path, sum)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| CliError::Config(format!("the checksum task failed: {e}")))?;

    let mut inputs = Inputs {
        cards: None,
        printings: None,
        rulings: None,
        decks: Vec::new(),
    };
    for (position, (name, path, sum)) in sums.into_iter().enumerate() {
        let sha256 = sum.map_err(|e| CliError::io(&path, e))?;
        let input = Input { path, sha256 };
        if position < scryfall_count {
            match name.as_str() {
                "oracle_cards" => inputs.cards = Some(input),
                PRINTINGS_TYPE => inputs.printings = Some(input),
                _ => inputs.rulings = Some(input),
            }
        } else {
            inputs.decks.push(DeckInput { name, input });
        }
    }
    Ok(inputs)
}

fn nothing_cached(cache: &Cache) -> CliError {
    CliError::Cache {
        path: cache.root().to_path_buf(),
        reason: "nothing is cached; run `mtg-cli fetch scryfall` and `mtg-cli fetch decks` first"
            .into(),
    }
}

/// Imports what the cache holds into the database, skipping what was imported before.
///
/// An input is skipped when the latest successful run of its source read the same bytes. Rulings
/// and decks refer to cards, so their identity includes the cards the database held: new cards
/// bring them back in.
pub async fn ingest(
    cache: &Cache,
    db: &Database,
    options: &IngestOptions,
    progress: &Progress,
) -> Result<IngestReport, CliError> {
    let manifest = cache.manifest()?;
    let inputs = gather(cache, &manifest).await?;
    if inputs.nothing() {
        return Err(nothing_cached(cache));
    }
    db.migrate().await?;
    let loader = Loader {
        db,
        runs: db.import_runs(),
        options,
        progress,
    };
    let cards = loader.cards(inputs.cards.as_ref()).await?;
    // Read after the cards went in, so that printings, rulings and decks are tied to the cards just loaded.
    let basis = cards_basis(&loader.runs).await?;
    let printings = loader.printings(inputs.printings.as_ref(), &basis).await?;
    let rulings = loader.rulings(inputs.rulings.as_ref(), &basis).await?;
    // Decks name printings, so they are tied to the printings as well.
    let deck_basis = decks_basis(&loader.runs, &basis).await?;
    let decks = loader.decks(&inputs, &deck_basis).await?;
    Ok(IngestReport {
        cards,
        printings,
        rulings,
        decks,
    })
}

/// What one ingest needs to run its three imports.
struct Loader<'a> {
    db: &'a Database,
    runs: ImportRuns,
    options: &'a IngestOptions,
    progress: &'a Progress,
}

impl Loader<'_> {
    fn importer(&self) -> mtg_db::Importer {
        self.db.importer().with_batch_size(self.options.batch_size)
    }

    async fn cards(&self, input: Option<&Input>) -> Result<Step<CardImportReport>, CliError> {
        let Some(input) = input else {
            return Ok(missing(self.progress, "cards", "fetch scryfall"));
        };
        let key = input.sha256.clone();
        if done(&self.runs, ImportSource::ScryfallCards, &key, self.options).await? {
            return Ok(unchanged(self.progress, "cards", &key));
        }
        let records = Counted::new(
            open_cards(&input.path)?,
            "cards",
            RECORDS_PER_LINE,
            self.progress.clone(),
        );
        let started = Instant::now();
        let report = self
            .importer()
            .with_input(key)
            .import_cards(ReadAhead::spawn(records, READ_AHEAD)?)
            .await?;
        self.progress.line(format_args!(
            "cards: {} new, {} changed, {} unchanged; printings {} new; {} skipped (run {}, {:.1} s)",
            report.cards.inserted,
            report.cards.updated,
            report.cards.unchanged,
            report.printings.inserted,
            report.skipped.count,
            report.run_id,
            started.elapsed().as_secs_f64()
        ));
        warn_skipped(self.progress, "cards", &report.skipped);
        Ok(Step::Imported(report))
    }

    async fn printings(
        &self,
        input: Option<&Input>,
        basis: &str,
    ) -> Result<Step<PrintingImportReport>, CliError> {
        let Some(input) = input else {
            return Ok(missing(
                self.progress,
                "printings",
                "fetch scryfall --printings",
            ));
        };
        let key = format!("{}@{basis}", input.sha256);
        if done(
            &self.runs,
            ImportSource::ScryfallPrintings,
            &key,
            self.options,
        )
        .await?
        {
            return Ok(unchanged(self.progress, "printings", &input.sha256));
        }
        let records = Counted::new(
            open_cards(&input.path)?,
            "printings",
            RECORDS_PER_LINE,
            self.progress.clone(),
        );
        let started = Instant::now();
        let report = self
            .importer()
            .with_input(key)
            .import_printings(ReadAhead::spawn(records, READ_AHEAD)?)
            .await?;
        self.progress.line(format_args!(
            "printings: {} new, {} changed, {} unchanged, {} without a card; {} skipped (run {}, {:.1} s)",
            report.printings.inserted,
            report.printings.updated,
            report.printings.unchanged,
            report.orphaned,
            report.skipped.count,
            report.run_id,
            started.elapsed().as_secs_f64()
        ));
        warn_skipped(self.progress, "printings", &report.skipped);
        Ok(Step::Imported(report))
    }

    async fn rulings(
        &self,
        input: Option<&Input>,
        basis: &str,
    ) -> Result<Step<RulingImportReport>, CliError> {
        let Some(input) = input else {
            return Ok(missing(self.progress, "rulings", "fetch scryfall"));
        };
        let key = format!("{}@{basis}", input.sha256);
        if done(
            &self.runs,
            ImportSource::ScryfallRulings,
            &key,
            self.options,
        )
        .await?
        {
            return Ok(unchanged(self.progress, "rulings", &input.sha256));
        }
        let records = Counted::new(
            open_rulings(&input.path)?,
            "rulings",
            RECORDS_PER_LINE,
            self.progress.clone(),
        );
        let started = Instant::now();
        let report = self
            .importer()
            .with_input(key)
            .import_rulings(ReadAhead::spawn(records, READ_AHEAD)?)
            .await?;
        self.progress.line(format_args!(
            "rulings: {} new, {} unchanged, {} without a card; {} skipped (run {}, {:.1} s)",
            report.inserted,
            report.unchanged,
            report.orphaned,
            report.skipped.count,
            report.run_id,
            started.elapsed().as_secs_f64()
        ));
        warn_skipped(self.progress, "rulings", &report.skipped);
        Ok(Step::Imported(report))
    }

    async fn decks(
        &self,
        inputs: &Inputs,
        basis: &str,
    ) -> Result<Step<PreconImportReport>, CliError> {
        if inputs.decks.is_empty() {
            return Ok(missing(self.progress, "decks", "fetch decks"));
        }
        let checksum = inputs.deck_checksum();
        let key = format!("{checksum}@{basis}");
        if done(&self.runs, ImportSource::MtgjsonDecks, &key, self.options).await? {
            return Ok(unchanged(self.progress, "decks", &checksum));
        }
        let started = Instant::now();
        let decks = deck_stream(&inputs.decks, self.progress);
        let report = self
            .importer()
            .with_input(key)
            .import_precons(ReadAhead::spawn(decks, READ_AHEAD)?)
            .await?;
        let entries: usize = report.decks.iter().map(|d| d.entries).sum();
        let unresolved: usize = report.decks.iter().map(|d| d.unresolved.len()).sum();
        self.progress.line(format_args!(
            "decks: {} stored with {entries} lines, {unresolved} lines matched no card; {} skipped (run {}, {:.1} s)",
            report.decks.len(),
            report.skipped.count,
            report.run_id,
            started.elapsed().as_secs_f64()
        ));
        warn_unresolved(self.progress, &report);
        warn_skipped(self.progress, "decks", &report.skipped);
        Ok(Step::Imported(report))
    }
}

/// Reads the cached files and counts what an import would store and skip, touching no database.
pub fn dry_run(cache: &Cache, progress: &Progress) -> Result<IngestReport, CliError> {
    let manifest = cache.manifest()?;
    let held = |kind: &str| {
        manifest
            .scryfall
            .get(kind)
            .filter(|record| cache.holds(&record.file))
            .map(|record| cache.path(&record.file.file))
    };
    let decks: Vec<DeckInput> = manifest
        .mtgjson
        .decks
        .iter()
        .filter(|(_, record)| cache.holds(&record.file))
        .map(|(name, record)| DeckInput {
            name: name.clone(),
            input: Input {
                path: cache.path(&record.file.file),
                sha256: String::new(),
            },
        })
        .collect();
    let (cards_path, printings_path, rulings_path) =
        (held("oracle_cards"), held(PRINTINGS_TYPE), held("rulings"));
    if cards_path.is_none()
        && printings_path.is_none()
        && rulings_path.is_none()
        && decks.is_empty()
    {
        return Err(nothing_cached(cache));
    }

    let cards = checked(progress, "cards", "fetch scryfall", cards_path, |path| {
        Ok(check_cards(Counted::new(
            open_cards(path)?,
            "cards",
            RECORDS_PER_LINE,
            progress.clone(),
        )))
    })?;
    let printings = checked(
        progress,
        "printings",
        "fetch scryfall --printings",
        printings_path,
        |path| {
            Ok(check_printings(Counted::new(
                open_cards(path)?,
                "printings",
                RECORDS_PER_LINE,
                progress.clone(),
            )))
        },
    )?;
    let rulings = checked(
        progress,
        "rulings",
        "fetch scryfall",
        rulings_path,
        |path| {
            Ok(check_rulings(Counted::new(
                open_rulings(path)?,
                "rulings",
                RECORDS_PER_LINE,
                progress.clone(),
            )))
        },
    )?;
    let decks = if decks.is_empty() {
        missing(progress, "decks", "fetch decks")
    } else {
        let check = check_precons(deck_stream(&decks, progress));
        progress.line(format_args!(
            "decks: {} usable, {} would be skipped",
            check.usable, check.skipped.count
        ));
        warn_skipped(progress, "decks", &check.skipped);
        Step::Checked(check)
    };
    Ok(IngestReport {
        cards,
        printings,
        rulings,
        decks,
    })
}

/// Counts what an import of the cached file at `path` would do, or notes that none is cached.
fn checked<R>(
    progress: &Progress,
    label: &str,
    command: &str,
    path: Option<PathBuf>,
    check: impl FnOnce(&Path) -> Result<Result<CheckReport, ScryfallError>, CliError>,
) -> Result<Step<R>, CliError> {
    let Some(path) = path else {
        return Ok(missing(progress, label, command));
    };
    let check = check(&path)?.map_err(|source| CliError::Bulk {
        what: path.display().to_string(),
        source,
    })?;
    progress.line(format_args!(
        "{label}: {} usable, {} would be skipped",
        check.usable, check.skipped.count
    ));
    warn_skipped(progress, label, &check.skipped);
    Ok(Step::Checked(check))
}

fn missing<R>(progress: &Progress, label: &str, command: &str) -> Step<R> {
    progress.line(format_args!("{label}: not cached, run `mtg-cli {command}`"));
    Step::Missing
}

fn unchanged<R>(progress: &Progress, label: &str, checksum: &str) -> Step<R> {
    progress.line(format_args!(
        "{label}: unchanged since the last import (sha256 {})",
        short(checksum)
    ));
    Step::Unchanged
}

/// Whether the latest successful run of `source` read exactly `key`.
async fn done(
    runs: &ImportRuns,
    source: ImportSource,
    key: &str,
    options: &IngestOptions,
) -> Result<bool, CliError> {
    if options.force {
        return Ok(false);
    }
    let latest = runs.latest_succeeded(source).await?;
    Ok(latest.and_then(|run| run.input).as_deref() == Some(key))
}

/// What the database knows of the cards: the input of the latest successful card import.
async fn cards_basis(runs: &ImportRuns) -> Result<String, CliError> {
    let latest = runs.latest_succeeded(ImportSource::ScryfallCards).await?;
    Ok(latest
        .and_then(|run| run.input)
        .unwrap_or_else(|| "no-cards".to_owned()))
}

/// What the decks are tied to: the cards, and the printings when an all-printings file was imported.
async fn decks_basis(runs: &ImportRuns, cards: &str) -> Result<String, CliError> {
    let latest = runs
        .latest_succeeded(ImportSource::ScryfallPrintings)
        .await?;
    Ok(match latest.and_then(|run| run.input) {
        Some(printings) => format!("{cards}+{printings}"),
        None => cards.to_owned(),
    })
}

/// The deck files in name order, parsed one by one; a bad file is an error item, not the end.
fn deck_stream(
    decks: &[DeckInput],
    progress: &Progress,
) -> impl Iterator<Item = Result<PreconDeck, MtgjsonError>> + Send + use<> {
    let files: Vec<(String, PathBuf)> = decks
        .iter()
        .map(|deck| (deck.name.clone(), deck.input.path.clone()))
        .collect();
    let parsed = files
        .into_iter()
        .map(|(name, path)| read_deck(&name, &path));
    Counted::new(parsed, "decks", DECKS_PER_LINE, progress.clone())
}

fn read_deck(name: &str, path: &Path) -> Result<PreconDeck, MtgjsonError> {
    let file = File::open(path).map_err(|e| unreadable(name, e))?;
    parse_deck(BufReader::new(file)).map_err(|e| unreadable(name, e))
}

/// A deck error that names the deck, since the parser's own messages do not.
fn unreadable(name: &str, error: impl Display) -> MtgjsonError {
    MtgjsonError::Json(<serde_json::Error as serde::de::Error>::custom(
        format_args!("{name}: {error}"),
    ))
}

fn warn_skipped(progress: &Progress, label: &str, skipped: &SkipLog) {
    for sample in skipped.samples.iter().take(WARNINGS_SHOWN) {
        progress.line(format_args!(
            "warning: {label}: record {} skipped: {}",
            sample.position, sample.reason
        ));
    }
    let silent = usize::try_from(skipped.count)
        .unwrap_or(usize::MAX)
        .saturating_sub(WARNINGS_SHOWN.min(skipped.samples.len()));
    if silent > 0 {
        progress.line(format_args!("warning: {label}: {silent} more skipped"));
    }
}

fn warn_unresolved(progress: &Progress, report: &PreconImportReport) {
    let lines = report
        .decks
        .iter()
        .flat_map(|deck| deck.unresolved.iter().map(move |card| (&deck.name, card)));
    for (deck, card) in lines.take(WARNINGS_SHOWN) {
        progress.line(format_args!(
            "warning: decks: {deck}: {:?} matched no card ({:?})",
            card.name, card.reason
        ));
    }
}
