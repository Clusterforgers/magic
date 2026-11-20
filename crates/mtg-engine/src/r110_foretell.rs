//! Foretell beyond the printed keyword (CR 702.143): a card foretold with a foretell cost a static ability gave it in its
//! owner's hand keeps that cost in exile ("Its foretell cost is equal to its mana cost reduced by {2}"), foretelling is an
//! activation kind (`ActivationKind::R110Foretell`) whose cost the modifiers in play change ("Foretelling cards from your
//! hand costs {1} less", "The first card you foretell each turn costs {0} to foretell") and whose timing an
//! `InstantActivation` opens ("can be done on any player's turn"), and each foretelling is a record of the turn's log.

use mtg_cards::ability::ActivationKind;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::TallyEvent;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::events::ZoneCause;
use crate::keywords::keywords_of;
use crate::state::GameState;
use crate::zones::Zone;

/// What a move for `cause` keeps of the card `object` before it moves: for a foretelling from a hand, its owner and the
/// foretell cost it has there (the first one, read by the layers).
pub(crate) fn r110_kept_cost(
    state: &GameState,
    cause: ZoneCause,
    object: ObjectId,
) -> Option<(PlayerId, Option<Cost>)> {
    if cause != ZoneCause::Foretold {
        return None;
    }
    let card = state.object(object)?;
    let Zone::Hand(owner) = card.zone() else {
        return None;
    };
    let cost = keywords_of(state, object)
        .into_iter()
        .find_map(|keyword| match keyword {
            Keyword::Foretell(cost) => Some(cost.clone()),
            _ => None,
        });
    Some((owner, cost))
}

/// Keeps the foretell cost on the card now in exile and counts the foretelling for its owner.
pub(crate) fn r110_keep(
    state: &mut GameState,
    object: ObjectId,
    kept: Option<(PlayerId, Option<Cost>)>,
) {
    let Some((owner, cost)) = kept else {
        return;
    };
    if let Some(card) = state.objects.get_mut(&object) {
        card.r110_foretell_cost = cost;
    }
    state.record(
        TallyEvent::KindActivated(ActivationKind::R110Foretell),
        Some(owner),
        (object, None),
        1,
    );
}

/// The foretell keyword a card foretold in exile is cast with when it no longer has one: the cost it kept.
pub(crate) fn r110_kept_keyword(state: &GameState, id: ObjectId) -> Option<Keyword> {
    let card = state.object(id)?;
    if keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Foretell(_)))
    {
        return None;
    }
    card.r110_foretell_cost().cloned().map(Keyword::Foretell)
}

/// The mana `player` pays to foretell the card `id`: {2}, or {0} while a `FreeActivation` of foretelling applies to them,
/// then changed by the modifiers of foretelling in play.
pub(crate) fn r110_action_cost(state: &GameState, player: PlayerId, id: ObjectId) -> ManaCost {
    let base = if crate::free_activation::applies(state, player, ActivationKind::R110Foretell) {
        ManaCost::default()
    } else {
        crate::keywords::foretell::action_cost()
    };
    crate::activation_kind::r110_foretell_modified(state, id, base)
}

/// Whether `player` may foretell now: during their turn, or any time they have priority while an `InstantActivation` of
/// foretelling applies to them.
pub(crate) fn r110_may_foretell_now(state: &GameState, player: PlayerId) -> bool {
    state.is_active(player)
        || crate::free_activation::r110_instant(state, player, ActivationKind::R110Foretell)
}
