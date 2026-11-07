//! g13d: attacks at some players, written back: "Whenever an opponent attacks you and/or one or more planeswalkers you
//! control" and "Whenever one or more of your opponents are attacked". No template reads them: only the proposal checker
//! writes them.

use mtg_core::card::CardType;

use crate::refs::{Pred, Ref};

/// The wording of a declaration of attackers at you and your planeswalkers, or at your opponents; `None` for any other.
pub(super) fn g13d_attacked_text(what: &[Pred], at_least: u32) -> Option<String> {
    if at_least != 1 {
        return None;
    }
    let creature = Pred::Type(CardType::Creature);
    let at_you = Pred::AttackingPlayerOrPlaneswalker(Ref::You);
    match what {
        // Only an opponent attacks you, so the attacking player needs no word of its own.
        [kind, at] | [kind, Pred::ControlledBy(Ref::Opponents), at]
            if *kind == creature && *at == at_you =>
        {
            Some(
                "Whenever an opponent attacks you and/or one or more planeswalkers you control"
                    .to_owned(),
            )
        }
        [kind, Pred::AttackingPlayer(Ref::Opponents)] if *kind == creature => {
            Some("Whenever one or more of your opponents are attacked".to_owned())
        }
        _ => None,
    }
}
