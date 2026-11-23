//! `Effect::R117AnotherOfKind`: "Choose a counter on target enchantment you control. Put another counter of that kind on
//! it." The kind is chosen among the counters the permanent has; the answer is logged as the index of the kind offered.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::put_counters::holders;
use super::wrong_effect;
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// For each permanent named, the controller picks one kind among its counters (asked only between two or more kinds,
/// in the fixed order of kinds) and one counter of that kind is put on it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R117AnotherOfKind { on } = effect else {
        return Err(wrong_effect("R117AnotherOfKind"));
    };
    let controller = cx.scope.controller;
    let mut answers = cx.journal().iter().filter(|entry| entry.0 == controller);
    let mut events = Vec::new();
    for holder in holders(cx, on)? {
        let Target::Object(object) = holder else {
            continue;
        };
        let Some(permanent) = cx.state().object(object.id).and_then(|o| o.permanent()) else {
            continue;
        };
        let kinds: Vec<&CounterKind> = permanent.counters().map(|(kind, _)| kind).collect();
        let kind = match kinds.as_slice() {
            [] => continue,
            [one] => *one,
            several => match answers.next() {
                Some((_, Choice::Option(at))) => several.get(*at).copied().ok_or_else(|| {
                    ResolveError::Invalid(format!("no counter kind {at} was offered"))
                })?,
                Some((_, other)) => {
                    return Err(ResolveError::Invalid(format!(
                        "a counter kind was asked, not {other:?}"
                    )));
                }
                None => {
                    return Ok(Run::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            source: cx.scope.source.id,
                            options: several
                                .iter()
                                .map(|kind| ChoiceOption::Label(format!("{} counter", kind.name())))
                                .collect(),
                            text: "Choose a kind of counter to put another of".to_owned(),
                            can_cancel: false,
                        }),
                    }));
                }
            },
        };
        events.push(Event::CountersChanged(CountersChanged {
            on: holder,
            counter: kind.clone(),
            delta: 1,
        }));
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question that was not asked".into(),
        ));
    }
    Ok(Run::events(events))
}
