//! Facts of the turn as amounts (g10c), one row each: "the amount of damage dealt to it this turn" (Tangled Colony),
//! "the damage already dealt to it this turn" (Whipkeeper), "the number of cards put into their graveyard from anywhere
//! this turn" (Fraying Sanity) and "for each +1/+1 counter you've put on creatures under your control this turn"
//! (Iridescent Hornbeetle).

use mtg_core::card::CardType;

use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, TallyEvent, TurnTally};

/// The amount after "where X is" or "equal to", when a row has it.
pub(super) fn said(amount: &Amount) -> Option<String> {
    let Amount::Tallied { who, what } = amount else {
        return None;
    };
    Some(
        match (who, what) {
            // When ~ dies: the creature as it was before it died.
            (Ref::This, TurnTally::DamageDealtTo) => "the amount of damage dealt to it this turn",
            (Ref::Target(0), TurnTally::DamageDealtTo) => {
                "the damage already dealt to it this turn"
            }
            (Ref::AttachedPlayer, TurnTally::CardsGraved) => {
                "the number of cards put into their graveyard from anywhere this turn"
            }
            _ => return None,
        }
        .to_owned(),
    )
}

/// "tokens with \"This token can't block,\" where X is ...": the comma of the where clause inside the closing quote,
/// beside the period of the quoted ability before it.
pub(super) fn add(texts: &mut Vec<String>) {
    const QUOTED: &str = ".\", where X is ";
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(QUOTED))
        .map(|text| text.replace(QUOTED, ",\" where X is "))
        .collect();
    texts.extend(found);
}

/// What "for each" counts, when a row has it.
pub(super) fn each(amount: &Amount) -> Option<String> {
    let Amount::Tally {
        event: TallyEvent::PlusOneCountersPut,
        who: Ref::You,
        filter,
    } = amount
    else {
        return None;
    };
    let yours = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    (*filter == yours)
        .then(|| "+1/+1 counter you've put on creatures under your control this turn".to_owned())
}
