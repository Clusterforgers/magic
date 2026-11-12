//! More Than Meets the Eye (CR 702.162): "You may cast this card converted by paying [cost] rather than its mana cost."
//!
//! The keyword is printed on the front face of a transforming double-faced card, and the spell cast is its back face from
//! its owner's hand: the face is settled with the others (`faces::candidates` lists the back face for a card in a hand
//! that has the keyword), the cost is the alternative cost of a way (`ways.rs`), and the permanent is the back face, as a
//! disturbed card is (CR 702.162b).

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

/// The rules of More Than Meets the Eye.
#[derive(Debug)]
pub(crate) struct MoreThanMeetsTheEye;

impl KeywordRules for MoreThanMeetsTheEye {}

/// The cost printed with the keyword on the front face of the card `id`, when it is a transforming double-faced card.
pub(crate) fn cost_of(state: &GameState, id: ObjectId) -> Option<&Cost> {
    let whole = state.object(id)?.whole_card();
    if whole.layout != Layout::Transform {
        return None;
    }
    ability_reader::printed_rules(whole.face(0)).find_map(|rule| match &**rule {
        Ability::Keyword(Keyword::MoreThanMeetsTheEye(cost)) => Some(cost),
        _ => None,
    })
}

/// Whether the card `id` lies in a hand with the keyword, so that its back face may be cast from there.
pub(crate) fn offers_back_face(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|card| matches!(card.zone(), Zone::Hand(_)))
        && cost_of(state, id).is_some()
}

/// The way of casting the card `id`, in use as its back face, from a hand for the keyword's cost (CR 702.162a).
pub(crate) fn way(state: &GameState, id: ObjectId, instant: bool) -> Option<Way> {
    let card = state.object(id)?;
    if card.face() != Some(1) || !matches!(card.zone(), Zone::Hand(_)) {
        return None;
    }
    let (mana, parts) = cast_cost(cost_of(state, id)?)?;
    Some(Way {
        alternative: Some(CastWay::Alternative),
        label: format!("More Than Meets the Eye {}", cost_words(&mana, &parts)),
        mana: Some(mana),
        instant,
        variant: 0,
        parts,
    })
}
