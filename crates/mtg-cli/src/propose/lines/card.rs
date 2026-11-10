//! One card of line mode as a unit of work: the conversations of its gapped lines, the choice among sampled answers, the
//! repair rounds fed with the checker's words, and how the result is kept. The run and the batch run share it.

use std::path::Path;

use mtg_cards::PrintedCard;
use mtg_cards::load::slug;
use mtg_cards::proposal::{Report, check};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::part::{self, Reply};
use super::pool::{Gap, GapAt, Translation, merge_all};
use super::{LineCtx, write};
use crate::error::CliError;
use crate::propose::model::{Message, Request};
use crate::propose::outcome::Outcome;
use crate::propose::prompt::diagnosis;
use crate::propose::session::detail_of;
use crate::propose::store::Store;
use crate::propose::summary::CardRecord;

/// The most repair rounds a card gets after its first answers.
pub const LINE_REPAIRS: u8 = 2;

/// A card ready to ask about: its translation, its gaps and the first request of each.
pub struct Prepared<'a> {
    /// The card.
    pub card: &'a PrintedCard,
    /// The translator's proposal and ledger.
    pub translation: Translation,
    /// Each gapped line: where it goes and what the model gets.
    pub gaps: Vec<(GapAt, Gap)>,
    /// The first request of each gapped line.
    pub requests: Vec<Request>,
}

/// Where a card is: the round, what is asked in it, the answers that stand and what was spent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work {
    /// The card's name.
    pub name: String,
    /// 0 for the first answers, then the repair rounds.
    pub round: u8,
    /// For each line, the requests of this round: the samples in the first round, one in a repair, none for a line
    /// whose answer stands.
    pub asking: Vec<Vec<Request>>,
    /// For each line, the conversation its standing answer came from.
    pub last: Vec<Request>,
    /// For each line, the answer that stands.
    pub standing: Vec<Option<String>>,
    /// With several samples: whether the first answers of every line said the same.
    pub agree: Option<bool>,
    /// Requests sent.
    pub requests: u32,
    /// What they cost in micro-dollars.
    pub micros: u64,
}

impl Work {
    /// The first round of a card: each line asked `samples` times.
    pub fn new(prepared: &Prepared<'_>, samples: usize) -> Work {
        Work {
            name: prepared.card.name.clone(),
            round: 0,
            asking: prepared
                .requests
                .iter()
                .map(|request| vec![request.clone(); samples.max(1)])
                .collect(),
            last: prepared.requests.clone(),
            standing: vec![None; prepared.requests.len()],
            agree: None,
            requests: 0,
            micros: 0,
        }
    }

    /// The places of this round's requests as (line, sample).
    pub fn slots(&self) -> Vec<(usize, usize)> {
        self.pending()
            .into_iter()
            .map(|(line, sample, _)| (line, sample))
            .collect()
    }

    /// The requests of this round as (line, sample, request).
    pub fn pending(&self) -> Vec<(usize, usize, &Request)> {
        self.asking
            .iter()
            .enumerate()
            .flat_map(|(line, requests)| {
                requests
                    .iter()
                    .enumerate()
                    .map(move |(sample, request)| (line, sample, request))
            })
            .collect()
    }
}

/// What a card's answers came to.
#[derive(Debug)]
pub enum Attempt {
    /// Every line is an ability; the merged card and the checker's report.
    Judged {
        /// The merged proposal.
        json: String,
        /// The checker's report of it.
        report: Box<Report>,
    },
    /// A line is not an ability of the rules.
    Bad {
        /// What is wrong, line by line.
        problems: String,
        /// The answers.
        answers: String,
    },
    /// The model says the rules cannot say a line; the note names the engine piece.
    Missing(String),
    /// No answer came for a line.
    NoAnswer(String),
}

/// What comes after a round.
#[derive(Debug)]
pub enum Step {
    /// The card is asked again: a repair round is in `Work::asking`.
    Again,
    /// The card is done.
    Done(Attempt),
}

/// A sample judged: its rank, its answer for each line, what it came to and the problem of each line.
type Candidate = (u8, Vec<String>, Attempt, Vec<Option<String>>);

/// The rank of an attempt: lower is better.
fn rank(attempt: &Attempt) -> u8 {
    match attempt {
        Attempt::Judged { report, .. } => match Outcome::of(report) {
            Outcome::Verified => 0,
            Outcome::Partial => 1,
            Outcome::Unchecked => 2,
            Outcome::Empty => 3,
            Outcome::Mismatch => 4,
            _ => 5,
        },
        Attempt::Missing(_) => 6,
        Attempt::Bad { .. } => 7,
        Attempt::NoAnswer(_) => 8,
    }
}

/// An answer as canonical JSON (an ability or a `missing` note), for comparing samples; none when it is neither.
fn canonical(answer: &str) -> Option<Value> {
    match part::read_reply(answer) {
        Reply::Ability(value) => Some(value),
        Reply::Missing(note) => Some(json!({"missing": note})),
        Reply::Bad(_) => None,
    }
}

impl LineCtx<'_> {
    /// Judges one answer for each line: the merged card through the checker, with the problem of each line that is not an
    /// ability.
    fn evaluate(
        &self,
        prepared: &Prepared<'_>,
        answers: &[String],
    ) -> (Attempt, Vec<Option<String>>) {
        let replies: Vec<Reply> = answers
            .iter()
            .map(|answer| part::read_reply(answer))
            .collect();
        let missing: Vec<&str> = replies
            .iter()
            .filter_map(|reply| match reply {
                Reply::Missing(note) => Some(note.as_str()),
                _ => None,
            })
            .collect();
        if !missing.is_empty() {
            let none = vec![None; answers.len()];
            return (Attempt::Missing(missing.join(" | ")), none);
        }
        let problems: Vec<Option<String>> = replies
            .iter()
            .map(|reply| match reply {
                Reply::Bad(problem) => Some(problem.clone()),
                _ => None,
            })
            .collect();
        if problems.iter().any(Option::is_some) {
            let text = problems
                .iter()
                .enumerate()
                .filter_map(|(line, problem)| {
                    problem.as_ref().map(|p| format!("line {}: {p}", line + 1))
                })
                .collect::<Vec<_>>()
                .join(" / ");
            let attempt = Attempt::Bad {
                problems: text,
                answers: answers.join("\n"),
            };
            return (attempt, problems);
        }
        let parts = replies
            .into_iter()
            .zip(&prepared.gaps)
            .filter_map(|(reply, (at, _))| match reply {
                Reply::Ability(value) => Some((at.face, at.at, value)),
                _ => None,
            })
            .collect();
        let Some(merged) = merge_all(&prepared.translation.proposal, parts) else {
            let problem = "the translation has no place for the ability".to_owned();
            let attempt = Attempt::Bad {
                problems: problem.clone(),
                answers: answers.join("\n"),
            };
            return (attempt, vec![Some(problem); answers.len()]);
        };
        let json = merged.to_string();
        let report = check(&json, prepared.card, self.library, self.names, self.caps);
        let attempt = Attempt::Judged {
            json,
            report: Box::new(report),
        };
        (attempt, vec![None; answers.len()])
    }

    /// What the answers that stand come to, for a card whose repair round is not asked (the spend cap ran out).
    pub fn standing(&self, prepared: &Prepared<'_>, work: &Work) -> Attempt {
        match work
            .standing
            .iter()
            .cloned()
            .collect::<Option<Vec<String>>>()
        {
            Some(answers) => self.evaluate(prepared, &answers).0,
            None => Attempt::NoAnswer("no answer".into()),
        }
    }

    /// Takes the answers of a round (for each line, one per request of `work.asking`; none for a request that got no
    /// answer): picks the best sample, records whether the samples agree, and sets up a repair round when the checker
    /// rejects the card and rounds are left. `failure` is why the last request without an answer got none.
    pub fn step(
        &self,
        prepared: &Prepared<'_>,
        work: &mut Work,
        answers: &[Vec<Option<String>>],
        failure: Option<String>,
    ) -> Step {
        let lines = prepared.gaps.len();
        let samples = answers.iter().map(Vec::len).max().unwrap_or(0).max(1);
        if work.round == 0 && samples > 1 {
            work.agree = Some(answers.iter().all(|line| {
                let seen: Vec<Option<Value>> = line
                    .iter()
                    .map(|answer| answer.as_deref().and_then(canonical))
                    .collect();
                seen.iter().all(Option::is_some) && seen.windows(2).all(|pair| pair[0] == pair[1])
            }));
        }
        let mut best: Option<Candidate> = None;
        for sample in 0..samples {
            let chosen: Option<Vec<String>> = (0..lines)
                .map(|line| match answers.get(line) {
                    Some(asked) if !asked.is_empty() => asked.get(sample).cloned().flatten(),
                    _ => work.standing.get(line).cloned().flatten(),
                })
                .collect();
            let Some(chosen) = chosen else { continue };
            let (attempt, problems) = self.evaluate(prepared, &chosen);
            let score = rank(&attempt);
            if best.as_ref().is_none_or(|(old, ..)| score < *old) {
                best = Some((score, chosen, attempt, problems));
            }
        }
        let Some((score, chosen, attempt, problems)) = best else {
            // A repair round that got no answer leaves the answers that stood before it.
            let standing: Option<Vec<String>> = work.standing.iter().cloned().collect();
            if let (true, Some(standing)) = (work.round > 0, standing) {
                return Step::Done(self.evaluate(prepared, &standing).0);
            }
            return Step::Done(Attempt::NoAnswer(
                failure.unwrap_or_else(|| "no answer".to_owned()),
            ));
        };
        let repairable = matches!(score, 4 | 5 | 7);
        if !repairable || work.round >= LINE_REPAIRS {
            return Step::Done(attempt);
        }
        let card_problem = match &attempt {
            Attempt::Judged { report, .. } => Some(format!(
                "the card with every written line fails the checker:\n{}",
                diagnosis(report)
            )),
            _ => None,
        };
        let mut asking = vec![Vec::new(); lines];
        for line in 0..lines {
            let problem = problems[line].clone().or_else(|| card_problem.clone());
            let Some(problem) = problem else { continue };
            let mut request = work.last[line].clone();
            request
                .messages
                .push(Message::assistant(chosen[line].clone()));
            request
                .messages
                .push(Message::user(part::repair_message(&problem)));
            work.last[line] = request.clone();
            asking[line] = vec![request];
        }
        work.standing = chosen.into_iter().map(Some).collect();
        work.asking = asking;
        work.round += 1;
        Step::Again
    }

    /// Saves what a card came to: `Verified` and `Partial` as accepted proposals, a whole valid card the checker cannot
    /// prove as a draft (plays Unverified once promoted), the rest under `rejected/`; each kept file has a meta file that
    /// marks it model-written and unread. Returns the outcome, the word of the results file and the card's record.
    pub fn finish(
        &self,
        prepared: &Prepared<'_>,
        store: &Store,
        work: &Work,
        attempt: Attempt,
    ) -> Result<(Outcome, String, CardRecord), CliError> {
        let name = &prepared.card.name;
        let meta = json!({
            "source": mtg_db::SOURCE_SWEEP,
            "written_by": "model-line",
            "model": self.model,
            "kinds": prepared.gaps.iter().map(|(_, gap)| gap.kind.word()).collect::<Vec<_>>(),
            "lines": prepared.gaps.iter().map(|(_, gap)| gap.oracle.as_str()).collect::<Vec<_>>(),
            "agree": work.agree,
        });
        let write_meta = |base: &Path| {
            write(
                &base.join("meta").join(format!("{}.json", slug(name))),
                &meta.to_string(),
            )
        };
        let mut missing = Vec::new();
        let (outcome, detail) = match attempt {
            Attempt::Judged { json, report } => {
                let outcome = Outcome::of(&report);
                match &report.rules {
                    Some(rules) if outcome.is_kept() => {
                        store.save_accepted(name, rules)?;
                        write_meta(store.dir())?;
                    }
                    Some(rules) if outcome.is_draft() => {
                        store.save_draft(name, rules)?;
                        write_meta(&store.dir().join("drafts"))?;
                    }
                    _ => store.save_rejected(name, &json, &report.render(false))?,
                }
                let detail = detail_of(&outcome, &report);
                (outcome, detail)
            }
            Attempt::Bad { problems, answers } => {
                store.save_rejected(name, &answers, &problems)?;
                (Outcome::Invalid, problems)
            }
            Attempt::Missing(note) => {
                store.save_rejected(name, &json!({ "missing": note }).to_string(), &note)?;
                missing.push(note.clone());
                (
                    Outcome::Invalid,
                    format!("the rules cannot say the line: {note}"),
                )
            }
            Attempt::NoAnswer(why) => (Outcome::Failed(why.clone()), why),
        };
        let label = if missing.is_empty() {
            outcome.word().to_owned()
        } else {
            "cannot express".to_owned()
        };
        let record = CardRecord {
            card: name.clone(),
            outcome: outcome.key().to_owned(),
            missing_engine_piece: missing,
            requests: work.requests,
            micros: work.micros,
            detail,
            agree: work.agree,
        };
        store.save_record(&record)?;
        Ok((outcome, label, record))
    }
}
