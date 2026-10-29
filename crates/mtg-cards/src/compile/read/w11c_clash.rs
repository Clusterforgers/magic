//! Clash wordings beside `w9b_clash`: "Clash with an opponent, then return target creature to its owner's hand. If you
//! win, you may put that creature on top of its owner's library instead." and "Clash with an opponent. If you win,
//! creatures that player controls don't untap during the player's next untap step."

use mtg_core::card::CardType;

use super::conditional::body;
use super::tokens::Reader;
use super::w9b_clash::W9B_CLASH;
use crate::effect::{Destination, Effect, Place, W9B_CLASHED};
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector};

/// The win branch that puts the returned creature on top of its owner's library in place of the return.
pub(super) const W11C_INSTEAD: &str =
    "you may put that creature on top of its owner's library instead";

/// The win branch that holds the clashed player's creatures through their next untap step.
pub(super) const W11C_HELD: &str =
    "creatures that player controls don't untap during the player's next untap step";

/// The move of the one target to its owner's hand.
pub(super) fn w11c_to_hand() -> Effect {
    Effect::Move {
        what: Ref::Target(0),
        to: Destination::Hand,
    }
}

/// The win branch of "instead": the move of the target to the top of its owner's library, which the player may make.
/// The return that follows finds the creature gone (CR 400.7) and does nothing.
pub(super) fn w11c_on_top() -> Effect {
    Effect::May {
        who: Ref::You,
        effects: vec![Effect::Move {
            what: Ref::Target(0),
            to: Destination::Library {
                place: Place::Top,
                order: None,
            },
        }],
    }
}

/// The rule that keeps the clashed player's creatures tapped through that player's next untap step.
pub(super) fn w11c_held() -> Effect {
    Effect::Continuous {
        affected: Ref::Each(Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::ControlledBy(Ref::Bound(W9B_CLASHED.to_owned())),
        ])),
        mods: vec![Modification::DoesntUntap],
        duration: Duration::NextUntapStep,
    }
}

/// "Clash with an opponent, then return target creature to its owner's hand. If you win, you may put that creature on
/// top of its owner's library instead.": the clash whose win may put the creature on top, then the return.
fn w11c_then_return(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.punct(',').then_some(())?;
    r.need("then")?;
    let moved = body(r)?;
    (moved == [w11c_to_hand()] && r.targets.len() == 1 && r.targets[0].bounds == Bounds::default())
        .then_some(())?;
    r.punct('.').then_some(())?;
    r.need("if you win")?;
    r.punct(',').then_some(())?;
    r.need(W11C_INSTEAD)?;
    Some(vec![
        Effect::Clash {
            who: Ref::You,
            win: vec![w11c_on_top()],
            lose: Vec::new(),
            until: false,
            with: None,
        },
        w11c_to_hand(),
    ])
}

/// ". If you win, creatures that player controls don't untap during the player's next untap step."
fn w11c_hold(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.punct('.').then_some(())?;
    r.need("if you win")?;
    r.punct(',').then_some(())?;
    r.need(W11C_HELD)?;
    Some(vec![Effect::Clash {
        who: Ref::You,
        win: vec![w11c_held()],
        lose: Vec::new(),
        until: false,
        with: None,
    }])
}

/// The clash wordings of this module, tried before `w9b_clash`.
pub(super) fn w11c_clash(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(W9B_CLASH)?;
    r.attempt(w11c_then_return).or_else(|| r.attempt(w11c_hold))
}
