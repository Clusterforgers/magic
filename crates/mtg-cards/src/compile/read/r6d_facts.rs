//! Facts of the turn an intervening "if" asks (r6d), one row for each amount and comparison: "if no spells were cast
//! last turn", "if a permanent left the battlefield under your control this turn", "if a creature died this turn".

use super::facts::quantity_texts;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, TallyEvent, TurnTally};

/// The wordings of the amount compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let creature = Selector(vec![Pred::Type(mtg_core::card::CardType::Creature)]);
    match (left, op, number) {
        (Amount::CastLastTurn(Ref::EachPlayer), Cmp::Eq, 0) => {
            vec!["no spells were cast last turn".to_owned()]
        }
        // The most spells one player cast: "a player", not the spells of all players together.
        (Amount::CastLastTurn(Ref::EachPlayer), Cmp::Ge, 2) => {
            vec!["a player cast two or more spells last turn".to_owned()]
        }
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::PermanentsLeftUnder,
            },
            Cmp::Ge,
            1,
        ) => vec![
            "a permanent left the battlefield under your control this turn".to_owned(),
            "a permanent you controlled left the battlefield this turn".to_owned(),
        ],
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::Descended,
            },
            Cmp::Ge,
            1,
        ) => vec!["you descended this turn".to_owned()],
        (
            Amount::Tally {
                event: TallyEvent::Died,
                who: Ref::EachPlayer,
                filter,
            },
            Cmp::Ge,
            _,
        ) if *filter == creature => {
            if number == 1 {
                vec!["a creature died this turn".to_owned()]
            } else {
                quantity_texts(number)
                    .into_iter()
                    .map(|n| format!("{n} or more creatures died this turn"))
                    .collect()
            }
        }
        _ => super::g10c_facts::texts(left, op, number),
    }
}
