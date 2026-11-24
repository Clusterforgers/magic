//! Beam me up: "You may cast this card from your graveyard for <cost> if you also return a creature you control to its
//! owner's hand. Then exile this spell."
//!
//! The cast from the graveyard is an alternative cost chosen with the cast (`ways.rs`): the beam me up cost and, as part
//! of the total cost, the return of a creature its caster controls to its owner's hand (CR 601.2f, 601.2h), picked with
//! the cast like any part of a spell's cost. The spell is exiled when it leaves the stack (`CastWay::exiles_the_spell`).

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of beam me up.
#[derive(Debug)]
pub(super) struct BeamMeUp;

impl KeywordRules for BeamMeUp {}

/// The whole cost of casting a card with beam me up `cost`: that cost and the return of a creature its caster controls.
pub(crate) fn r116_beam_cost(cost: &Cost) -> Cost {
    let mut whole = cost.clone();
    whole.0.push(CostItem::ReturnToHand {
        filter: Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::R119CanBeBeamedUp,
        ]),
        count: mtg_cards::value::Amount::one(),
    });
    whole
}
