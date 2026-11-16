//! The record of one step's priority points and the answers between them, and the search for a cycle in it.

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use serde_json::Value;

use super::shape::{self, Counts, Measure};
use crate::decision::{Answer, Decision, DecisionKind, PriorityAction};
use crate::state::GameState;
use crate::turn::Step;

/// Identical cycles in a row that make a loop.
pub(crate) const CYCLES: usize = 3;
/// The longest cycle looked for, in priority points.
const MAX_PERIOD: usize = 24;
/// Priority points kept: enough for the longest cycle.
const KEPT: usize = CYCLES * MAX_PERIOD + 1;

/// How an answer was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum How {
    /// The only answer the question allows.
    Forced,
    /// A pass of priority; `could_act` when the player had something else to do.
    Passed {
        /// The holder could have acted instead.
        could_act: bool,
    },
    /// A choice among several.
    Chose,
}

impl How {
    /// How `answer` answers `decision` in `state`.
    pub(crate) fn of(state: &GameState, decision: &Decision, answer: &Answer) -> How {
        match (&decision.kind, answer) {
            (DecisionKind::Priority(options), Answer::Priority(PriorityAction::Pass)) => {
                How::Passed {
                    could_act: options.can_act,
                }
            }
            (DecisionKind::Priority(_), _) => How::Chose,
            _ if crate::choose::sole_answer(state, decision).is_some() => How::Forced,
            _ => How::Chose,
        }
    }
}

/// One answer given in the step.
#[derive(Debug, Clone)]
pub(crate) struct Taken {
    /// Who answered.
    pub player: PlayerId,
    /// The kind of question.
    pub kind: &'static str,
    /// The answer.
    pub answer: Answer,
    /// How it was given.
    pub how: How,
}

#[derive(Debug, Clone)]
struct Mark {
    shape: u64,
    counts: Counts,
    /// Answers taken before this point, counted from the start of the step.
    taken: usize,
    next_object: u64,
    lives: Vec<(PlayerId, i64)>,
    log: usize,
    stack: Vec<String>,
}

/// A loop found: one cycle's answers, ready to be repeated.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    /// The answers of one cycle, ids made relative (`relative`).
    pub answers: Vec<(Taken, Value)>,
    /// The shape of the point the cycle starts and ends at.
    pub start: u64,
    /// The count changes of one cycle.
    pub delta: Counts,
    /// The players who chose something in the cycle, in the order they did.
    pub drivers: Vec<PlayerId>,
    /// The players who passed priority while they could have acted.
    pub responders: Vec<PlayerId>,
    /// The names of the objects on the stack in the cycle, in the order they first appear.
    pub cards: Vec<String>,
    /// The key the loop is known by in this turn.
    pub key: u64,
    /// The life changes of one cycle.
    pub life: Vec<super::LoopLife>,
    /// The log positions the last cycle spans.
    pub log: (usize, usize),
}

impl Found {
    /// Some count goes down each cycle: the loop ends by itself.
    pub(crate) fn bounded(&self) -> bool {
        self.delta.iter().any(|(_, change)| *change < 0)
    }

    /// Only life totals go down: the loop ends with a player's loss.
    pub(crate) fn drains_life_only(&self) -> bool {
        self.delta
            .iter()
            .all(|((measure, _), change)| *change >= 0 || *measure == Measure::Life)
    }

    /// Every choice in the cycle was forced and no one passed while they could act.
    pub(crate) fn mandatory(&self) -> bool {
        self.drivers.is_empty() && self.responders.is_empty()
    }
}

/// The priority points of the step being played.
#[derive(Debug, Clone, Default)]
pub(crate) struct Watch {
    scope: Option<(u32, Step)>,
    marks: Vec<Mark>,
    taken: Vec<Taken>,
    /// Answers dropped from the front of `taken`.
    dropped: usize,
    /// The loops offered this turn, by key, with how often.
    offered: BTreeMap<u64, u32>,
    turn: u32,
    /// Iterations run by shortcuts this turn.
    pub run: u32,
}

impl Watch {
    /// Forgets the points: a loop must repeat its cycles again before it is found.
    pub(crate) fn clear(&mut self) {
        self.marks.clear();
        self.taken.clear();
        self.dropped = 0;
    }

    /// Notes an answer given.
    pub(crate) fn take(&mut self, taken: Taken) {
        if self.scope.is_some() {
            self.taken.push(taken);
        }
    }

    /// How often the loop `key` was offered this turn, counting this time.
    pub(crate) fn offered(&mut self, key: u64) -> u32 {
        let count = self.offered.entry(key).or_insert(0);
        *count += 1;
        *count - 1
    }

    /// Notes the priority point `state` stands at, `holder` holding priority, and returns the loop it closes, if any.
    pub(crate) fn mark(
        &mut self,
        state: &GameState,
        holder: PlayerId,
        log: usize,
    ) -> Option<Found> {
        let turn = state.turn();
        let scope = (turn.number(), turn.step());
        if self.turn != scope.0 {
            self.turn = scope.0;
            self.offered.clear();
            self.run = 0;
        }
        if self.scope != Some(scope) {
            self.clear();
            self.scope = Some(scope);
        }
        self.marks.push(Mark {
            shape: shape::shape(state, holder),
            counts: shape::counts(state),
            taken: self.dropped + self.taken.len(),
            next_object: state.next_object,
            lives: state
                .players()
                .iter()
                .map(|player| (player.id, i64::from(player.life)))
                .collect(),
            log,
            stack: shape::stack_names(state),
        });
        if self.marks.len() > KEPT {
            self.marks.remove(0);
            let first = self.marks.first().map_or(self.dropped, |mark| mark.taken);
            let drop = first.saturating_sub(self.dropped).min(self.taken.len());
            self.taken.drain(..drop);
            self.dropped += drop;
        }
        (1..=MAX_PERIOD).find_map(|period| self.cycle(state, period))
    }

    /// The loop of `period` points that the last points close, if they repeat `CYCLES` times.
    fn cycle(&self, state: &GameState, period: usize) -> Option<Found> {
        let span = CYCLES * period;
        let last = self.marks.len().checked_sub(1)?;
        let first = last.checked_sub(span)?;
        let marks = &self.marks;
        if !(first..=last - period).all(|i| marks[i].shape == marks[i + period].shape) {
            return None;
        }
        let delta = shape::delta(&marks[last - period].counts, &marks[last].counts);
        let ids = marks[last].next_object - marks[last - period].next_object;
        let window = marks[first].next_object;
        let mut answers: Option<Vec<Value>> = None;
        for k in 0..CYCLES {
            let (from, to) = (&marks[last - (k + 1) * period], &marks[last - k * period]);
            if to.next_object - from.next_object != ids
                || shape::delta(&from.counts, &to.counts) != delta
            {
                return None;
            }
            let cycle: Vec<Value> = self
                .answers(from, to)?
                .iter()
                .map(|taken| relative(&taken.answer, window, from.next_object))
                .collect::<Option<_>>()?;
            match &answers {
                Some(seen) if *seen != cycle => return None,
                Some(_) => {}
                None => answers = Some(cycle),
            }
        }
        let from = &marks[last - period];
        let to = &marks[last];
        let taken = self.answers(from, to)?;
        let relative = answers?;
        let mut drivers = Vec::new();
        let mut responders = Vec::new();
        for answer in taken {
            match answer.how {
                How::Chose if !drivers.contains(&answer.player) => drivers.push(answer.player),
                How::Passed { could_act: true } if !responders.contains(&answer.player) => {
                    responders.push(answer.player);
                }
                _ => {}
            }
        }
        let cards = cards_of(state, &marks[last - period + 1..=last], taken);
        let mut shapes: Vec<u64> = marks[last - period + 1..=last]
            .iter()
            .map(|mark| mark.shape)
            .collect();
        shapes.sort_unstable();
        let key = shape::loop_key(&shapes);
        Some(Found {
            answers: taken.iter().cloned().zip(relative).collect(),
            start: to.shape,
            delta,
            drivers,
            responders,
            cards,
            key,
            life: from
                .lives
                .iter()
                .zip(&to.lives)
                .filter(|(before, after)| before.1 != after.1)
                .map(|(before, after)| super::LoopLife {
                    player: before.0,
                    change: after.1 - before.1,
                })
                .collect(),
            log: (from.log, to.log),
        })
    }

    fn answers(&self, from: &Mark, to: &Mark) -> Option<&[Taken]> {
        let start = from.taken.checked_sub(self.dropped)?;
        let end = to.taken.checked_sub(self.dropped)?;
        self.taken.get(start..end)
    }
}

/// The names of the objects on the stack at the cycle's points and of the sources of the abilities activated in it.
fn cards_of(state: &GameState, marks: &[Mark], taken: &[Taken]) -> Vec<String> {
    let mut names: Vec<String> = marks
        .iter()
        .flat_map(|mark| mark.stack.iter().cloned())
        .collect();
    for answer in taken {
        if let Answer::Priority(PriorityAction::Activate(ability)) = &answer.answer
            && let Some(object) = state.object(ability.object)
        {
            names.push(object.card().name.clone());
        }
    }
    let mut seen = Vec::new();
    for name in names {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

/// `answer` as JSON with each object id made at or after `window` written relative to `base`, the first id of its
/// cycle, so the answers of two cycles compare equal; `None` when it cannot be written.
pub(crate) fn relative(answer: &Answer, window: u64, base: u64) -> Option<Value> {
    let mut value = serde_json::to_value(answer).ok()?;
    shift(&mut value, &|id| {
        (id >= window).then(|| Value::String(format!("@{}", i128::from(id) - i128::from(base))))
    });
    Some(value)
}

/// The answer `relative` wrote, with its relative ids made absolute against `base`.
pub(crate) fn absolute(value: &Value, base: u64) -> Option<Answer> {
    let mut value = value.clone();
    restore(&mut value, base);
    serde_json::from_value(value).ok()
}

fn shift(value: &mut Value, map: &dyn Fn(u64) -> Option<Value>) {
    match value {
        Value::Number(number) => {
            if let Some(mapped) = number.as_u64().and_then(map) {
                *value = mapped;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| shift(item, map)),
        Value::Object(fields) => fields.values_mut().for_each(|item| shift(item, map)),
        _ => {}
    }
}

fn restore(value: &mut Value, base: u64) {
    match value {
        Value::String(text) => {
            if let Some(offset) = text.strip_prefix('@').and_then(|n| n.parse::<i128>().ok())
                && let Ok(id) = u64::try_from(i128::from(base) + offset)
            {
                *value = Value::from(id);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| restore(item, base)),
        Value::Object(fields) => fields.values_mut().for_each(|item| restore(item, base)),
        _ => {}
    }
}
