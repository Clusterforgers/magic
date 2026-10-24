//! The loop of a run: for each card ask the model, check the answer, send one repair round when the checker rejects it,
//! and keep what is accepted.

use std::collections::BTreeSet;

use mtg_cards::load::Library;
use mtg_cards::proposal::{Report, check};
use mtg_cards::{Caps, PrintedCard};

use super::budget::{Budget, Stop};
use super::examples::ExampleIndex;
use super::model::{ModelClient, ModelError, Request, Response};
use super::outcome::{Outcome, ProposeReport, Tally};
use super::prompt::{card_prompt, extract_json, first_request, repair_request};
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
}

/// The settings of a run.
#[derive(Debug, Clone)]
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
}

/// What the model said and the checker's report of it.
struct Attempt {
    json: String,
    report: Report,
}

/// Why a request got no answer.
enum Asked {
    /// The budget refused; the session remembers which.
    Budget,
    Failed(ModelError),
}

/// One run over a list of cards.
pub struct Session<'a> {
    ctx: &'a Context<'a>,
    client: &'a dyn ModelClient,
    options: &'a Options,
    store: &'a Store,
    progress: &'a Progress,
    budget: Budget,
    stopped: Option<Stop>,
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
            budget: Budget::new(options.max_requests, options.max_input_tokens),
            stopped: None,
        }
    }

    /// Asks about each card in turn until the cards or the budget are spent.
    pub fn run(mut self, cards: &[&PrintedCard]) -> Result<ProposeReport, CliError> {
        let mut tally = Tally::default();
        let mut failures = 0;
        for card in cards {
            if self.options.resume && self.store.has(&card.name) {
                tally.skipped += 1;
                continue;
            }
            let before = self.budget.requests();
            let outcome = if self.stopped.is_some() {
                Outcome::NotRun
            } else {
                self.card(card)?
            };
            if let Outcome::Failed(message) = &outcome {
                failures += 1;
                if failures >= FAILURES_IN_A_ROW {
                    return Err(CliError::Config(format!(
                        "{failures} cards in a row got no answer; the last: {message}"
                    )));
                }
            } else {
                failures = 0;
            }
            tally.add(&outcome);
            if outcome != Outcome::NotRun {
                self.progress.line(format_args!(
                    "{}: {} ({} requests)",
                    card.name,
                    outcome.word(),
                    self.budget.requests() - before
                ));
            }
        }
        Ok(ProposeReport {
            model: self.options.model.clone(),
            tally,
            budget: self.budget,
            stopped: self.stopped,
            out: self.store.dir().to_path_buf(),
        })
    }

    /// One card: ask, check, and repair once when the checker rejects the answer.
    fn card(&mut self, printed: &PrintedCard) -> Result<Outcome, CliError> {
        let near = self.ctx.examples.nearest(printed, self.options.examples);
        let first = first_request(
            &self.options.model,
            &self.ctx.system,
            card_prompt(printed, &near),
        );
        let answer = match self.ask(&first, false) {
            Ok(answer) => answer,
            Err(Asked::Budget) => return Ok(Outcome::NotRun),
            Err(Asked::Failed(error)) => return unanswered(error),
        };
        let mut attempt = self.judge(printed, &answer.text);
        if Outcome::of(&attempt.report).is_repairable() {
            let repair = repair_request(&first, &answer.text, &attempt.report);
            match self.ask(&repair, true) {
                Ok(second) => attempt = self.judge(printed, &second.text),
                // The first answer stands as it is; the run ends.
                Err(Asked::Budget | Asked::Failed(_)) => {}
            }
        }
        self.keep(printed, &attempt)
    }

    /// Sends a request when the budget allows it.
    fn ask(&mut self, request: &Request, repair: bool) -> Result<Response, Asked> {
        if let Err(stop) = self.budget.admit(request, repair) {
            self.stopped = Some(stop);
            return Err(Asked::Budget);
        }
        let response = self.client.complete(request).map_err(Asked::Failed)?;
        self.budget.record(response.usage);
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

    /// Saves the attempt as an accepted proposal, or as a rejected one with the report.
    fn keep(&self, printed: &PrintedCard, attempt: &Attempt) -> Result<Outcome, CliError> {
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
