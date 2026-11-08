//! Facts of the turn written back after "if" or "as long as" (g10c), one row for each amount and comparison: "you've
//! committed a crime this turn", "you've put one or more +1/+1 counters on a creature this turn", "4 or more damage was
//! dealt to it this turn".

use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, TallyEvent, TurnTally};

/// The wordings of the amount compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    if op != Cmp::Ge || number == 0 {
        return Vec::new();
    }
    // "When ~ dies, if 4 or more damage was dealt to it this turn": the creature as it was before it died.
    if *left
        == (Amount::Tallied {
            who: Ref::This,
            what: TurnTally::DamageDealtTo,
        })
    {
        return super::facts::quantity_texts(number)
            .into_iter()
            .flat_map(|n| {
                [
                    format!("{n} or more damage was dealt to it this turn"),
                    format!("{n} or more damage was dealt to ~ this turn"),
                ]
            })
            .collect();
    }
    if number != 1 {
        return Vec::new();
    }
    match left {
        Amount::Tallied {
            who: Ref::You,
            what: TurnTally::Crimes,
        } => vec!["you've committed a crime this turn".to_owned()],
        Amount::Tallied {
            who: Ref::You,
            what: TurnTally::CreatureCardsGraved,
        } => vec!["a creature card was put into your graveyard from anywhere this turn".to_owned()],
        Amount::Tally {
            event: TallyEvent::PlusOneCountersPut,
            who,
            filter,
        } => plus_one_texts(who, filter),
        _ => Vec::new(),
    }
}

/// "you've put one or more +1/+1 counters on ~ this turn" (on a creature), and "a +1/+1 counter was put on a permanent
/// under your control this turn" when anyone put it.
fn plus_one_texts(who: &Ref, filter: &Selector) -> Vec<String> {
    let creature = Selector(vec![Pred::Type(mtg_core::card::CardType::Creature)]);
    let this = Selector(vec![Pred::This]);
    let yours = Selector(vec![Pred::ControlledBy(Ref::You)]);
    match who {
        Ref::You if *filter == this => {
            vec!["you've put one or more +1/+1 counters on ~ this turn".to_owned()]
        }
        Ref::You if *filter == creature => {
            vec!["you've put one or more +1/+1 counters on a creature this turn".to_owned()]
        }
        Ref::EachPlayer if *filter == yours => {
            vec!["a +1/+1 counter was put on a permanent under your control this turn".to_owned()]
        }
        _ => Vec::new(),
    }
}
