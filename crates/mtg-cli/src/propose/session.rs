//! The loop of a run: for each card ask the model, check the answer, send one repair round when the checker rejects it,
//! and keep what is accepted. `--jobs` cards are in flight at once; they share one budget and one set of stop rules.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
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
use super::outcome::{Outcome, ProposeReport, Tally};
use super::prompt::{extract_json, first_request, hinted_card_prompt, repair_request};
use super::status::status_line;
use super::store::Store;
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
}

/// What the model said and the checker's report of it.
struct Attempt {
    json: String,
    report: Report,
}

/// Why a request got no answer.
enum Asked {
    /// The budget refused; the shared state remembers which.
    Budget,
    Failed(ModelError),
}

/// How a card ended and how many requests it took.
struct Finished {
    outcome: Outcome,
    requests: u32,
    hint: HintState,
}

/// What the workers of a run share, behind one lock: the budget and the stop rules are checked and updated together.
struct Shared {
    budget: Budget,
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
                stopped: None,
                tally: Tally::default(),
                failures: 0,
                error: None,
            }),
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
        let shared = self
            .shared
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        match shared.error {
            Some(error) => Err(error),
            None if panicked => Err(CliError::Config("a worker of the run stopped".into())),
            None => Ok(ProposeReport {
                model: self.options.model.clone(),
                tally: shared.tally,
                budget: shared.budget,
                stopped: shared.stopped,
                out: self.store.dir().to_path_buf(),
            }),
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
                Ok(Finished {
                    outcome: Outcome::NotRun,
                    requests: 0,
                    hint: HintState::Off,
                })
            } else {
                self.card(card)
            };
            match finished {
                Ok(finished) => self.finish(card, &finished),
                Err(error) => {
                    let mut state = self.state();
                    if state.error.is_none() {
                        state.error = Some(error);
                    }
                }
            }
        }
    }

    /// Counts a finished card, ends the run after too many failures in a row, and prints the card's line.
    fn finish(&self, card: &PrintedCard, finished: &Finished) {
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
        if finished.outcome != Outcome::NotRun {
            let why = match &finished.outcome {
                Outcome::Failed(message) => format!(": {message}"),
                _ => String::new(),
            };
            self.progress.line(format_args!(
                "{}: {} ({} requests){why}",
                card.name,
                finished.outcome.word(),
                finished.requests
            ));
        }
    }

    /// One card: ask, check, and repair once when the checker rejects the answer.
    fn card(&self, printed: &PrintedCard) -> Result<Finished, CliError> {
        let mut requests = 0;
        let message = self.ctx.card_message(printed, self.options.examples);
        let hint = message.hint;
        let first = first_request(&self.options.model, &self.ctx.system, message.text);
        let answer = match self.ask(&first, false, &mut requests) {
            Ok(answer) => answer,
            Err(Asked::Budget) => {
                return Ok(Finished {
                    outcome: Outcome::NotRun,
                    requests,
                    hint: HintState::Off,
                });
            }
            Err(Asked::Failed(error)) => {
                return Ok(Finished {
                    outcome: unanswered(error)?,
                    requests,
                    hint,
                });
            }
        };
        let mut attempt = self.judge(printed, &answer.text);
        if Outcome::of(&attempt.report).is_repairable() {
            let repair = repair_request(&first, &answer.text, &attempt.report);
            match self.ask(&repair, true, &mut requests) {
                Ok(second) => attempt = self.judge(printed, &second.text),
                // The first answer stands as it is.
                Err(Asked::Budget | Asked::Failed(_)) => {}
            }
        }
        Ok(Finished {
            outcome: self.keep(printed, &attempt, hint)?,
            requests,
            hint,
        })
    }

    /// Sends a request when the budget allows it; the tokens it may cost stay reserved while it is out.
    fn ask(&self, request: &Request, repair: bool, requests: &mut u32) -> Result<Response, Asked> {
        {
            let mut state = self.state();
            if let Err(stop) = state.budget.admit(request, repair) {
                state.stopped.get_or_insert(stop);
                return Err(Asked::Budget);
            }
        }
        *requests += 1;
        let result = self.client.complete(request);
        let mut state = self.state();
        state.budget.release(request.estimated_tokens());
        let response = result.map_err(Asked::Failed)?;
        state.budget.record(response.usage);
        Ok(response)
    }

    /// Checks what the model wrote against the card.
    fn judge(&self, printed: &PrintedCard, answer: &str) -> Attempt {
        let json = extract_json(answer).to_owned();
        let report = check(
            &json,
            printed,
            self.ctx.library,
            &self.ctx.names,
            &self.ctx.caps,
        );
        Attempt { json, report }
    }

    /// Saves the attempt as an accepted proposal, or as a rejected one with the report, and the hint modes of the run when
    /// it used any.
    fn keep(
        &self,
        printed: &PrintedCard,
        attempt: &Attempt,
        hint: HintState,
    ) -> Result<Outcome, CliError> {
        let outcome = Outcome::of(&attempt.report);
        match (&attempt.report.rules, outcome.is_kept()) {
            (Some(rules), true) => {
                self.store.save_accepted(&printed.name, rules)?;
            }
            _ => self.store.save_rejected(
                &printed.name,
                &attempt.json,
                &attempt.report.render(false),
            )?,
        }
        let record = self.ctx.hint_record(hint);
        if !record.is_plain() {
            self.store.save_hints(&printed.name, &record)?;
        }
        Ok(outcome)
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
