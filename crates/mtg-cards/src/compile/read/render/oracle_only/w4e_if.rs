//! An intervening "if" spliced after the event of a trigger written without it, for the conditions the trigger renderer
//! has no wording for: the state of ~ ("if ~ is tapped", "if ~ has ten or more luck counters on it"), the counters of
//! the object of the event ("if it had a -1/-1 counter on it", "if it had counters on it") and the life a player lost
//! this turn ("if a player lost 4 or more life this turn").

use super::super::super::as_long_as;
use super::super::super::counters::kind_text;
use super::super::super::draws;
use super::super::pattern_text;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Cmp, Condition, TurnTally};

/// The wordings of a trigger with an intervening condition; `bodies` writes the trigger without it.
pub(super) fn wordings(ability: &Ability, bodies: impl Fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if: Some(condition),
        limit: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let phrases = w4e_phrases(condition);
    let Some(event) = pattern_text(when) else {
        return Vec::new();
    };
    if phrases.is_empty() {
        return Vec::new();
    }
    let mut bare = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut bare {
        *intervening_if = None;
    }
    let opening = format!("{event}, ");
    let mut spliced = Vec::new();
    for text in bodies(&bare) {
        if let Some(rest) = text.strip_prefix(&opening) {
            spliced.extend(
                phrases
                    .iter()
                    .map(|phrase| format!("{opening}if {phrase}, {rest}")),
            );
        }
    }
    // "you draw a card" and "each player's" before a step are other wordings of the same trigger.
    let more = draws::wordings(ability, &spliced);
    spliced.extend(more);
    spliced.extend(you_draw(&spliced));
    spliced
}

/// "you draw a card" for "draw a card" after the condition.
fn you_draw(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.ends_with(", draw a card."))
        .map(|text| text.replace(", draw a card.", ", you draw a card."))
        .collect()
}

/// The ways the condition is worded after "if".
fn w4e_phrases(condition: &Condition) -> Vec<String> {
    let mut found = as_long_as::texts(condition);
    found.extend(event_counters(condition));
    found.extend(life_lost(condition));
    found
}

/// "it had a -1/-1 counter on it", "it had counters on it": the counters of the object of a leaving event.
fn event_counters(condition: &Condition) -> Option<String> {
    let Condition::Matches {
        subject: Ref::Triggering,
        selector,
    } = condition
    else {
        return None;
    };
    match selector.0.as_slice() {
        [Pred::HasAnyCounter] => Some("it had counters on it".to_owned()),
        [Pred::HasCounter(counter)] => {
            let kind = kind_text(counter)?;
            let article = super::super::super::triggers::article(kind);
            Some(format!("it had {article} {kind} counter on it"))
        }
        _ => None,
    }
}

/// "a player lost 4 or more life this turn".
fn life_lost(condition: &Condition) -> Option<String> {
    let Condition::Compare {
        left:
            Amount::Tallied {
                who: Ref::EachPlayer,
                what: TurnTally::LifeLost,
            },
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = condition
    else {
        return None;
    };
    Some(format!("a player lost {number} or more life this turn"))
}
