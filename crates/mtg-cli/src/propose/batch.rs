//! `propose --batch`: the same questions through the Message Batches API at half the price. A batch is asked for in
//! pieces that fit the spend cap, waited for by polling, and settled into the ledger when its results are read; a state
//! file in the output directory lets `--resume` pick up a batch that is out.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mtg_cards::PrintedCard;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::anthropic::{AnthropicClient, parse_message, request_body_with};
use super::budget::{Budget, Stop};
use super::forge_hint::HintState;
use super::ledger::{Entry, reserved_label, settled_label};
use super::model::{ModelError, Request, Response, Usage};
use super::outcome::{Outcome, ProposeReport, Tally};
use super::price::Usd;
use super::prompt::{first_request, repair_request};
use super::session::{Context, Finished, Options, Used, judge, keep, spend_report};
use super::spend::{Refused, Spend, signed};
use super::store::Store;
use crate::error::CliError;
use crate::progress::Progress;

/// The most requests in one batch when `--batch-size` is not given.
pub const DEFAULT_BATCH_SIZE: u32 = 500;

/// The most bytes of requests in one batch; the API takes 256 MB.
const MAX_BATCH_BYTES: usize = 180_000_000;

/// The file in the output directory that says which batch is out.
pub const STATE_FILE: &str = "batch-state.json";

/// How the API answers a request of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// The model wrote text.
    Text(Response),
    /// The request was billed but the answer has no text.
    NoText {
        /// What it cost.
        usage: Usage,
        /// Why there is no text.
        why: String,
    },
    /// The request errored, expired or was canceled; none of these is billed.
    Failed(String),
}

/// Where a batch is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatchInfo {
    /// The batch id.
    pub id: String,
    /// Whether every request is settled.
    pub ended: bool,
    /// Requests still being processed.
    pub processing: u64,
    /// Requests that got an answer.
    pub succeeded: u64,
    /// Requests that errored.
    pub errored: u64,
    /// Requests that were canceled.
    pub canceled: u64,
    /// Requests that expired.
    pub expired: u64,
}

fn parse_info(text: &str) -> Result<BatchInfo, ModelError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| ModelError::Answer(error.to_string()))?;
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelError::Answer("the batch has no id".into()))?;
    let count = |name: &str| {
        value
            .pointer(&format!("/request_counts/{name}"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    Ok(BatchInfo {
        id: id.to_owned(),
        ended: value.get("processing_status").and_then(Value::as_str) == Some("ended"),
        processing: count("processing"),
        succeeded: count("succeeded"),
        errored: count("errored"),
        canceled: count("canceled"),
        expired: count("expired"),
    })
}

/// The results of a batch: a JSON line for each request, in any order, keyed by its `custom_id`.
fn parse_results(text: &str) -> Result<BTreeMap<String, Answered>, ModelError> {
    let mut answers = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value =
            serde_json::from_str(line).map_err(|error| ModelError::Answer(error.to_string()))?;
        let Some(id) = value.get("custom_id").and_then(Value::as_str) else {
            return Err(ModelError::Answer("a result has no custom_id".into()));
        };
        let result = value.get("result").unwrap_or(&Value::Null);
        let answered = match result.get("type").and_then(Value::as_str) {
            Some("succeeded") => match parse_message(result.get("message").unwrap_or(&Value::Null))
            {
                Ok(response) => Answered::Text(response),
                Err(ModelError::Unanswered { usage, why }) => Answered::NoText { usage, why },
                Err(error) => Answered::Failed(error.to_string()),
            },
            Some("errored") => {
                let error = result
                    .pointer("/error/error")
                    .or_else(|| result.get("error"));
                let kind = error
                    .and_then(|error| error.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("error");
                let message = error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("no message");
                Answered::Failed(format!("the request errored: {kind}: {message}"))
            }
            Some("expired") => Answered::Failed("the request expired".into()),
            Some("canceled") => Answered::Failed("the request was canceled".into()),
            other => Answered::Failed(format!("unknown result type {other:?}")),
        };
        answers.insert(id.to_owned(), answered);
    }
    Ok(answers)
}

/// The Message Batches API.
pub struct BatchApi {
    client: AnthropicClient,
    effort: Option<&'static str>,
}

impl BatchApi {
    /// The batches endpoints through a client that holds the key; requests ask for `effort` when given.
    pub fn new(client: AnthropicClient, effort: Option<&'static str>) -> Self {
        Self { client, effort }
    }

    /// Asks for a batch. Creating is not repeated after a timeout, which may have left a batch behind.
    pub fn create(&self, items: &[(String, &Request)]) -> Result<BatchInfo, ModelError> {
        let params: Vec<(String, Value)> = items
            .iter()
            .map(|(id, request)| (id.clone(), request_body_with(request, self.effort)))
            .collect();
        self.create_params(&params)
    }

    /// The effort every request asks for.
    pub fn effort(&self) -> Option<&'static str> {
        self.effort
    }

    /// Asks for a batch of request bodies built by the caller, each with its custom id.
    pub fn create_params(&self, items: &[(String, Value)]) -> Result<BatchInfo, ModelError> {
        let requests: Vec<Value> = items
            .iter()
            .map(|(id, params)| json!({"custom_id": id, "params": params}))
            .collect();
        let body = json!({ "requests": requests });
        let text = self
            .client
            .call(&Method::POST, "/v1/messages/batches", Some(&body), false)?;
        parse_info(&text)
    }

    /// Where a batch is.
    pub fn status(&self, id: &str) -> Result<BatchInfo, ModelError> {
        let text = self.client.call(
            &Method::GET,
            &format!("/v1/messages/batches/{id}"),
            None,
            true,
        )?;
        parse_info(&text)
    }

    /// The results of an ended batch, keyed by custom id.
    pub fn results(&self, id: &str) -> Result<BTreeMap<String, Answered>, ModelError> {
        let text = self.client.call(
            &Method::GET,
            &format!("/v1/messages/batches/{id}/results"),
            None,
            true,
        )?;
        parse_results(&text).map(|answers| {
            answers
                .into_iter()
                .map(|(id, answered)| match answered {
                    Answered::Failed(message) => {
                        (id, Answered::Failed(self.client.scrub(&message)))
                    }
                    other => (id, other),
                })
                .collect()
        })
    }
}

/// One request of a batch, with what is known of its card.
struct Item {
    id: String,
    card: String,
    request: Request,
    /// For a repair round: the answer the repair is about.
    answer: Option<String>,
    used: Used,
}

/// The state file: the batch that is out, so that `--resume` can collect it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    model: String,
    /// 1 for the first answers, 2 for the repair round.
    round: u8,
    /// The label of the ledger line that holds the batch's worst case.
    ticket: String,
    batch_id: String,
    reserved_micros: u64,
    entries: Vec<StateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateEntry {
    id: String,
    card: String,
    /// The first answer, for the repair round.
    answer: Option<String>,
    requests: u32,
    micros: u64,
}

/// What `run` needs from outside.
pub struct BatchRun<'a> {
    /// The checker's inputs and the prompts.
    pub ctx: &'a Context<'a>,
    /// The batches endpoints.
    pub api: &'a BatchApi,
    /// The settings of the run.
    pub options: &'a Options,
    /// Where the proposals go.
    pub store: &'a Store,
    /// Where lines of progress go.
    pub progress: &'a Progress,
}

impl BatchRun<'_> {
    /// Asks about `targets` (positions in `all`) through batches until the cards or the money are spent.
    pub fn run(&self, all: &[PrintedCard], targets: &[usize]) -> Result<ProposeReport, CliError> {
        let spend = self.options.spend.clone().ok_or_else(|| {
            CliError::Config("--batch needs --max-spend: a batch is paid before it ends".into())
        })?;
        let mut run = Run {
            cfg: self,
            all,
            budget: Budget::new(self.options.max_requests, self.options.max_input_tokens),
            spend,
            tally: Tally::default(),
            stopped: None,
            started: Instant::now(),
        };
        let chosen: Vec<&PrintedCard> = targets.iter().filter_map(|at| all.get(*at)).collect();
        run.go(&chosen)?;
        let summary = self.store.write_summary();
        let report = ProposeReport {
            model: self.options.model.clone(),
            tally: run.tally,
            budget: run.budget,
            stopped: run.stopped,
            out: self.store.dir().to_path_buf(),
            spend: Some(spend_report(&run.spend)),
        };
        summary?;
        Ok(report)
    }
}

struct Run<'a, 'b> {
    cfg: &'a BatchRun<'b>,
    all: &'a [PrintedCard],
    budget: Budget,
    spend: Spend,
    tally: Tally,
    stopped: Option<Stop>,
    started: Instant,
}

impl Run<'_, '_> {
    fn card(&self, name: &str) -> Result<&PrintedCard, CliError> {
        self.all
            .iter()
            .find(|card| card.name == name)
            .ok_or_else(|| {
                CliError::Config(format!(
                    "the batch in {} is about `{name}`, which is not a stored card",
                    self.cfg.store.dir().display()
                ))
            })
    }

    fn first_of(&self, card: &PrintedCard) -> Request {
        let message = self.cfg.ctx.card_message(card, self.cfg.options.examples);
        first_request(&self.cfg.options.model, &self.cfg.ctx.system, message.text)
    }

    fn state_path(&self) -> PathBuf {
        self.cfg.store.dir().join(STATE_FILE)
    }

    fn load_state(&self) -> Result<Option<State>, CliError> {
        let path = self.state_path();
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

    fn save_state(&self, state: &State) -> Result<(), CliError> {
        let path = self.state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| CliError::io(parent, error))?;
        }
        let text = serde_json::to_string_pretty(state).unwrap_or_default();
        fs::write(&path, format!("{text}\n")).map_err(|error| CliError::io(path, error))
    }

    fn clear_state(&self) -> Result<(), CliError> {
        let path = self.state_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(CliError::io(path, error)),
        }
    }

    /// The whole run: a batch that is out first, then the cards in pieces that fit the cap.
    fn go(&mut self, chosen: &[&PrintedCard]) -> Result<(), CliError> {
        let mut done: BTreeSet<String> = BTreeSet::new();
        if let Some(state) = self.load_state()? {
            if !self.cfg.options.resume {
                return Err(CliError::Config(format!(
                    "a batch ({}) is still out for the cards in {}; `--resume` collects it, and deleting {} drops it unpaid",
                    state.batch_id,
                    self.cfg.store.dir().display(),
                    self.state_path().display()
                )));
            }
            if state.model != self.cfg.options.model {
                return Err(CliError::Config(format!(
                    "the batch that is out was asked of `{}`; resume with --model {}",
                    state.model, state.model
                )));
            }
            self.progress(format_args!(
                "resuming batch {} (round {})",
                state.batch_id, state.round
            ));
            done.extend(state.entries.iter().map(|entry| entry.card.clone()));
            self.finish_state(&state)?;
        }
        let (work, skipped): (Vec<&PrintedCard>, Vec<&PrintedCard>) = chosen
            .iter()
            .filter(|card| !done.contains(&card.name))
            .partition(|card| !(self.cfg.options.resume && self.cfg.store.has(&card.name)));
        self.tally.skipped = skipped.len();
        let mut queue: VecDeque<&PrintedCard> = work.into_iter().collect();
        while !queue.is_empty() && self.stopped.is_none() {
            let items = self.chunk(&mut queue)?;
            if items.is_empty() {
                break;
            }
            self.first_round(items)?;
        }
        self.tally.not_run += queue.len();
        Ok(())
    }

    fn progress(&self, line: impl std::fmt::Display) {
        self.cfg.progress.line(line);
    }

    /// The next requests that fit the cap, the budget and the size of a batch; the cards left over stay in the queue.
    fn chunk(&mut self, queue: &mut VecDeque<&PrintedCard>) -> Result<Vec<Item>, CliError> {
        let size = match self.cfg.options.batch_size {
            0 => usize::try_from(DEFAULT_BATCH_SIZE).unwrap_or(500),
            size => size,
        };
        let mut items: Vec<Item> = Vec::new();
        let (mut worst, mut bytes) = (Usd(0), 0usize);
        while let Some(card) = queue.front() {
            if items.len() >= size {
                break;
            }
            let request = self.first_of(card);
            let own = self.spend.worst_case(&request);
            match self.spend.check(worst.plus(own)) {
                Ok(()) => {}
                Err(Refused::Cap(stop)) => {
                    // A piece the cap cuts short is not the end: what its answers really cost may leave room for the
                    // next. Only a card that fits alone ends the run.
                    if items.is_empty() {
                        self.stopped.get_or_insert(stop);
                    }
                    break;
                }
                Err(Refused::Ledger(error)) => return Err(error),
            }
            if !items.is_empty() && bytes + request.bytes() > MAX_BATCH_BYTES {
                break;
            }
            if let Err(stop) = self.budget.admit(&request, false) {
                if items.is_empty() {
                    self.stopped.get_or_insert(stop);
                }
                break;
            }
            worst = worst.plus(own);
            bytes += request.bytes();
            items.push(Item {
                id: format!("c{}", items.len()),
                card: card.name.clone(),
                request,
                answer: None,
                used: Used::default(),
            });
            queue.pop_front();
        }
        Ok(items)
    }

    /// Asks for the first answers of a piece, then for the repair round of the cards the checker rejected.
    fn first_round(&mut self, items: Vec<Item>) -> Result<(), CliError> {
        let state = self.submit(1, &items)?;
        let answers = self.collect(&state, &items)?;
        self.after_first_round(items, &answers)
    }

    /// Judges the first answers: what the checker accepts is kept, what it rejects goes to a repair batch.
    fn after_first_round(
        &mut self,
        items: Vec<Item>,
        answers: &BTreeMap<String, Answered>,
    ) -> Result<(), CliError> {
        let mut repairs: Vec<Item> = Vec::new();
        let mut worst = Usd(0);
        for mut item in items {
            let card = self.card(&item.card)?.clone();
            let answered = answers.get(&item.id);
            self.charge(&mut item.used, answered);
            let text = match answered {
                Some(Answered::Text(response)) => response.text.clone(),
                other => {
                    let why = match other {
                        Some(Answered::NoText { why, .. }) => {
                            format!("the answer holds no text: {why}")
                        }
                        Some(Answered::Failed(why)) => why.clone(),
                        _ => "the batch has no result for the request".to_owned(),
                    };
                    self.finish_failed(&card, item.used, why)?;
                    continue;
                }
            };
            let attempt = judge(self.cfg.ctx, &card, &text);
            if Outcome::of(&attempt.report).is_repairable() {
                let repair = repair_request(&item.request, &text, &attempt.report);
                let own = self.spend.worst_case(&repair);
                match self.spend.check(worst.plus(own)) {
                    Ok(()) => {}
                    Err(Refused::Cap(stop)) => {
                        self.stopped.get_or_insert(stop);
                        self.finalize(&card, &text, item.used)?;
                        continue;
                    }
                    Err(Refused::Ledger(error)) => return Err(error),
                }
                if let Err(stop) = self.budget.admit(&repair, true) {
                    self.stopped.get_or_insert(stop);
                    self.finalize(&card, &text, item.used)?;
                    continue;
                }
                worst = worst.plus(own);
                repairs.push(Item {
                    id: format!("r{}", repairs.len()),
                    card: item.card,
                    request: repair,
                    answer: Some(text),
                    used: item.used,
                });
            } else {
                self.finalize(&card, &text, item.used)?;
            }
        }
        if repairs.is_empty() {
            return self.clear_state();
        }
        let state = self.submit(2, &repairs)?;
        let answers = self.collect(&state, &repairs)?;
        self.after_repair_round(repairs, &answers)
    }

    /// Keeps the repaired answers; a card whose repair got no answer keeps its first one.
    fn after_repair_round(
        &mut self,
        repairs: Vec<Item>,
        answers: &BTreeMap<String, Answered>,
    ) -> Result<(), CliError> {
        for mut item in repairs {
            let card = self.card(&item.card)?.clone();
            let answered = answers.get(&item.id);
            self.charge(&mut item.used, answered);
            let first = item.answer.clone().unwrap_or_default();
            match answered {
                Some(Answered::Text(response)) => {
                    // The first answer's missing pieces stand when the repaired one names none.
                    let earlier = judge(self.cfg.ctx, &card, &first).missing;
                    self.finalize_with(&card, &response.text, item.used, earlier)?;
                }
                _ => self.finalize(&card, &first, item.used)?,
            }
        }
        self.clear_state()
    }

    /// Adds what one answered request cost to a card's use.
    fn charge(&mut self, used: &mut Used, answered: Option<&Answered>) {
        used.requests += 1;
        let usage = match answered {
            Some(Answered::Text(response)) => Some(response.usage),
            Some(Answered::NoText { usage, .. }) => Some(*usage),
            _ => None,
        };
        if let Some(usage) = usage {
            let cost = self.spend.price().cost(&usage, self.spend.billing());
            used.cost = used.cost.plus(cost);
        }
    }

    fn finalize(&mut self, card: &PrintedCard, text: &str, used: Used) -> Result<(), CliError> {
        self.finalize_with(card, text, used, Vec::new())
    }

    fn finalize_with(
        &mut self,
        card: &PrintedCard,
        text: &str,
        used: Used,
        earlier_missing: Vec<String>,
    ) -> Result<(), CliError> {
        let mut attempt = judge(self.cfg.ctx, card, text);
        if attempt.missing.is_empty() {
            attempt.missing = earlier_missing;
        }
        let hint = self
            .cfg
            .ctx
            .card_message(card, self.cfg.options.examples)
            .hint;
        let (outcome, detail) = keep(self.cfg.ctx, self.cfg.store, card, &attempt, hint)?;
        let finished = Finished {
            outcome,
            used,
            hint,
            missing: attempt.missing,
            detail,
        };
        self.count(card, &finished)
    }

    fn finish_failed(
        &mut self,
        card: &PrintedCard,
        used: Used,
        why: String,
    ) -> Result<(), CliError> {
        let finished = Finished {
            outcome: Outcome::Failed(why.clone()),
            used,
            hint: HintState::Off,
            missing: Vec::new(),
            detail: why,
        };
        self.count(card, &finished)
    }

    /// Counts a finished card, saves its record and prints its line.
    fn count(&mut self, card: &PrintedCard, finished: &Finished) -> Result<(), CliError> {
        self.tally.add(&finished.outcome);
        match finished.hint {
            HintState::Off => {}
            HintState::Hinted => self.tally.hinted += 1,
            HintState::Unhinted => self.tally.unhinted += 1,
        }
        self.cfg.store.save_record(&finished.record(&card.name))?;
        let why = match &finished.outcome {
            Outcome::Failed(message) => format!(": {message}"),
            _ => String::new(),
        };
        self.progress(format_args!(
            "{}: {} ({} requests){why}",
            card.name,
            finished.outcome.word(),
            finished.used.requests
        ));
        Ok(())
    }

    /// Writes the worst case of a batch to the ledger, asks for the batch and saves the state that finds it again.
    fn submit(&mut self, round: u8, items: &[Item]) -> Result<State, CliError> {
        let worst = items.iter().fold(Usd(0), |sum, item| {
            sum.plus(self.spend.worst_case(&item.request))
        });
        let params: Vec<(String, Value)> = items
            .iter()
            .map(|item| {
                (
                    item.id.clone(),
                    request_body_with(&item.request, self.cfg.api.effort()),
                )
            })
            .collect();
        let opened = open(
            self.cfg.api,
            &self.spend,
            &self.cfg.options.model,
            round,
            &params,
            worst,
            self.cfg.progress,
        )?;
        let state = State {
            model: self.cfg.options.model.clone(),
            round,
            ticket: opened.ticket,
            batch_id: opened.batch_id,
            reserved_micros: worst.0,
            entries: items
                .iter()
                .map(|item| StateEntry {
                    id: item.id.clone(),
                    card: item.card.clone(),
                    answer: item.answer.clone(),
                    requests: item.used.requests,
                    micros: item.used.cost.0,
                })
                .collect(),
        };
        self.save_state(&state)?;
        Ok(state)
    }

    /// Waits for the batch, reads its results and settles the ledger (see [`settle`]).
    fn collect(
        &mut self,
        state: &State,
        items: &[Item],
    ) -> Result<BTreeMap<String, Answered>, CliError> {
        let answers = wait(
            self.cfg.api,
            &state.batch_id,
            self.cfg.options.poll_every,
            self.cfg.progress,
        )?;
        let mut billed = Vec::new();
        for item in items {
            self.budget.release(item.request.estimated_tokens());
            if let Some(usage) = usage_of(answers.get(&item.id)) {
                self.budget.record(usage);
                billed.push((item.card.clone(), usage));
            }
        }
        let total = settle(
            &mut self.spend,
            &state.model,
            &state.ticket,
            Usd(state.reserved_micros),
            &billed,
        )?;
        self.budget.add_cost(total);
        self.progress(format_args!(
            "batch {} ended after {} s: {} billed",
            state.batch_id,
            self.started.elapsed().as_secs(),
            total
        ));
        Ok(answers)
    }

    /// Carries a batch of an earlier run through: its results, the repair round after them, and the cards left.
    fn finish_state(&mut self, state: &State) -> Result<(), CliError> {
        let mut items = Vec::new();
        for entry in &state.entries {
            let card = self.card(&entry.card)?.clone();
            let first = self.first_of(&card);
            let request = match (&entry.answer, state.round) {
                (Some(answer), 2) => {
                    let attempt = judge(self.cfg.ctx, &card, answer);
                    repair_request(&first, answer, &attempt.report)
                }
                _ => first,
            };
            items.push(Item {
                id: entry.id.clone(),
                card: entry.card.clone(),
                request,
                answer: entry.answer.clone(),
                used: Used {
                    requests: entry.requests,
                    cost: Usd(entry.micros),
                },
            });
        }
        let answers = self.collect(state, &items)?;
        if state.round == 2 {
            self.after_repair_round(items, &answers)
        } else {
            self.after_first_round(items, &answers)
        }
    }
}

/// A batch that is out: the label of its worst case in the ledger and its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The label of the ledger line that holds the batch's worst case.
    pub ticket: String,
    /// The batch id.
    pub batch_id: String,
}

/// Writes the worst case of a batch to the ledger and asks for the batch of `params` (custom id, request body). When the
/// ask fails for certain, the worst case is given back; a timeout may have left a batch behind, so it stays.
pub fn open(
    api: &BatchApi,
    spend: &Spend,
    model: &str,
    round: u8,
    params: &[(String, Value)],
    worst: Usd,
    progress: &Progress,
) -> Result<Opened, CliError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let ticket = format!("{millis}-r{round}");
    let ledger = spend.ledger().clone();
    ledger.append(&[Entry {
        card: reserved_label(&ticket),
        model: model.to_owned(),
        usage: Usage::default(),
        micros: signed(worst),
    }])?;
    progress.line(format_args!(
        "asking for a batch of {} requests (round {round}, at most {worst})",
        params.len()
    ));
    match api.create_params(params) {
        Ok(info) => Ok(Opened {
            ticket,
            batch_id: info.id,
        }),
        Err(error) => {
            if !error.may_be_billed() {
                ledger.append(&[Entry {
                    card: settled_label(&ticket),
                    model: model.to_owned(),
                    usage: Usage::default(),
                    micros: -signed(worst),
                }])?;
            }
            Err(CliError::Model(error))
        }
    }
}

/// Waits for a batch to end, looking every `poll_every`, and reads its results.
pub fn wait(
    api: &BatchApi,
    id: &str,
    poll_every: std::time::Duration,
    progress: &Progress,
) -> Result<BTreeMap<String, Answered>, CliError> {
    let mut info = api.status(id).map_err(CliError::Model)?;
    while !info.ended {
        progress.line(format_args!(
            "batch {}: {} processing, {} succeeded, {} errored, {} canceled, {} expired",
            info.id, info.processing, info.succeeded, info.errored, info.canceled, info.expired
        ));
        thread::sleep(poll_every);
        info = api.status(id).map_err(CliError::Model)?;
    }
    api.results(id).map_err(CliError::Model)
}

/// The tokens a batch result was billed for; none for a request that errored, expired or was canceled.
pub fn usage_of(answered: Option<&Answered>) -> Option<Usage> {
    match answered {
        Some(Answered::Text(response)) => Some(response.usage),
        Some(Answered::NoText { usage, .. }) => Some(*usage),
        _ => None,
    }
}

/// Settles a batch into the ledger: each billed request (card, usage) at its cost, and the batch's worst case given
/// back. A batch settled before (a run that stopped after this step) is not billed twice. Returns what it cost.
pub fn settle(
    spend: &mut Spend,
    model: &str,
    ticket: &str,
    reserved: Usd,
    billed: &[(String, Usage)],
) -> Result<Usd, CliError> {
    let ledger = spend.ledger().clone();
    let settled = settled_label(ticket);
    let already = ledger.has_label(&settled)?;
    let mut rows = Vec::new();
    let mut total = Usd(0);
    for (card, usage) in billed {
        let cost = spend.price().cost(usage, spend.billing());
        total = total.plus(cost);
        rows.push(Entry {
            card: card.clone(),
            model: model.to_owned(),
            usage: *usage,
            micros: signed(cost),
        });
    }
    if !already {
        rows.push(Entry {
            card: settled,
            model: model.to_owned(),
            usage: Usage::default(),
            micros: -signed(reserved),
        });
        ledger.append(&rows)?;
        spend.add_run(total);
    }
    Ok(total)
}
