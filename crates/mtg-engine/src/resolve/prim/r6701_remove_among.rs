//! `Effect::R6701RemoveAmong`: "Remove any number of counters from among permanents on the battlefield" (CR 122.8): the
//! controller takes one counter at a time from any holder the reference names, of any kind it still holds, until they
//! stop or none is left, and the number taken is kept under the effect's name (`Amount::Bound`).
//!
//! Each counter is one `ChooseOption` among every (holder, kind) still held and "Stop"; the answers are read back from
//! the journal in order, so a step with k questions runs k + 1 times.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::put_counters::holders;
use super::wrong_effect;
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::{CountersChanged, kinds_held};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, LET, Next, ResolveError, Run};

/// The option that ends the removal.
const R6701_STOP: &str = "Stop";

/// A counter still held: its holder, its kind and how many are left.
type Held = (Target, CounterKind, u32);

/// Runs the removal: asks for the next counter, or removes the counters picked and binds their number.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6701RemoveAmong { from, bind } = effect else {
        return Err(wrong_effect("R6701RemoveAmong"));
    };
    let mut held: Vec<Held> = Vec::new();
    for holder in holders(cx, from)? {
        for (kind, n) in kinds_held(cx.state(), holder) {
            if n > 0 {
                held.push((holder, kind, n));
            }
        }
    }
    let player = cx.scope.controller;
    let mut taken: Vec<(Target, CounterKind, i32)> = Vec::new();
    let mut answers = cx.journal().iter();
    loop {
        let offered: Vec<usize> = (0..held.len()).filter(|&at| held[at].2 > 0).collect();
        if offered.is_empty() {
            break;
        }
        let index = match answers.next() {
            None => return Ok(Run::Ask(r6701_question(cx, &held, &offered))),
            Some((who, Choice::Option(index))) if *who == player && *index <= offered.len() => {
                *index
            }
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to the choice of a counter to remove"
                )));
            }
        };
        let Some(&at) = offered.get(index) else {
            break;
        };
        let (holder, kind, left) = &mut held[at];
        *left -= 1;
        match taken
            .iter_mut()
            .find(|(on, took, _)| on == holder && took == kind)
        {
            Some((_, _, n)) => *n += 1,
            None => taken.push((*holder, kind.clone(), 1)),
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a counter that was not asked".into(),
        ));
    }
    let total: i32 = taken.iter().map(|(_, _, n)| n).sum();
    let events = taken
        .into_iter()
        .map(|(on, counter, n)| {
            Event::CountersChanged(CountersChanged {
                on,
                counter,
                delta: -n,
            })
        })
        .collect();
    Ok(Run::Done {
        events,
        bound: vec![(format!("{LET}{total}:{bind}"), Vec::new())],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// "Choose a counter to remove.": each kind still held on each holder, then "Stop".
fn r6701_question(cx: &Cx<'_>, held: &[Held], offered: &[usize]) -> Decision {
    let mut options: Vec<ChoiceOption> = offered
        .iter()
        .filter_map(|&at| held.get(at))
        .map(|(holder, kind, _)| {
            let whose = match holder {
                Target::Object(object) => cx
                    .state()
                    .object(object.id)
                    .map_or_else(String::new, |object| format!(" from {}", object.card().name)),
                Target::Player(player) => format!(" from player {}", player.0 + 1),
            };
            ChoiceOption::Label(format!("{} counter{whose}", kind.name()))
        })
        .collect();
    options.push(ChoiceOption::Label(R6701_STOP.into()));
    Decision {
        player: cx.scope.controller,
        kind: DecisionKind::ChooseOption(OptionOptions {
            can_cancel: false,
            source: cx.scope.source.id,
            options,
            text: "Choose a counter to remove, or stop.".into(),
        }),
    }
}
