//! `propose --lines`: for a card whose Forge translation has one or two gapped lines, a model writes only those abilities
//! from a small prompt (the line, its Forge script lines, the proven abilities and the nearest solved examples); the parts
//! are merged into the translation and the checker judges the whole card. A local server or the paid API, one request at
//! a time or through batches (`docs/cards-dsl.md`, Line fill). Only the checker's `Verified` can play without a read.

pub mod batch;
pub mod card;
pub mod client;
pub mod paid;
pub mod part;
pub mod pool;
pub mod price;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use mtg_cards::Caps;
use mtg_cards::PrintedCard;
use mtg_cards::load::Library;

use self::card::{Prepared, Step, Work};
use self::paid::{Meter, Sent};
use self::part::Kind;
use self::pool::{Pool, Shape, Translation, describe};
use super::budget::Stop;
use super::forge_hint::ForgeHints;
use super::model::ModelClient;
use super::order::digital_only;
use super::outcome::Outcome;
use super::select;
use super::store::Store;
use crate::args::{ProposeArgs, Provider};
use crate::error::CliError;
use crate::progress::Progress;

/// The file of a run that holds one line for each card: name, result, requests, seconds and a note.
pub const RESULTS: &str = "lines.tsv";

/// Cards in a row that get no answer before the run gives up.
const FAILURES_IN_A_ROW: usize = 5;

/// How many solved examples a line's prompt holds.
pub const EXAMPLES: usize = 4;

/// The sentence that closes the report: what may play without a read.
pub const SAFETY: &str = "Only `accepted Verified`, the checker's own pass, can play without a player's read. Every other result is a proposal marked source `sweep` (model-written, unread) in its meta file, and plays only after a player reads and approves it.";

/// Refuses the flag combinations line mode does not allow: a local server needs its address, a batch the paid API.
pub fn check_args(args: &ProposeArgs) -> Result<(), CliError> {
    if args.provider == Provider::Openai && args.base_url.is_none() {
        return Err(CliError::Config(
            "--lines on a local model needs --base-url, such as http://localhost:11434/v1".into(),
        ));
    }
    if args.batch && args.provider != Provider::Anthropic {
        return Err(CliError::Config(
            "--batch is the Message Batches API of Anthropic; it needs --provider anthropic".into(),
        ));
    }
    for (given, flag) in [
        (args.names_file.is_some(), "--names-file"),
        (args.translations.is_some(), "--translations"),
        (args.forge_hint.is_some(), "--forge-hint"),
    ] {
        if !given {
            return Err(CliError::Config(format!("--lines needs {flag}")));
        }
    }
    Ok(())
}

/// The names in a file, one a line, in order; blank lines and `#` comments are left out.
pub fn read_names(path: &Path) -> Result<Vec<String>, CliError> {
    let text = fs::read_to_string(path).map_err(|source| CliError::io(path, source))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

/// The clients of a run, one for each kind of ability (each may constrain the answer to the schema of its kind).
pub trait LineModels: Send + Sync {
    /// The client that asks for an ability of `kind`.
    fn client(&self, kind: Kind) -> Result<Arc<dyn ModelClient>, CliError>;
}

/// What a run reads and checks against.
pub struct LineCtx<'a> {
    /// The stored cards.
    pub cards: &'a [PrintedCard],
    /// The hand-written rules.
    pub library: &'a Library,
    /// The lowercase names a rule may name.
    pub names: &'a BTreeSet<String>,
    /// What the engine runs.
    pub caps: &'a Caps,
    /// The Forge scripts.
    pub forge: &'a ForgeHints,
    /// The folder of the translator's proposals and their meta files.
    pub translations: &'a Path,
    /// The model id.
    pub model: &'a str,
    /// The oracle ids of the Commander-legal cards; none takes every card.
    pub legal: Option<&'a BTreeSet<String>>,
    /// The solved lines the examples come from, read when first needed.
    pub pool: OnceLock<Pool>,
}

/// The settings of a run.
#[derive(Debug, Default)]
pub struct LineOptions {
    /// How many independent answers each line gets in the first round.
    pub samples: usize,
    /// Skip the names the results file already holds.
    pub resume: bool,
    /// The meter: the spend cap and the ledger of a paid run.
    pub meter: Meter,
}

/// What became of one name.
enum Done {
    Skipped(String),
    Asked {
        word: String,
        outcome: Outcome,
        requests: u32,
    },
}

/// The printed text of every face, the face names in front when there are several.
fn oracle_of(card: &PrintedCard) -> String {
    if card.faces.len() < 2 {
        return card
            .faces
            .first()
            .map(|face| face.oracle_text.clone())
            .unwrap_or_default();
    }
    card.faces
        .iter()
        .map(|face| format!("{}: {}", face.name, face.oracle_text))
        .collect::<Vec<_>>()
        .join("\n//\n")
}

impl LineCtx<'_> {
    fn pool(&self) -> &Pool {
        self.pool
            .get_or_init(|| Pool::load_with(self.translations, Some(self.forge), self.cards))
    }

    /// The card of a name that line mode can work on, with the request of each gapped line; the error says why it is
    /// skipped.
    pub fn prepare<'p>(&'p self, name: &str) -> Result<Prepared<'p>, String> {
        let at = select::by_names(self.cards, &[name.to_owned()])
            .map_err(|_| "not in the database".to_owned())?
            .first()
            .copied()
            .ok_or("not in the database")?;
        let card = &self.cards[at];
        if self
            .legal
            .is_some_and(|legal| !legal.contains(&card.oracle_id) || digital_only(card))
        {
            return Err("not Commander-legal".into());
        }
        if !select::in_sweep_scope(card, self.library, self.caps) {
            return Err("out of scope (hand-written, compiled or textless)".into());
        }
        let translation = Translation::load(self.translations, &card.name)?;
        let ats = translation.gaps()?;
        let script = self.forge.script_of(card).ok_or("no Forge script")?;
        let oracle = oracle_of(card);
        let mut gaps = Vec::new();
        let mut requests = Vec::new();
        for at in ats {
            let gap = describe(card, &script, &at).ok_or("a gapped line is not in the script")?;
            let examples = self.pool().nearest(
                gap.kind,
                &Shape::of(&gap.forge),
                &at.text,
                &card.name,
                EXAMPLES,
            );
            let about = part::Card {
                name: &card.name,
                oracle: &oracle,
                proven: &translation.proposal,
            };
            requests.push(part::request(self.model, about, &gap, &examples));
            gaps.push((at, gap));
        }
        Ok(Prepared {
            card,
            translation,
            gaps,
            requests,
        })
    }

    /// Asks the model about each name in order and saves the results in `out`.
    pub fn run(
        &self,
        models: &dyn LineModels,
        names: &[String],
        out: &Path,
        mut options: LineOptions,
        progress: &Progress,
    ) -> Result<String, CliError> {
        let store = Store::new(out);
        let results = out.join(RESULTS);
        let done: BTreeSet<String> = if options.resume {
            read_results(&results)
                .into_iter()
                .map(|row| row.name)
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut failures = 0;
        let mut stopped = None;
        for name in names.iter().filter(|name| !done.contains(*name)) {
            let started = Instant::now();
            let finished = match self.prepare(name) {
                Ok(prepared) => match self.ask(&prepared, models, &store, &mut options)? {
                    Ok(done) => done,
                    Err(stop) => {
                        stopped = Some(stop);
                        break;
                    }
                },
                Err(why) => Done::Skipped(why),
            };
            let secs = started.elapsed().as_secs_f64();
            let row = Row::of(name, &finished, secs);
            append(&results, &row)?;
            progress.line(row.display());
            failures = match finished {
                Done::Asked {
                    outcome: Outcome::Failed(_),
                    ..
                } => failures + 1,
                _ => 0,
            };
            if failures >= FAILURES_IN_A_ROW {
                return Err(CliError::Config(format!(
                    "{failures} cards in a row got no answer; the last: {}",
                    row.note
                )));
            }
        }
        store.write_summary()?;
        let mut text = render(&read_results(&results), out);
        if let Some(spend) = &options.meter.spend {
            let _ = write!(
                text,
                "\nspent: {} this run; the ledger holds {} of the cap {}",
                spend.run(),
                spend.ledger_total()?,
                spend.cap()
            );
        }
        if let Some(stop) = stopped {
            let _ = write!(text, "\nstopped: {stop}; `--resume` goes on");
        }
        Ok(text)
    }

    /// Asks a card's lines, repairs up to [`card::LINE_REPAIRS`] rounds, merges, judges and saves; the error of the
    /// inner result is the spend cap's stop.
    fn ask(
        &self,
        prepared: &Prepared<'_>,
        models: &dyn LineModels,
        store: &Store,
        options: &mut LineOptions,
    ) -> Result<Result<Done, Stop>, CliError> {
        let mut work = Work::new(prepared, options.samples);
        let attempt = loop {
            let mut answers: Vec<Vec<Option<String>>> = vec![Vec::new(); work.asking.len()];
            let mut failure = None;
            let pending: Vec<(usize, usize, crate::propose::model::Request)> = work
                .pending()
                .into_iter()
                .map(|(line, sample, request)| (line, sample, request.clone()))
                .collect();
            for (line, sample, request) in &pending {
                let (line, sample) = (*line, *sample);
                let client = models.client(prepared.gaps[line].1.kind)?;
                let sent = options.meter.send(&*client, request, &work.name)?;
                let (answer, cost) = match sent {
                    Sent::Answer(text, cost) => (Some(text), cost),
                    Sent::Unanswered(why, cost) => {
                        failure = Some(why);
                        (None, cost)
                    }
                    Sent::Stop(stop) => return Ok(Err(stop)),
                };
                debug_assert_eq!(answers[line].len(), sample);
                answers[line].push(answer);
                work.requests += 1;
                work.micros = work.micros.saturating_add(cost.0);
            }
            match self.step(prepared, &mut work, &answers, failure) {
                Step::Again => {}
                Step::Done(attempt) => break attempt,
            }
        };
        let requests = work.requests;
        let (outcome, label, _) = self.finish(prepared, store, &work, attempt)?;
        Ok(Ok(Done::Asked {
            word: label,
            outcome,
            requests,
        }))
    }
}

/// Writes `text` and a line break to `path`, making its folder.
pub(crate) fn write(path: &Path, text: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    fs::write(path, format!("{text}\n")).map_err(|source| CliError::io(path, source))
}

/// One line of the results file.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The name as given.
    pub name: String,
    /// The result word, or `skipped`.
    pub word: String,
    /// Requests sent.
    pub requests: u32,
    /// Seconds spent.
    pub secs: f64,
    /// Why it was skipped, or the error.
    pub note: String,
}

impl Row {
    fn of(name: &str, done: &Done, secs: f64) -> Row {
        let (word, requests, note) = match done {
            Done::Skipped(why) => ("skipped".to_owned(), 0, why.clone()),
            Done::Asked {
                outcome: Outcome::Failed(why),
                requests,
                ..
            } => ("no answer".to_owned(), *requests, why.clone()),
            Done::Asked { word, requests, .. } => (word.clone(), *requests, String::new()),
        };
        Row {
            name: name.to_owned(),
            word,
            requests,
            secs,
            note,
        }
    }

    /// A row of a card asked through a batch.
    pub(crate) fn asked(name: &str, word: &str, outcome: &Outcome, requests: u32) -> Row {
        let note = match outcome {
            Outcome::Failed(why) => why.clone(),
            _ => String::new(),
        };
        Row {
            name: name.to_owned(),
            word: word.to_owned(),
            requests,
            secs: 0.0,
            note,
        }
    }

    /// A row of a card that line mode cannot work on.
    pub(crate) fn skipped(name: &str, why: &str) -> Row {
        Row {
            name: name.to_owned(),
            word: "skipped".to_owned(),
            requests: 0,
            secs: 0.0,
            note: why.to_owned(),
        }
    }

    /// The row as a line of progress.
    pub(crate) fn display(&self) -> String {
        let note = if self.note.is_empty() {
            String::new()
        } else {
            format!(": {}", self.note)
        };
        format!(
            "{}: {} ({} requests, {:.1} s){note}",
            self.name, self.word, self.requests, self.secs
        )
    }
}

/// Adds a row to the results file.
pub(crate) fn append(path: &Path, row: &Row) -> Result<(), CliError> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CliError::io(path, source))?;
    let line = format!(
        "{}\t{}\t{}\t{:.3}\t{}\n",
        row.name,
        row.word,
        row.requests,
        row.secs,
        row.note.replace(['\t', '\n'], " ")
    );
    file.write_all(line.as_bytes())
        .map_err(|source| CliError::io(path, source))
}

/// The rows of a results file; none when it is missing.
pub fn read_results(path: &Path) -> Vec<Row> {
    let text = fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(Row {
                name: parts.next()?.to_owned(),
                word: parts.next()?.to_owned(),
                requests: parts.next()?.parse().ok()?,
                secs: parts.next()?.parse().ok()?,
                note: parts.next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// The summary of a run: a count for each result, the seconds a card took, and what may play.
pub fn render(rows: &[Row], out: &Path) -> String {
    let mut text = format!(
        "propose --lines: {} names in {}\n",
        rows.len(),
        out.display()
    );
    let asked: Vec<&Row> = rows.iter().filter(|row| row.word != "skipped").collect();
    let count = |word: &str| rows.iter().filter(|row| row.word == word).count();
    for word in [
        "accepted Verified",
        "accepted Partial",
        "unchecked",
        "mismatch",
        "invalid",
        "cannot express",
        "accepted but empty",
        "no answer",
        "skipped",
    ] {
        let _ = writeln!(text, "{word}: {}", count(word));
    }
    let secs: f64 = asked.iter().map(|row| row.secs).sum();
    #[allow(clippy::cast_precision_loss)]
    let each = if asked.is_empty() {
        0.0
    } else {
        secs / asked.len() as f64
    };
    let _ = writeln!(
        text,
        "asked: {} cards, {each:.1} seconds per card",
        asked.len()
    );
    text.push_str(SAFETY);
    text
}

/// What `propose --lines` reads from the database and the cache.
pub struct Stored {
    /// The stored cards.
    pub cards: Vec<PrintedCard>,
    /// The oracle ids of the Commander-legal ones.
    pub legal: BTreeSet<String>,
    /// Where the results go.
    pub out: PathBuf,
    /// The spend cap of a paid run; none for a dry run or a local server.
    pub spend: Option<super::spend::Spend>,
}

/// `propose --lines` from the command line, on the cards of the database: the dry run's prompt and prices, or the run's
/// summary.
pub async fn command(
    stored: Stored,
    args: &ProposeArgs,
    progress: Progress,
    env: &dyn Fn(&str) -> Option<String>,
    api: &super::anthropic::AnthropicConfig,
) -> Result<String, CliError> {
    let Stored {
        cards,
        legal,
        out,
        spend,
    } = stored;
    check_args(args)?;
    let (Some(names_file), Some(translations), Some(forge_dir)) =
        (&args.names_file, &args.translations, &args.forge_hint)
    else {
        return Err(CliError::Config(
            "--lines needs --names-file, --translations and --forge-hint".into(),
        ));
    };
    let names = read_names(names_file)?;
    let forge = ForgeHints::open(forge_dir)?;
    let model = super::options_of(args)?.model;
    let samples = usize::from(args.samples);
    let (translations, dry, resume) = (translations.clone(), args.dry_run, args.resume);
    let models = if dry {
        None
    } else {
        Some(models_of(args, env, api, &progress)?)
    };
    let batch_api = if args.batch && !dry {
        Some(super::batch::BatchApi::new(
            super::anthropic_client_of(args, env, api, progress.clone())?,
            args.reasoning.map(crate::args::Reasoning::as_str),
        ))
    } else {
        None
    };
    let settings = batch::BatchSettings {
        size: usize::try_from(args.batch_size).unwrap_or(usize::MAX),
        poll_every: std::time::Duration::from_secs(args.poll_every),
        schema: args.schema,
    };
    let max_spend = args.max_spend;
    // The clients block on the runtime, so the loop runs on a blocking thread.
    tokio::task::spawn_blocking(move || {
        let library = mtg_cards::builtin::library().map_err(|errors| {
            CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
        })?;
        let known = crate::compile::names_of(&cards);
        let caps = mtg_engine::caps();
        let ctx = LineCtx {
            cards: &cards,
            library,
            names: &known,
            caps: &caps,
            forge: &forge,
            translations: &translations,
            model: &model,
            legal: Some(&legal),
            pool: OnceLock::new(),
        };
        if dry {
            return ctx.dry_run(&names, samples, max_spend);
        }
        let options = LineOptions {
            samples,
            resume,
            meter: Meter { spend },
        };
        if let Some(api) = &batch_api {
            return ctx.run_batch(api, &names, &out, options, &settings, &progress);
        }
        let models = models.ok_or_else(|| CliError::Config("no model client".into()))?;
        ctx.run(&*models, &names, &out, options, &progress)
    })
    .await
    .map_err(|error| CliError::Config(format!("the run stopped: {error}")))?
}

/// The clients of a run the arguments name: the Anthropic API with the key of `env`, or a local server.
fn models_of(
    args: &ProposeArgs,
    env: &dyn Fn(&str) -> Option<String>,
    api: &super::anthropic::AnthropicConfig,
    progress: &Progress,
) -> Result<Box<dyn LineModels>, CliError> {
    let handle = tokio::runtime::Handle::current();
    let mut config = api.clone();
    config.timeout = std::time::Duration::from_secs(args.request_timeout);
    config.attempts = args.retries.saturating_add(1);
    Ok(match args.provider {
        Provider::Anthropic => {
            if let Some(base) = &args.base_url {
                config.base_url.clone_from(base);
            }
            let base = super::anthropic::Anthropic {
                effort: args.reasoning.map(crate::args::Reasoning::as_str),
                workspace: super::anthropic::workspace_from_env(env),
            };
            Box::new(paid::PaidModels::new(
                super::anthropic::ApiKey::from_env(env)?,
                config,
                handle,
                progress.clone(),
                base,
                args.schema,
            ))
        }
        Provider::Openai => {
            config.base_url = args.base_url.clone().unwrap_or_default();
            let base = super::c11i_openai::OpenAi {
                json_mode: false,
                schema: false,
                max_answer_tokens: args.max_answer_tokens.min(part::LINE_ANSWER_TOKENS),
                reasoning: args.reasoning,
            };
            Box::new(client::HttpModels::new(
                base,
                config,
                handle,
                progress.clone(),
            ))
        }
    })
}
