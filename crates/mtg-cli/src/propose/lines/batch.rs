//! `propose --lines --batch`: the line requests of many cards through the Message Batches API at half the price, round by
//! round (the first answers with their samples, then the repair rounds), each piece sized to the spend cap and
//! `--batch-size`. A state file in the output directory lets `--resume` collect a batch that is out; a batch is settled
//! into the ledger once.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::card::{Prepared, Step, Work};
use super::paid::{format_of, line_body};
use super::part::Kind;
use super::{LineCtx, LineOptions, RESULTS, Row, append, read_results, render};
use crate::error::CliError;
use crate::progress::Progress;
use crate::propose::batch::{Answered, BatchApi, open, settle, usage_of, wait};
use crate::propose::budget::Stop;
use crate::propose::price::Usd;
use crate::propose::spend::{Refused, Spend};
use crate::propose::store::Store;

/// The file in the output directory that says which batch of line requests is out.
pub const STATE_FILE: &str = "lines-batch-state.json";

/// How a batch run asks.
#[derive(Debug, Clone)]
pub struct BatchSettings {
    /// The most requests in one batch.
    pub size: usize,
    /// The time between two looks at a batch that is out.
    pub poll_every: Duration,
    /// Constrain each answer to the schema of its kind when the API takes it.
    pub schema: bool,
}

/// The batch that is out and the cards it asks about, in the order of their custom ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    model: String,
    round: u8,
    ticket: String,
    batch_id: String,
    reserved_micros: u64,
    works: Vec<Work>,
}

fn state_path(out: &Path) -> PathBuf {
    out.join(STATE_FILE)
}

fn load_state(out: &Path) -> Result<Option<State>, CliError> {
    let path = state_path(out);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| CliError::Cache {
                path,
                reason: format!("the batch state does not read: {error}"),
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CliError::io(path, error)),
    }
}

fn save_state(out: &Path, state: &State) -> Result<(), CliError> {
    let text = serde_json::to_string(state).unwrap_or_default();
    super::write(&state_path(out), &text)
}

fn clear_state(out: &Path) -> Result<(), CliError> {
    let path = state_path(out);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::io(path, error)),
    }
}

/// The custom id of a request: the card's place in the batch, its line and its sample.
fn custom_id(work: usize, line: usize, sample: usize) -> String {
    format!("w{work}-l{line}-s{sample}")
}

/// What a batch run carries from round to round.
struct Batch<'r> {
    api: &'r BatchApi,
    spend: Spend,
    store: Store,
    out: PathBuf,
    settings: &'r BatchSettings,
    progress: &'r Progress,
    formats: BTreeMap<Kind, Option<Value>>,
    stopped: Option<Stop>,
}

impl LineCtx<'_> {
    /// Asks about `names` through batches until the names or the money are spent, and saves the results in `out`.
    pub fn run_batch(
        &self,
        api: &BatchApi,
        names: &[String],
        out: &Path,
        options: LineOptions,
        settings: &BatchSettings,
        progress: &Progress,
    ) -> Result<String, CliError> {
        let spend = options.meter.spend.ok_or_else(|| {
            CliError::Config("--batch needs --max-spend: a batch is paid before it ends".into())
        })?;
        let results = out.join(RESULTS);
        let mut done: BTreeSet<String> = if options.resume {
            read_results(&results)
                .into_iter()
                .map(|row| row.name)
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut batch = Batch {
            api,
            spend,
            store: Store::new(out),
            out: out.to_path_buf(),
            settings,
            progress,
            formats: BTreeMap::new(),
            stopped: None,
        };
        if let Some(state) = load_state(out)? {
            if !options.resume {
                return Err(CliError::Config(format!(
                    "a batch ({}) is still out for the cards in {}; `--resume` collects it, and deleting {} drops it unpaid",
                    state.batch_id,
                    out.display(),
                    state_path(out).display()
                )));
            }
            if state.model != self.model {
                return Err(CliError::Config(format!(
                    "the batch that is out was asked of `{}`; resume with --model {}",
                    state.model, state.model
                )));
            }
            progress.line(format_args!(
                "resuming batch {} (round {})",
                state.batch_id, state.round
            ));
            done.extend(state.works.iter().map(|work| work.name.clone()));
            let mut pairs = Vec::new();
            for work in state.works.clone() {
                let prepared = self.prepare(&work.name).map_err(|why| {
                    CliError::Config(format!(
                        "the batch that is out names `{}`: {why}",
                        work.name
                    ))
                })?;
                pairs.push((prepared, work));
            }
            let answers = collect(&mut batch, &state, &pairs)?;
            self.rounds(&mut batch, pairs, Some(answers))?;
        }
        let mut queue: VecDeque<&String> =
            names.iter().filter(|name| !done.contains(*name)).collect();
        while !queue.is_empty() && batch.stopped.is_none() {
            let pairs = self.chunk(&mut batch, &mut queue, options.samples)?;
            if pairs.is_empty() {
                break;
            }
            self.rounds(&mut batch, pairs, None)?;
        }
        batch.store.write_summary()?;
        let mut text = render(&read_results(&results), out);
        let _ = write!(
            text,
            "\nspent: {} this run; the ledger holds {} of the cap {}",
            batch.spend.run(),
            batch.spend.ledger_total()?,
            batch.spend.cap()
        );
        if let Some(stop) = batch.stopped {
            let _ = write!(text, "\nstopped: {stop}; `--resume` goes on");
        }
        Ok(text)
    }

    /// The next cards whose first requests fit the cap and the size of a batch; a name line mode cannot work on is
    /// written as skipped.
    fn chunk<'p>(
        &'p self,
        batch: &mut Batch<'_>,
        queue: &mut VecDeque<&String>,
        samples: usize,
    ) -> Result<Vec<(Prepared<'p>, Work)>, CliError> {
        let results = batch.out.join(RESULTS);
        let mut pairs: Vec<(Prepared<'p>, Work)> = Vec::new();
        let (mut worst, mut count) = (Usd(0), 0usize);
        while let Some(name) = queue.front() {
            let prepared = match self.prepare(name) {
                Ok(prepared) => prepared,
                Err(why) => {
                    let row = Row::skipped(name, &why);
                    append(&results, &row)?;
                    batch.progress.line(row.display());
                    queue.pop_front();
                    continue;
                }
            };
            let work = Work::new(&prepared, samples);
            let pending = work.pending();
            if !pairs.is_empty() && count + pending.len() > batch.settings.size {
                break;
            }
            let own = pending.iter().fold(Usd(0), |sum, (_, _, request)| {
                sum.plus(batch.spend.worst_case(request))
            });
            match batch.spend.check(worst.plus(own)) {
                Ok(()) => {}
                Err(Refused::Cap(stop)) => {
                    // A piece the cap cuts short is not the end: what its answers cost may leave room for the next.
                    if pairs.is_empty() {
                        batch.stopped.get_or_insert(stop);
                    }
                    break;
                }
                Err(Refused::Ledger(error)) => return Err(error),
            }
            worst = worst.plus(own);
            count += pending.len();
            pairs.push((prepared, work));
            queue.pop_front();
        }
        Ok(pairs)
    }

    /// Asks the rounds of `pairs` until every card is done: a batch for the pending requests, the answers judged, the
    /// repairs into the next batch. `first` holds the answers of a batch that was out when the run began.
    fn rounds(
        &self,
        batch: &mut Batch<'_>,
        mut pairs: Vec<(Prepared<'_>, Work)>,
        mut first: Option<BTreeMap<String, Answered>>,
    ) -> Result<(), CliError> {
        while !pairs.is_empty() {
            let answers = if let Some(answers) = first.take() {
                answers
            } else {
                {
                    let worst = pairs.iter().fold(Usd(0), |sum, (_, work)| {
                        work.pending().iter().fold(sum, |sum, (_, _, request)| {
                            sum.plus(batch.spend.worst_case(request))
                        })
                    });
                    match batch.spend.check(worst) {
                        Ok(()) => {}
                        Err(Refused::Cap(stop)) => {
                            // A repair round the cap cannot hold leaves the answers that stand.
                            batch.stopped.get_or_insert(stop);
                            for (prepared, work) in pairs {
                                let attempt = self.standing(&prepared, &work);
                                self.done(batch, &prepared, &work, attempt)?;
                            }
                            return clear_state(&batch.out);
                        }
                        Err(Refused::Ledger(error)) => return Err(error),
                    }
                    let state = self.submit(batch, &pairs, worst)?;
                    collect(batch, &state, &pairs)?
                }
            };
            let mut next = Vec::new();
            for (at, (prepared, mut work)) in pairs.into_iter().enumerate() {
                let mut by_line: Vec<Vec<Option<String>>> = vec![Vec::new(); work.asking.len()];
                let mut failure = None;
                for (line, sample, _) in work.pending() {
                    let answered = answers.get(&custom_id(at, line, sample));
                    let text = match answered {
                        Some(Answered::Text(response)) => Some(response.text.clone()),
                        Some(Answered::NoText { why, .. }) => {
                            failure = Some(format!("the answer holds no text: {why}"));
                            None
                        }
                        Some(Answered::Failed(why)) => {
                            failure = Some(why.clone());
                            None
                        }
                        None => {
                            failure = Some("the batch has no result for the request".into());
                            None
                        }
                    };
                    by_line[line].push(text);
                }
                for (line, sample) in work.slots() {
                    if let Some(usage) = usage_of(answers.get(&custom_id(at, line, sample))) {
                        let cost = batch.spend.price().cost(&usage, batch.spend.billing());
                        work.micros = work.micros.saturating_add(cost.0);
                    }
                    work.requests += 1;
                }
                match self.step(&prepared, &mut work, &by_line, failure) {
                    Step::Again => next.push((prepared, work)),
                    Step::Done(attempt) => self.done(batch, &prepared, &work, attempt)?,
                }
            }
            pairs = next;
        }
        clear_state(&batch.out)
    }

    fn done(
        &self,
        batch: &Batch<'_>,
        prepared: &Prepared<'_>,
        work: &Work,
        attempt: super::card::Attempt,
    ) -> Result<(), CliError> {
        let (outcome, label, _) = self.finish(prepared, &batch.store, work, attempt)?;
        let row = Row::asked(&work.name, &label, &outcome, work.requests);
        append(&batch.out.join(RESULTS), &row)?;
        batch.progress.line(row.display());
        Ok(())
    }

    /// Asks for the batch of every pending request of `pairs` and saves the state that finds it again.
    fn submit(
        &self,
        batch: &mut Batch<'_>,
        pairs: &[(Prepared<'_>, Work)],
        worst: Usd,
    ) -> Result<State, CliError> {
        let mut params = Vec::new();
        for (at, (prepared, work)) in pairs.iter().enumerate() {
            for (line, sample, request) in work.pending() {
                let kind = prepared.gaps[line].1.kind;
                let schema = batch.settings.schema;
                let format = batch
                    .formats
                    .entry(kind)
                    .or_insert_with(|| format_of(kind, schema));
                let body = line_body(request, batch.api.effort(), format.as_ref());
                params.push((custom_id(at, line, sample), body));
            }
        }
        let round = pairs.iter().map(|(_, work)| work.round).max().unwrap_or(0) + 1;
        let opened = open(
            batch.api,
            &batch.spend,
            self.model,
            round,
            &params,
            worst,
            batch.progress,
        )?;
        let state = State {
            model: self.model.to_owned(),
            round,
            ticket: opened.ticket,
            batch_id: opened.batch_id,
            reserved_micros: worst.0,
            works: pairs.iter().map(|(_, work)| work.clone()).collect(),
        };
        save_state(&batch.out, &state)?;
        Ok(state)
    }
}

/// Waits for the batch and settles it into the ledger.
fn collect(
    batch: &mut Batch<'_>,
    state: &State,
    pairs: &[(Prepared<'_>, Work)],
) -> Result<BTreeMap<String, Answered>, CliError> {
    let answers = wait(
        batch.api,
        &state.batch_id,
        batch.settings.poll_every,
        batch.progress,
    )?;
    let mut billed = Vec::new();
    for (at, (_, work)) in pairs.iter().enumerate() {
        for (line, sample, _) in work.pending() {
            if let Some(usage) = usage_of(answers.get(&custom_id(at, line, sample))) {
                billed.push((work.name.clone(), usage));
            }
        }
    }
    let total = settle(
        &mut batch.spend,
        &state.model,
        &state.ticket,
        Usd(state.reserved_micros),
        &billed,
    )?;
    batch.progress.line(format_args!(
        "batch {} ended: {total} billed",
        state.batch_id
    ));
    Ok(answers)
}
