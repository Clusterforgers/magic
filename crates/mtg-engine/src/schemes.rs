//! Schemes and minions, campaign rules. A seat set up with a scheme deck sets its next scheme in motion as each of its
//! turns begins (in the manner of Archenemy, CR 314): the scheme is an emblem in its command zone until the next one
//! replaces it, and the scheme after it is public as the Omen. The deck repeats in its order. A seat set up with a
//! leader is a minion: it loses when its leader does.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::player::LossReason;
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// The events of `active` setting its next scheme in motion as its turn begins: the scheme in motion leaves the game and
/// the next one's emblem is numbered as the next new object.
pub(crate) fn at_turn_start(state: &GameState, active: PlayerId) -> Vec<Event> {
    let Some(player) = state.player(active) else {
        return Vec::new();
    };
    if player.schemes.is_empty()
        || !player.is_in_game()
        || player.is_spirit()
        || crate::spotlight::lull_ahead(state, active)
    {
        return Vec::new();
    }
    let mut events: Vec<Event> = player
        .scheme
        .filter(|old| state.object(*old).is_some())
        .map(|object| Event::ObjectLeftGame { object })
        .into_iter()
        .collect();
    events.push(Event::SchemeSetInMotion {
        player: active,
        emblem: state.next_object(),
    });
    events
}

/// Folds `SchemeSetInMotion`: the next scheme of the deck is a new emblem in the player's command zone.
pub(crate) fn fold(
    state: &mut GameState,
    id: PlayerId,
    emblem: ObjectId,
) -> Result<(), EngineError> {
    if state.objects.contains_key(&emblem) || state.ceased.contains_key(&emblem) {
        return Err(invalid(format!("{emblem:?} already exists")));
    }
    let player = state
        .players
        .iter_mut()
        .find(|p| p.id() == id)
        .ok_or_else(|| invalid(format!("no player {id:?}")))?;
    let len = player.schemes.len();
    if len == 0 {
        return Err(invalid(format!("{id:?} has no scheme deck")));
    }
    let at = usize::try_from(player.schemes_set).unwrap_or(0) % len;
    let definition = Arc::clone(&player.schemes[at]);
    player.schemes_set += 1;
    player.scheme = Some(emblem);
    let zone = Zone::Command(id);
    let mut object = GameObject::new(emblem, id, Arc::clone(&definition), zone);
    object.kind = ObjectKind::Emblem;
    crate::copies::note_definition(state, &definition);
    state.objects.insert(emblem, object);
    state.zones.insert(zone, emblem, Placement::Top);
    state.next_object = state.next_object.max(emblem.get() + 1);
    reset_passes(state);
    Ok(())
}

/// The losses of a state-based pass with the minions of each losing or departed leader added.
pub(crate) fn leaders_fall(state: &GameState, mut losses: Vec<Event>) -> Vec<Event> {
    if state.players().iter().all(|p| p.leader.is_none()) {
        return losses;
    }
    let mut losing: BTreeSet<PlayerId> = losses
        .iter()
        .filter_map(|event| match event {
            Event::PlayerLost { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    let gone = |id: PlayerId, losing: &BTreeSet<PlayerId>| {
        losing.contains(&id) || state.player(id).is_none_or(|p| !p.is_in_game())
    };
    // A chain of leaders falls in one pass.
    loop {
        let fallen: Vec<PlayerId> = state
            .players()
            .iter()
            .filter(|p| p.is_in_game() && !losing.contains(&p.id()))
            .filter(|p| p.leader.is_some_and(|leader| gone(leader, &losing)))
            .map(crate::player::Player::id)
            .collect();
        if fallen.is_empty() {
            return losses;
        }
        for player in fallen {
            losing.insert(player);
            losses.push(Event::PlayerLost {
                player,
                reason: LossReason::LeaderFell,
            });
        }
    }
}
