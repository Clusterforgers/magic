//! Disturb (CR 702.146): "You may cast this card transformed from your graveyard by paying [cost] rather than its mana cost."
//!
//! The keyword is printed on the front face of a transforming double-faced card, and the spell cast is its back face: the
//! face is settled with the others (`faces::candidates` lists the back face for a card in a graveyard that has the keyword),
//! the cost is the alternative cost of a way (`ways.rs`), and a spell cast that way is exiled when it leaves the stack
//! (`CastWay::exiles_the_spell`). The permanent is the back face, which prints its own replacement of a move to a graveyard.

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::definition::Layout;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::ability_reader;
use crate::state::GameState;
use crate::ways::{Way, cast_cost, cost_words};
use crate::zones::Zone;

/// The rules of disturb.
#[derive(Debug)]
pub(crate) struct Disturb;

impl KeywordRules for Disturb {}

/// The disturb cost printed on the front face of the card `id`, when it is a transforming double-faced card.
pub(crate) fn cost_of(state: &GameState, id: ObjectId) -> Option<&Cost> {
    let whole = state.object(id)?.whole_card();
    if whole.layout != Layout::Transform {
        return None;
    }
    ability_reader::printed_rules(whole.face(0)).find_map(|rule| match &**rule {
        Ability::Keyword(Keyword::Disturb(cost)) => Some(cost),
        _ => None,
    })
}

/// Whether the card `id` lies in a graveyard with a disturb cost, so that its back face may be cast from there.
pub(crate) fn offers_back_face(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|card| matches!(card.zone(), Zone::Graveyard(_)))
        && cost_of(state, id).is_some()
}

/// The way of casting the card `id`, in use as its back face, from a graveyard for its disturb cost (CR 702.146a). The
/// cost is paid as flashback is: mana, life and the objects the caster picks.
pub(crate) fn way(state: &GameState, id: ObjectId, instant: bool) -> Option<Way> {
    let card = state.object(id)?;
    if card.face() != Some(1) || !matches!(card.zone(), Zone::Graveyard(_)) {
        return None;
    }
    let (mana, parts) = cast_cost(cost_of(state, id)?)?;
    Some(Way {
        alternative: Some(CastWay::Disturb),
        label: format!("Disturb {}", cost_words(&mana, &parts)),
        mana: Some(mana),
        instant,
        variant: 0,
        parts,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: disturb is a static rule of the engine and adds no ability of its own.
    use super::*;

    #[test]
    fn disturb_adds_no_ability() {
        let cost = mtg_cards::cost::Cost::mana("{2}{W}".parse().unwrap());
        assert_eq!(Disturb.adds_abilities(&Keyword::Disturb(cost)), []);
    }
}
