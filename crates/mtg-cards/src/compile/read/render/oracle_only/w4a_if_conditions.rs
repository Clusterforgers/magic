//! The wordings of the condition of a conditional step: the compiler's ("you do", "X is 5 or more") and those only the
//! oracle uses ("you can't", "it was a creature card", "an opponent has more life than you", "that card's mana value
//! is 4 or greater").

use super::super::super::conditional::condition_texts;
use super::super::super::counters::kind_text;
use super::super::super::facts::quantity_texts;
use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, StatKind};

/// The wordings of the condition after "if".
pub(super) fn w4a_condition_texts(condition: &Condition) -> Vec<String> {
    let mut out = condition_texts(condition);
    match condition {
        Condition::Not(inner) if **inner == Condition::Performed => {
            out.push("you can't".to_owned());
            out.push("you don't".to_owned());
        }
        Condition::Matches { subject, selector } => out.extend(w4a_matches(subject, selector)),
        Condition::Compare { left, op, right } => out.extend(w4a_compare(left, *op, right)),
        _ => {}
    }
    out
}

/// "it was a creature card", "it's a noncreature, nonland card", "this spell was cast from a graveyard".
fn w4a_matches(subject: &Ref, selector: &Selector) -> Vec<String> {
    match (subject, selector.0.as_slice()) {
        (Ref::This, [Pred::CastFrom(ZoneKind::Graveyard)]) => {
            vec!["this spell was cast from a graveyard".to_owned()]
        }
        (Ref::Target(0) | Ref::Bound(_), _) => {
            let Some(kind) = phrase_text(selector, Form::Subject) else {
                return Vec::new();
            };
            let card = if kind.ends_with(" card") {
                kind
            } else {
                format!("{kind} card")
            };
            let a = article(&card);
            vec![format!("it's {a} {card}"), format!("it was {a} {card}")]
        }
        _ => Vec::new(),
    }
}

/// The wordings of a comparison of two amounts.
fn w4a_compare(left: &Amount, op: Cmp, right: &Amount) -> Vec<String> {
    match (left, op, right) {
        (Amount::Life(Ref::You), Cmp::Ge, Amount::Sum(parts)) => match parts.as_slice() {
            [Amount::StartingLife(Ref::You), Amount::Fixed(points)] => vec![format!(
                "you have at least {points} life more than your starting life total"
            )],
            _ => Vec::new(),
        },
        (Amount::HighestLife(Ref::Opponents), Cmp::Gt, Amount::Life(Ref::You)) => {
            vec!["an opponent has more life than you".to_owned()]
        }
        (Amount::Largest { who, of }, Cmp::Gt, Amount::Count(counted))
            if *who == Ref::Opponents && of == counted =>
        {
            w4a_more_than_you(of)
        }
        (
            Amount::Greatest { of: yours, stat },
            Cmp::Ge,
            Amount::Greatest {
                of: all,
                stat: other,
            },
        ) if stat == other => w4a_greatest(yours, all, *stat),
        (Amount::Counters { on, counter }, _, Amount::Fixed(number)) => {
            let Some(kind) = kind_text(counter) else {
                return Vec::new();
            };
            w4a_counters(on, kind, op, *number)
        }
        (Amount::Stat { of, stat }, _, _) => w4a_stat(of, *stat, op, right),
        _ => Vec::new(),
    }
}

/// "an opponent has more cards in hand than you".
fn w4a_more_than_you(of: &Selector) -> Vec<String> {
    match of.0.as_slice() {
        [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)] => {
            vec!["an opponent has more cards in hand than you".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// "you control a creature with the greatest power among creatures on the battlefield".
fn w4a_greatest(yours: &Selector, all: &Selector, stat: StatKind) -> Vec<String> {
    let mut mine = all.0.clone();
    mine.push(Pred::ControlledBy(Ref::You));
    if yours.0 != mine
        || all
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        return Vec::new();
    }
    let (Some(kind), Some(kinds), Some(stat)) = (
        phrase_text(all, Form::Subject),
        phrase_text(all, Form::All),
        w4a_stat_word(stat),
    ) else {
        return Vec::new();
    };
    vec![format!(
        "you control {} {kind} with the greatest {stat} among {kinds} on the battlefield",
        article(&kind)
    )]
}

/// "it has eight or more night counters on it", "they don't have any rad counters".
fn w4a_counters(on: &Ref, kind: &str, op: Cmp, number: i32) -> Vec<String> {
    match (on, op, u32::try_from(number)) {
        (Ref::Triggering, Cmp::Eq, Ok(0)) => {
            vec![format!("they don't have any {kind} counters")]
        }
        (Ref::This, Cmp::Ge, Ok(number)) if number > 1 => quantity_texts(number)
            .iter()
            .map(|words| format!("it has {words} or more {kind} counters on it"))
            .collect(),
        _ => Vec::new(),
    }
}

/// "that card was all colors", "that card's mana value is less than or equal to the number of experience counters you
/// have", "the exiled card's mana value is 4 or greater".
fn w4a_stat(of: &Ref, stat: StatKind, op: Cmp, right: &Amount) -> Vec<String> {
    if !matches!(of, Ref::Target(0)) {
        return Vec::new();
    }
    if (stat, op, right) == (StatKind::Colors, Cmp::Eq, &Amount::Fixed(5)) {
        return ["that card is all colors", "that card was all colors"]
            .map(str::to_owned)
            .to_vec();
    }
    let Some(word) = w4a_stat_word(stat) else {
        return Vec::new();
    };
    let compared: Vec<String> = match (op, right) {
        (
            Cmp::Le,
            Amount::Counters {
                on: Ref::You,
                counter,
            },
        ) => kind_text(counter)
            .map(|kind| format!("is less than or equal to the number of {kind} counters you have"))
            .into_iter()
            .collect(),
        (Cmp::Ge | Cmp::Le, Amount::Fixed(number)) => {
            let more = if op == Cmp::Ge { "greater" } else { "less" };
            u32::try_from(*number)
                .map(quantity_texts)
                .unwrap_or_default()
                .iter()
                .map(|words| format!("is {words} or {more}"))
                .collect()
        }
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for subject in ["that card's", "the exiled card's", "its"] {
        for rest in &compared {
            out.push(format!("{subject} {word} {rest}"));
        }
    }
    out
}

/// The word of a characteristic.
fn w4a_stat_word(stat: StatKind) -> Option<&'static str> {
    match stat {
        StatKind::Power => Some("power"),
        StatKind::Toughness => Some("toughness"),
        StatKind::ManaValue => Some("mana value"),
        _ => None,
    }
}
