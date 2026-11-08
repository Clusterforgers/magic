//! The turn's activations by kind written back (e18a): "you may pay {0} rather than pay the equip cost of the first equip
//! ability you activate each turn" and its forms, and the facts "you cycled two or more cards this turn", "you've activated
//! a loyalty ability this turn" and "you've cast a spell from a graveyard or activated an ability of a card in a graveyard
//! this turn".

use super::as_long_as;
use super::facts::quantity_texts;
use crate::ability::{Ability, ActivationKind};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, TallyEvent};

/// The kind of the abilities you activated this turn that the amount counts, every source counted.
fn kind_tally(amount: &Amount) -> Option<ActivationKind> {
    match amount {
        Amount::Tally {
            event: TallyEvent::KindActivated(kind),
            who: Ref::You,
            filter,
        } if filter.is_empty() => Some(*kind),
        _ => None,
    }
}

/// The kind of "the first [kind] ability you activate": none of the kind activated by you this turn.
fn first_of(condition: &Condition) -> Option<ActivationKind> {
    match condition {
        Condition::Compare {
            left,
            op: Cmp::Lt,
            right: Amount::Fixed(1),
        }
        | Condition::Compare {
            left,
            op: Cmp::Eq,
            right: Amount::Fixed(0),
        } => kind_tally(left),
        _ => None,
    }
}

/// The word of the kind in "the equip cost", "equip costs".
const fn word(kind: ActivationKind) -> Option<&'static str> {
    match kind {
        ActivationKind::Equip => Some("equip"),
        ActivationKind::Cycling => Some("cycling"),
        ActivationKind::Unearth => Some("unearth"),
        _ => None,
    }
}

/// The sentences of a `FreeActivation`, without the period.
pub(super) fn free_activation_texts(ability: &Ability) -> Vec<String> {
    let Ability::FreeActivation { kind, condition } = ability else {
        return Vec::new();
    };
    let Some(noun) = word(*kind) else {
        return Vec::new();
    };
    let pay = "you may pay {0} rather than pay";
    let first = |each: &str| {
        let mut out = vec![format!(
            "{pay} the {noun} cost of the first {noun} ability you activate {each}"
        )];
        if *kind == ActivationKind::Cycling {
            out.push(format!(
                "{pay} the cycling cost of the first card you cycle {each}"
            ));
        }
        out
    };
    match condition {
        None => vec![format!("{pay} {noun} costs")],
        Some(held) if first_of(held) == Some(*kind) => first("each turn"),
        Some(Condition::All(parts)) => match parts.as_slice() {
            [Condition::YourTurn, held] | [held, Condition::YourTurn]
                if first_of(held) == Some(*kind) =>
            {
                first("during each of your turns")
            }
            _ => Vec::new(),
        },
        Some(held) if first_of(held).is_none() => as_long_as::texts(held)
            .into_iter()
            .map(|held| format!("as long as {held}, {pay} {noun} costs"))
            .collect(),
        Some(_) => Vec::new(),
    }
}

/// The wordings of the turn's activations compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    if op != Cmp::Ge || number == 0 {
        return Vec::new();
    }
    if from_a_graveyard(left) && number == 1 {
        return vec![
            "you've cast a spell from a graveyard or activated an ability of a card in a graveyard this turn"
                .to_owned(),
        ];
    }
    match kind_tally(left) {
        Some(ActivationKind::Loyalty) if number == 1 => vec![
            "you've activated a loyalty ability this turn".to_owned(),
            "you activated a loyalty ability this turn".to_owned(),
        ],
        Some(ActivationKind::Cycling) if number == 1 => vec![
            "you've cycled a card this turn".to_owned(),
            "you cycled a card this turn".to_owned(),
        ],
        Some(ActivationKind::Cycling) => quantity_texts(number)
            .into_iter()
            .flat_map(|n| {
                [
                    format!("you cycled {n} or more cards this turn"),
                    format!("you've cycled {n} or more cards this turn"),
                ]
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The spells you cast from a graveyard this turn and the abilities you activated of cards in a graveyard, added up.
fn from_a_graveyard(amount: &Amount) -> bool {
    let Amount::Sum(parts) = amount else {
        return false;
    };
    let cast = Amount::Tally {
        event: TallyEvent::SpellCast,
        who: Ref::EachPlayer,
        filter: Selector(vec![
            Pred::ControlledBy(Ref::You),
            Pred::WasCast {
                by: None,
                from: Some(ZoneKind::Graveyard),
            },
        ]),
    };
    let activated = Amount::Tally {
        event: TallyEvent::Activated,
        who: Ref::You,
        filter: Selector(vec![Pred::In(ZoneKind::Graveyard)]),
    };
    matches!(parts.as_slice(), [a, b] if *a == cast && *b == activated)
}
