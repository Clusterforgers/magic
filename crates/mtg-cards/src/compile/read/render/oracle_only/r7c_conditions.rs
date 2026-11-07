//! Conditions of a conditional step as the oracle words them (r7c): the mana of one color spent to cast the spell
//! ("Adamant — If at least three white mana was spent to cast this spell"), the mana spent to cast the spell that
//! triggered ("If five or more mana was spent to cast that spell"), the gift and the bargain, and "unless" with the
//! condition that does not hold ("Then discard a card unless you attacked this turn").

use super::super::super::amounts::count_word;
use super::super::super::facts::quantity_texts;
use super::w4a_if_conditions::w4a_condition_texts;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, StatKind};
use mtg_core::mana::ManaType;

/// The color word of a colored mana.
fn color_word(mana: ManaType) -> Option<&'static str> {
    Some(match mana {
        ManaType::White => "white",
        ManaType::Blue => "blue",
        ManaType::Black => "black",
        ManaType::Red => "red",
        ManaType::Green => "green",
        ManaType::Colorless => return None,
    })
}

/// The wordings of a condition the other writers do not word.
pub(super) fn r7c_condition_texts(condition: &Condition) -> Vec<String> {
    match condition {
        Condition::AdditionalCostPaid => vec!["this spell was bargained".to_owned()],
        Condition::Compare {
            left: Amount::Promised,
            op: Cmp::Eq,
            right: Amount::Fixed(promised @ (0 | 1)),
        } => vec![if *promised == 1 {
            "the gift was promised".to_owned()
        } else {
            "the gift wasn't promised".to_owned()
        }],
        Condition::Compare {
            left:
                Amount::ManaSpent {
                    of: Some(mana),
                    from: None,
                },
            op: Cmp::Ge,
            right: Amount::Fixed(number),
        } => {
            let (Some(color), Some(words)) = (
                color_word(*mana),
                u32::try_from(*number).ok().and_then(count_word),
            ) else {
                return Vec::new();
            };
            ["this spell", "it"]
                .iter()
                .map(|spell| format!("at least {words} {color} mana was spent to cast {spell}"))
                .collect()
        }
        Condition::Compare {
            left:
                Amount::Stat {
                    of: Ref::Triggering,
                    stat: StatKind::ManaSpent,
                },
            op: Cmp::Ge,
            right: Amount::Fixed(number),
        } => u32::try_from(*number)
            .map(quantity_texts)
            .unwrap_or_default()
            .iter()
            .map(|words| format!("{words} or more mana was spent to cast that spell"))
            .collect(),
        _ => Vec::new(),
    }
}

/// The condition that holds exactly when this one does not, for a comparison with a number or a negation.
fn negated(condition: &Condition) -> Option<Condition> {
    match condition {
        Condition::Not(inner) => Some((**inner).clone()),
        Condition::Compare { left, op, right } => {
            let (op, right) = match (op, right) {
                (Cmp::Eq, Amount::Fixed(0)) => (Cmp::Ge, Amount::Fixed(1)),
                (Cmp::Lt, right) => (Cmp::Ge, right.clone()),
                (Cmp::Ge, right) => (Cmp::Lt, right.clone()),
                (Cmp::Le, right) => (Cmp::Gt, right.clone()),
                (Cmp::Gt, right) => (Cmp::Le, right.clone()),
                _ => return None,
            };
            Some(Condition::Compare {
                left: left.clone(),
                op,
                right,
            })
        }
        _ => None,
    }
}

/// The wordings after "unless" of the condition of a step: those of the condition that does not hold.
pub(super) fn r7c_unless_texts(condition: &Condition) -> Vec<String> {
    negated(condition)
        .map(|other| w4a_condition_texts(&other))
        .unwrap_or_default()
}
