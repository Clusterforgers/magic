//! The loop of a run: for each card ask the model, check the answer, send one repair round when the checker rejects it,
//! and keep what is accepted. `--jobs` cards are in flight at once; they share one budget and one set of stop rules.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Report, check};
use mtg_cards::{Caps, PrintedCard};

use super::budget::{Budget, Stop};
use super::examples::ExampleIndex;
use super::forge_hint::{ForgeHints, HintRecord, HintState};
use super::model::{ModelClient, ModelError, Request, Response};
use super::outcome::{Outcome, ProposeReport, SpendReport, Tally};
use super::price::Usd;
use super::prompt::{
    diagnosis, extract_json, first_request, hinted_card_prompt, repair_request, take_missing,
};
use super::spend::{Refused, Spend};
use super::status::status_line;
use super::store::Store;
use super::summary::CardRecord;
use crate::error::CliError;
use crate::progress::Progress;

/// Cards in a row that get no answer before the run gives up: an outage costs nothing more than this.
const FAILURES_IN_A_ROW: usize = 5;

/// What the checker needs and what every prompt shares.
pub struct Context<'a> {
    /// The hand-written rules: the predefined tokens for the checker and the examples.
    pub library: &'a Library,
    /// The lowercase names of the stored cards, which a rule may name.
    pub names: BTreeSet<String>,
    /// What the engine runs.
    pub caps: Caps,
    /// The system prompt: the instructions and the DSL reference.
    pub system: String,
    /// The hand-written cards, for the examples.
    pub examples: ExampleIndex,
    /// How many solved examples each prompt holds (`--solved-examples`); 0 means the hand-written ones.
    pub solved_examples: usize,
    /// The scripts of Forge (`--forge-hint`), when the run hints with them.
    pub forge: Option<ForgeHints>,
}

/// The first message about a card and how it was made.
pub struct CardMessage {
    /// The message text.
    pub text: String,
    /// How many examples it holds.
    pub examples: usize,
    /// Whether it holds a Forge hint.
    pub hint: HintState,
}

impl Context<'_> {
    /// The first message about `card`: its text, the Forge hint of the run when the card has one, and `examples` nearest
    /// examples (the run's solved count replaces it).
    pub fn card_message(&self, card: &PrintedCard, examples: usize) -> CardMessage {
        let near = self.examples.nearest(card, examples);
        let hint = self.forge.as_ref().map(|forge| forge.hint(card));
        let state = match &hint {
            None => HintState::Off,
            Some(Some(_)) => HintState::Hinted,
            Some(None) => HintState::Unhinted,
        };
        CardMessage {
            text: hinted_card_prompt(card, &near, hint.flatten().as_deref()),
            examples: near.len(),
            hint: state,
        }
    }

    /// The hint modes the run used for a card in this state.
    pub fn hint_record(&self, hint: HintState) -> HintRecord {
        HintRecord {
            solved_examples: self.solved_examples,
            forge_hint: hint,
        }
    }
}

/// The settings of a run.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The model id.
    pub model: String,
    /// How many examples each prompt holds.
    pub examples: usize,
    /// Skip the cards that have a saved proposal.
    pub resume: bool,
    /// The most requests.
    pub max_requests: u32,
    /// The most input tokens.
    pub max_input_tokens: u64,
    /// How many cards are worked on at the same time (at least 1).
    pub jobs: u32,
    /// The time between two status lines while the run goes on; zero prints only the last one.
    pub status_every: Duration,
    /// The spend cap and the cost ledger of a paid run; none for a local model.
    pub spend: Option<Spend>,
    /// Sends the first request alone, so that the others read the shared rules from the API's cache and do not each write
    /// it (a cache entry exists only once the first request has been answered).
    pub warm_cache: bool,
    /// With `--batch`: the time between two looks at a batch that is out.
    pub poll_every: Duration,
    /// With `--batch`: the most requests in one batch; zero takes the default.
    pub batch_size: usize,
}

/// What the model said and the checker's report of it.
pub(super) struct Attempt {
    pub(super) json: String,
    pub(super) report: Report,
    /// The engine pieces the model says are missing.
    pub(super) missing: Vec<String>,
}

/// Why a request got no answer.
enum Asked {
    /// The budget refused; the shared state remembers which.
    Budget,
    Failed(ModelError),
    /// The ledger of the spend cap cannot be kept, which ends the run.
    Ledger(CliError),
}

/// What a card has cost so far.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Used {
    pub(super) requests: u32,
    pub(super) cost: Usd,
}

/// How a card ended.
pub(super) struct Finished {
    pub(super) outcome: Outcome,
    pub(super) used: Used,
    pub(super) hint: HintState,
    pub(super) missing: Vec<String>,
    pub(super) detail: String,
}

impl Finished {
    /// A card the budget kept from running.
    fn not_run(used: Used) -> Self {
        Self {
            outcome: Outcome::NotRun,
            used,
            hint: HintState::Off,
            missing: Vec::new(),
            detail: String::new(),
        }
    }

    /// The card's record for the summary.
    pub(super) fn record(&self, card: &str) -> CardRecord {
        CardRecord {
            card: card.to_owned(),
            outcome: self.outcome.key().to_owned(),
            missing_engine_piece: self.missing.clone(),
            requests: self.used.requests,
            micros: self.used.cost.0,
            detail: self.detail.clone(),
            agree: None,
        }
    }
}

/// What the workers of a run share, behind one lock: the budget and the stop rules are checked and updated together.
struct Shared {
    budget: Budget,
    spend: Option<Spend>,
    stopped: Option<Stop>,
    tally: Tally,
    /// Cards in a row, in the order they finished, that got no answer.
    failures: usize,
    /// The error that ends the run; the first one stays.
    error: Option<CliError>,
}

/// One run over a list of cards: `jobs` workers take the cards in order, and the rounds of one card stay sequential.
pub struct Session<'a> {
    ctx: &'a Context<'a>,
    client: &'a dyn ModelClient,
    options: &'a Options,
    store: &'a Store,
    progress: &'a Progress,
    shared: Mutex<Shared>,
    /// Whether a request has been answered, so the API holds the shared rules in its cache.
    warmed: AtomicBool,
    /// Held by the first request, so that the others wait for the cache entry instead of each writing it.
    gate: Mutex<()>,
}

impl<'a> Session<'a> {
    /// A session with the budget of the options.
    pub fn new(
        ctx: &'a Context<'a>,
        client: &'a dyn ModelClient,
        options: &'a Options,
        store: &'a Store,
        progress: &'a Progress,
    ) -> Self {
        Self {
            ctx,
            client,
            options,
            store,
            progress,
            shared: Mutex::new(Shared {
                budget: Budget::new(options.max_requests, options.max_input_tokens),
                spend: options.spend.clone(),
                stopped: None,
                tally: Tally::default(),
                failures: 0,
                error: None,
            }),
            warmed: AtomicBool::new(false),
            gate: Mutex::new(()),
        }
    }

    /// Asks about the cards with `options.jobs` workers until the cards or the budget are spent, printing a status line
    /// every `options.status_every` and at the end.
    pub fn run(self, cards: &[&PrintedCard]) -> Result<ProposeReport, CliError> {
        let (work, skipped): (Vec<&PrintedCard>, Vec<&PrintedCard>) = cards
            .iter()
            .partition(|card| !(self.options.resume && self.store.has(&card.name)));
        self.state().tally.skipped = skipped.len();
        let total = work.len();
        let jobs = usize::try_from(self.options.jobs)
            .unwrap_or(1)
            .clamp(1, total.max(1));
        let started = Instant::now();
        let next = AtomicUsize::new(0);
        let (this, work_ref, next_ref) = (&self, &work, &next);
        let (finish, ticks) = mpsc::channel::<()>();
        let panicked = thread::scope(|scope| {
            let ticker = scope.spawn(move || this.tick(&ticks, total, started));
            let workers: Vec<_> = (0..jobs)
                .map(|_| scope.spawn(move || this.work(work_ref, next_ref)))
                .collect();
            let ended: Vec<bool> = workers
                .into_iter()
                .map(|worker| worker.join().is_err())
                .collect();
            drop(finish);
            ticker.join().is_err() || ended.contains(&true)
        });
        self.progress.line(self.status(total, started.elapsed()));
        let summary = self.store.write_summary();
        let shared = self
            .shared
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        match shared.error {
            Some(error) => Err(error),
            None if panicked => Err(CliError::Config("a worker of the run stopped".into())),
            None => {
                summary?;
                Ok(ProposeReport {
                    model: self.options.model.clone(),
                    tally: shared.tally,
                    budget: shared.budget,
                    stopped: shared.stopped,
                    out: self.store.dir().to_path_buf(),
                    spend: shared.spend.as_ref().map(spend_report),
                })
            }
        }
    }

    fn state(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The status line of the run so far.
    fn status(&self, total: usize, elapsed: Duration) -> String {
        let state = self.state();
        status_line(&state.tally, &state.budget, total, elapsed)
    }

    /// Prints the status line every `status_every` until the workers are done (the sender is dropped); zero prints none.
    fn tick(&self, ticks: &mpsc::Receiver<()>, total: usize, started: Instant) {
        let every = self.options.status_every;
        if every.is_zero() {
            return;
        }
        while let Err(RecvTimeoutError::Timeout) = ticks.recv_timeout(every) {
            self.progress.line(self.status(total, started.elapsed()));
        }
    }

    /// One worker: takes the next card until none is left or the run has an error.
    fn work(&self, cards: &[&PrintedCard], next: &AtomicUsize) {
        while self.state().error.is_none() {
            let Some(card) = cards.get(next.fetch_add(1, Ordering::SeqCst)) else {
                return;
            };
            let finished = if self.state().stopped.is_some() {
                Ok(Finished::not_run(Used::default()))
            } else {
                self.card(card)
            };
            let saved = finished.and_then(|finished| self.finish(card, &finished));
            if let Err(error) = saved {
                let mut state = self.state();
                if state.error.is_none() {
                    state.error = Some(error);
                }
            }
        }
    }

    /// Counts a finished card, ends the run after too many failures in a row, saves the card's record and prints its line.
    fn finish(&self, card: &PrintedCard, finished: &Finished) -> Result<(), CliError> {
        let mut state = self.state();
        if let Outcome::Failed(message) = &finished.outcome {
            state.failures += 1;
            if state.failures >= FAILURES_IN_A_ROW && state.error.is_none() {
                state.error = Some(CliError::Config(format!(
                    "{} cards in a row got no answer; the last: {message}",
                    state.failures
                )));
            }
        } else {
            state.failures = 0;
        }
        state.tally.add(&finished.outcome);
        match finished.hint {
            HintState::Off => {}
            HintState::Hinted => state.tally.hinted += 1,
            HintState::Unhinted => state.tally.unhinted += 1,
        }
        drop(state);
        if finished.outcome == Outcome::NotRun {
            return Ok(());
        }
        self.store.save_record(&finished.record(&card.name))?;
        let why = match &finished.outcome {
            Outcome::Failed(message) => format!(": {message}"),
            _ => String::new(),
        };
        self.progress.line(format_args!(
            "{}: {} ({} requests){why}",
            card.name,
            finished.outcome.word(),
            finished.used.requests
        ));
        Ok(())
    }

    /// One card: ask, check, and repair once when the checker rejects the answer.
    fn card(&self, printed: &PrintedCard) -> Result<Finished, CliError> {
        let mut used = Used::default();
        let message = self.ctx.card_message(printed, self.options.examples);
        let hint = message.hint;
        let first = first_request(&self.options.model, &self.ctx.system, message.text);
        let answer = match self.ask(&first, false, &printed.name, &mut used) {
            Ok(answer) => answer,
            Err(Asked::Budget) => return Ok(Finished::not_run(used)),
            Err(Asked::Ledger(error)) => return Err(error),
            Err(Asked::Failed(error)) => {
                let detail = error.to_string();
                return Ok(Finished {
                    outcome: unanswered(error)?,
                    used,
                    hint,
                    missing: Vec::new(),
                    detail,
                });
            }
        };
        let mut attempt = judge(self.ctx, printed, &answer.text);
        if Outcome::of(&attempt.report).is_repairable() {
            let repair = repair_request(&first, &answer.text, &attempt.report);
            match self.ask(&repair, true, &printed.name, &mut used) {
                Ok(second) => {
                    let mut next = judge(self.ctx, printed, &second.text);
                    if next.missing.is_empty() {
                        next.missing = std::mem::take(&mut attempt.missing);
                    }
                    attempt = next;
                }
                Err(Asked::Ledger(error)) => return Err(error),
                // The first answer stands as it is.
                Err(Asked::Budget | Asked::Failed(_)) => {}
            }
        }
        let (outcome, detail) = keep(self.ctx, self.store, printed, &attempt, hint)?;
        Ok(Finished {
            outcome,
            used,
            hint,
            missing: attempt.missing,
            detail,
        })
    }

    /// Sends a request when the budget and the spend cap allow it. The tokens and the money it may cost stay held while it
    /// is out; what it really cost goes into the ledger, whether or not the answer is usable.
    fn ask(
        &self,
        request: &Request,
        repair: bool,
        card: &str,
        used: &mut Used,
    ) -> Result<Response, Asked> {
        let held = {
            let mut state = self.state();
            let held = match state.spend.as_mut().map(|spend| spend.reserve(request)) {
                None => None,
                Some(Ok(held)) => Some(held),
                Some(Err(Refused::Cap(stop))) => {
                    state.stopped.get_or_insert(stop);
                    return Err(Asked::Budget);
                }
                Some(Err(Refused::Ledger(error))) => return Err(Asked::Ledger(error)),
            };
            if let Err(stop) = state.budget.admit(request, repair) {
                if let (Some(spend), Some(held)) = (state.spend.as_mut(), held) {
                    spend.release(held);
                }
                state.stopped.get_or_insert(stop);
                return Err(Asked::Budget);
            }
            held
        };
        used.requests += 1;
        let result = self.complete(request);
        let mut state = self.state();
        state.budget.release(request.estimated_tokens());
        let usage = match &result {
            Ok(response) => Some(response.usage),
            Err(error) => error.billed(),
        };
        if let Some(usage) = usage {
            state.budget.record(usage);
        }
        if let (Some(spend), Some(held)) = (state.spend.as_mut(), held) {
            let billed = match (&result, usage) {
                (_, Some(usage)) => spend.bill(held, card, usage),
                (Err(error), None) if error.may_be_billed() => spend.forfeit(held, card),
                _ => {
                    spend.release(held);
                    Ok(Usd(0))
                }
            };
            let cost = billed.map_err(Asked::Ledger)?;
            used.cost = used.cost.plus(cost);
            state.budget.add_cost(cost);
        }
        result.map_err(Asked::Failed)
    }

    /// Sends the request; with `warm_cache` the first one goes alone, and the others wait for it.
    fn complete(&self, request: &Request) -> Result<Response, ModelError> {
        if !self.options.warm_cache || self.warmed.load(Ordering::Acquire) {
            return self.client.complete(request);
        }
        let gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if self.warmed.load(Ordering::Acquire) {
            drop(gate);
            return self.client.complete(request);
        }
        let result = self.client.complete(request);
        if result.is_ok() {
            self.warmed.store(true, Ordering::Release);
        }
        drop(gate);
        result
    }
}

/// What a run spent, from its spend state.
pub(super) fn spend_report(spend: &Spend) -> SpendReport {
    SpendReport {
        run: spend.run(),
        ledger: spend.ledger_total().unwrap_or_default(),
        cap: spend.cap(),
    }
}

/// Checks what the model wrote against the card: its `missing` field is taken out first, since the checker reads rules.
pub(super) fn judge(ctx: &Context<'_>, printed: &PrintedCard, answer: &str) -> Attempt {
    let (json, missing) = take_missing(extract_json(answer));
    let report = check(&json, printed, ctx.library, &ctx.names, &ctx.caps);
    Attempt {
        json,
        report,
        missing,
    }
}

/// Saves the attempt as an accepted proposal, as a draft (valid, with a construct that has no writer), or as a rejected
/// one with the report, and the hint modes of the run when it used any. Returns the outcome and the first thing worth
/// knowing about it.
pub(super) fn keep(
    ctx: &Context<'_>,
    store: &Store,
    printed: &PrintedCard,
    attempt: &Attempt,
    hint: HintState,
) -> Result<(Outcome, String), CliError> {
    let outcome = Outcome::of(&attempt.report);
    match (&attempt.report.rules, &outcome) {
        (Some(rules), kept) if kept.is_kept() => {
            store.save_accepted(&printed.name, rules)?;
        }
        (Some(rules), draft) if draft.is_draft() => {
            store.save_draft(&printed.name, rules)?;
        }
        _ => store.save_rejected(&printed.name, &attempt.json, &attempt.report.render(false))?,
    }
    let record = ctx.hint_record(hint);
    if !record.is_plain() {
        store.save_hints(&printed.name, &record)?;
    }
    Ok((outcome.clone(), detail_of(&outcome, &attempt.report)))
}

/// The first thing the checker found wrong, the constructs without a writer, or a word about an empty proposal.
pub(super) fn detail_of(outcome: &Outcome, report: &Report) -> String {
    match outcome {
        Outcome::Invalid | Outcome::Mismatch => diagnosis(report)
            .lines()
            .map(|line| line.trim_start_matches("- "))
            .collect::<Vec<_>>()
            .join(" / "),
        Outcome::Unchecked => format!(
            "no writer for: {}",
            report.unwritten().collect::<Vec<_>>().join(", ")
        ),
        Outcome::Empty => "accepted, but nothing is modelled".to_owned(),
        Outcome::Verified | Outcome::Partial | Outcome::Failed(_) | Outcome::NotRun => {
            String::new()
        }
    }
}

/// A card whose request failed: the run ends on an error no card can recover from, else the card gets no proposal.
fn unanswered(error: ModelError) -> Result<Outcome, CliError> {
    if error.is_fatal() {
        Err(CliError::Model(error))
    } else {
        Ok(Outcome::Failed(error.to_string()))
    }
}
