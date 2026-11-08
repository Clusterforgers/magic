//! `propose --lines`: for a card whose translation has exactly one gapped line, a local model writes only that ability
//! from a small prompt and a schema for its kind; the part is merged into the translation at its line and the checker
//! judges the whole card. Only the checker's `Verified` can play without a player's read (`docs/cards-dsl.md`).

pub mod client;
pub mod part;
pub mod pool;

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
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::check;
use serde_json::{Value, json};

use self::part::Kind;
use self::pool::{Gap, GapAt, Pool, Translation, describe, merge};
use super::forge_hint::ForgeHints;
use super::model::{Message, ModelClient, Request};
use super::outcome::Outcome;
use super::prompt::{diagnosis, extract_json};
use super::select;
use super::store::Store;
use crate::args::{ProposeArgs, Provider};
use crate::error::CliError;
use crate::progress::Progress;

/// The file of a run that holds one line for each card: name, result, requests, seconds and a note.
pub const RESULTS: &str = "lines.tsv";

/// Cards in a row that get no answer before the run gives up.
const FAILURES_IN_A_ROW: usize = 5;

/// The sentence that closes the report: what may play without a read.
pub const SAFETY: &str = "Only `accepted Verified`, the checker's own pass, can play without a player's read. Every other result is a proposal marked source `sweep` (model-written, unread) in its meta file, and plays only after a player reads and approves it.";

/// Refuses the flag combinations line mode does not allow: the local provider with an address of its own.
pub fn check_args(args: &ProposeArgs) -> Result<(), CliError> {
    if args.provider != Provider::Openai || args.base_url.is_none() {
        return Err(CliError::Config(
            "--lines runs on a local model only: pass --provider openai with --base-url, such as http://localhost:11434/v1".into(),
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

/// The clients of a run, one for each kind of ability (each sends the schema of its kind).
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
}

/// A card ready to ask about.
struct Prepared<'a> {
    card: &'a PrintedCard,
    translation: Translation,
    at: GapAt,
    gap: Gap,
    request: Request,
}

/// What became of one name.
enum Done {
    Skipped(String),
    Asked(Outcome, u32),
}

impl LineCtx<'_> {
    /// The card of a name that line mode can work on, with its request; the error says why it is skipped.
    fn prepare<'p>(&'p self, name: &str, pool: &OnceLock<Pool>) -> Result<Prepared<'p>, String> {
        let at = select::by_names(self.cards, &[name.to_owned()])
            .map_err(|_| "not in the database".to_owned())?
            .first()
            .copied()
            .ok_or("not in the database")?;
        let card = &self.cards[at];
        if !select::in_sweep_scope(card, self.library, self.caps) {
            return Err("out of scope (hand-written, compiled or textless)".into());
        }
        let translation = Translation::load(self.translations, &card.name)?;
        let at = translation.single_gap()?;
        let script = self.forge.script_of(card).ok_or("no Forge script")?;
        let gap = describe(card, &script, &at).ok_or("the gapped line is not in the script")?;
        let pool = pool.get_or_init(|| Pool::load(self.translations));
        let examples = pool.nearest(gap.kind, &at.text, 3);
        let request = part::request(self.model, &card.name, &gap, gap.kind, &examples);
        Ok(Prepared {
            card,
            translation,
            at,
            gap,
            request,
        })
    }

    /// The prompt of the first card that can be asked, as a dry run shows it.
    pub fn dry_run(&self, names: &[String]) -> Result<String, CliError> {
        let pool = OnceLock::new();
        let mut skipped = 0;
        for name in names {
            let Ok(done) = self.prepare(name, &pool) else {
                skipped += 1;
                continue;
            };
            let tokens = done.request.estimated_tokens();
            return Ok(format!(
                "propose --lines --dry-run: nothing is sent\nmodel: {}\nnames: {} ({skipped} skipped before the first)\nfirst: {} ({}, schema of {} bytes)\nestimated input tokens of one request: {tokens} (an upper estimate, 3 bytes a token)\n\n--- system ---\n{}\n\n--- user ---\n{}",
                self.model,
                names.len(),
                done.card.name,
                done.gap.kind.word(),
                part::schema(done.gap.kind).to_string().len(),
                done.request.system,
                done.request
                    .messages
                    .first()
                    .map_or("", |m| m.text.as_str()),
            ));
        }
        Err(CliError::Config("no name can be asked in line mode".into()))
    }

    /// Asks the model about each name in order and saves the results in `out`.
    pub fn run(
        &self,
        models: &dyn LineModels,
        names: &[String],
        out: &Path,
        resume: bool,
        progress: &Progress,
    ) -> Result<String, CliError> {
        let store = Store::new(out);
        let results = out.join(RESULTS);
        let pool = OnceLock::new();
        let done: BTreeSet<String> = if resume {
            read_results(&results)
                .into_iter()
                .map(|row| row.name)
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut failures = 0;
        for name in names.iter().filter(|name| !done.contains(*name)) {
            let started = Instant::now();
            let finished = match self.prepare(name, &pool) {
                Ok(prepared) => self.ask(&prepared, models, &store)?,
                Err(why) => Done::Skipped(why),
            };
            let secs = started.elapsed().as_secs_f64();
            let row = Row::of(name, &finished, secs);
            append(&results, &row)?;
            progress.line(row.display());
            failures = if matches!(finished, Done::Asked(Outcome::Failed(_), _)) {
                failures + 1
            } else {
                0
            };
            if failures >= FAILURES_IN_A_ROW {
                return Err(CliError::Config(format!(
                    "{failures} cards in a row got no answer; the last: {}",
                    row.note
                )));
            }
        }
        Ok(render(&read_results(&results), out))
    }

    /// Asks for the ability, repairs once, merges and judges.
    fn ask(
        &self,
        prepared: &Prepared<'_>,
        models: &dyn LineModels,
        store: &Store,
    ) -> Result<Done, CliError> {
        let client = models.client(prepared.gap.kind)?;
        let mut requests = 0;
        let mut request = prepared.request.clone();
        let mut judged = None;
        for round in 0..2 {
            requests += 1;
            let answer = match client.complete(&request) {
                Ok(answer) => answer.text,
                Err(error) if error.is_fatal() => return Err(CliError::Model(error)),
                // The first answer stands when its repair round gets none.
                Err(_) if round > 0 => break,
                Err(error) => {
                    return Ok(Done::Asked(Outcome::Failed(error.to_string()), requests));
                }
            };
            let attempt = self.judge(prepared, &answer);
            let problem = match &attempt {
                Err(problem) => Some(problem.clone()),
                Ok(judgement) if Outcome::of_judged(judgement).is_repairable() => {
                    Some(diagnosis(&judgement.report))
                }
                Ok(_) => None,
            };
            let repair = problem.filter(|_| round == 0);
            judged = Some((answer.clone(), attempt));
            let Some(problem) = repair else { break };
            request.messages.push(Message::assistant(answer));
            request
                .messages
                .push(Message::user(part::repair_message(&problem)));
        }
        let (answer, attempt) = judged.ok_or_else(|| CliError::Config("no answer".into()))?;
        let outcome = self.keep(prepared, store, &answer, attempt)?;
        Ok(Done::Asked(outcome, requests))
    }

    /// Reads the answer as one ability, merges it at the gap and checks the whole card; the error is for the model.
    fn judge(&self, prepared: &Prepared<'_>, answer: &str) -> Result<Judged, String> {
        let value: Value = serde_json::from_str(extract_json(answer))
            .map_err(|error| format!("the answer is not JSON: {error}"))?;
        serde_json::from_value::<Ability>(value.clone())
            .map_err(|error| format!("the answer is not an ability of the rules: {error}"))?;
        let merged = merge(
            &prepared.translation.proposal,
            prepared.at.face,
            prepared.at.at,
            value,
        )
        .ok_or("the translation has no place for the ability")?;
        let json = merged.to_string();
        let report = check(&json, prepared.card, self.library, self.names, self.caps);
        Ok(Judged { json, report })
    }

    /// Saves the result: `Verified` and `Partial` as accepted proposals, `Unchecked` under `unchecked/`, the rest under
    /// `rejected/`. Every file has a meta file that marks the source as model-written and unread.
    fn keep(
        &self,
        prepared: &Prepared<'_>,
        store: &Store,
        answer: &str,
        attempt: Result<Judged, String>,
    ) -> Result<Outcome, CliError> {
        let name = &prepared.card.name;
        let meta = json!({
            "source": mtg_db::SOURCE_SWEEP,
            "written_by": "model-line",
            "model": self.model,
            "kind": prepared.gap.kind.word(),
            "line": prepared.gap.oracle,
        });
        let dir = store.dir();
        let stem = slug(name);
        let write_meta = |base: &Path| {
            write(
                &base.join("meta").join(format!("{stem}.json")),
                &meta.to_string(),
            )
        };
        let judged = match attempt {
            Err(problem) => {
                store.save_rejected(name, answer, &problem)?;
                return Ok(Outcome::Invalid);
            }
            Ok(judged) => judged,
        };
        let outcome = Outcome::of_judged(&judged);
        match (&judged.report.rules, outcome.is_kept(), &outcome) {
            (Some(rules), true, _) => {
                store.save_accepted(name, rules)?;
                write_meta(dir)?;
            }
            (_, _, Outcome::Unchecked) => {
                let base = dir.join("unchecked");
                write(&base.join(format!("{stem}.json")), &judged.json)?;
                write_meta(&base)?;
            }
            _ => store.save_rejected(name, &judged.json, &judged.report.render(false))?,
        }
        Ok(outcome)
    }
}

/// A merged proposal and the checker's report of it.
struct Judged {
    json: String,
    report: mtg_cards::proposal::Report,
}

impl Outcome {
    fn of_judged(judged: &Judged) -> Outcome {
        Outcome::of(&judged.report)
    }
}

fn write(path: &Path, text: &str) -> Result<(), CliError> {
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
            Done::Asked(Outcome::Failed(why), requests) => {
                ("no answer".to_owned(), *requests, why.clone())
            }
            Done::Asked(outcome, requests) => (outcome.word().to_owned(), *requests, String::new()),
        };
        Row {
            name: name.to_owned(),
            word,
            requests,
            secs,
            note,
        }
    }

    fn display(&self) -> String {
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

fn append(path: &Path, row: &Row) -> Result<(), CliError> {
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

/// `propose --lines` from the command line, on the cards of the database: the dry run's prompt or the run's summary.
pub async fn command(
    cards: Vec<PrintedCard>,
    out: PathBuf,
    args: &ProposeArgs,
    progress: Progress,
    api: &super::anthropic::AnthropicConfig,
) -> Result<String, CliError> {
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
    let mut config = api.clone();
    config.base_url = args.base_url.clone().unwrap_or_default();
    config.timeout = std::time::Duration::from_secs(args.request_timeout);
    config.attempts = args.retries.saturating_add(1);
    let base = super::c11i_openai::OpenAi {
        json_mode: false,
        schema: false,
        max_answer_tokens: args.max_answer_tokens.min(part::LINE_ANSWER_TOKENS),
        reasoning: args.reasoning,
    };
    let (translations, dry, resume) = (translations.clone(), args.dry_run, args.resume);
    let handle = tokio::runtime::Handle::current();
    // The client blocks on the runtime, so the loop runs on a blocking thread.
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
        };
        if dry {
            return ctx.dry_run(&names);
        }
        let models = client::HttpModels::new(base, config, handle, progress.clone());
        ctx.run(&models, &names, &out, resume, &progress)
    })
    .await
    .map_err(|error| CliError::Config(format!("the run stopped: {error}")))?
}
