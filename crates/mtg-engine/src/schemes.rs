//! Schemes and minions, campaign rules. A seat set up with a scheme deck sets its next scheme in motion as one of its
//! scheme turns begins (in the manner of Archenemy, CR 314): its first turn after the lull, then every `scheme_every`th
//! turn. The scheme is an emblem in its command zone until the seat's next turn begins, and the scheme after it is
//! public as the Omen. The deck repeats in its order. A seat set up with a leader is a minion: it loses when its leader
//! does.

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

/// The first of `player`'s turns that sets a scheme in motion: the first after the lull in a campaign fight, else its
/// first.
fn first_scheme_turn(state: &GameState, player: PlayerId) -> u32 {
    if crate::spotlight::active(state) && crate::spotlight::is_boss_side(state, player) {
        crate::spotlight::LULL_TURNS
    } else {
        1
    }
}

/// Whether `player`'s turn `turn` (counted from 1) sets a scheme in motion.
fn is_scheme_turn(state: &GameState, player: PlayerId, turn: u32) -> bool {
    let first = first_scheme_turn(state, player);
    let every = state.player(player).map_or(1, |p| p.scheme_every.max(1));
    turn >= first && (turn - first) % every == 0
}

/// How many of `player`'s turns are still to begin when its Omen is set in motion (1: as its next turn begins);
/// `None` without a scheme deck.
pub fn omen_in(state: &GameState, player: PlayerId) -> Option<u32> {
    let seat = state.player(player)?;
    if seat.schemes.is_empty() {
        return None;
    }
    // A turn that has begun has set its scheme already.
    let taken = state.rmt8_turns_taken(player);
    let next = taken.saturating_add(1);
    (next..next.saturating_add(seat.scheme_every.max(1) + crate::spotlight::LULL_TURNS))
        .find(|turn| is_scheme_turn(state, player, *turn))
        .map(|turn| turn - taken)
}

/// The events of `active` as its turn begins: the scheme in motion leaves the game and, on a scheme turn, the next
/// one's emblem is numbered as the next new object.
pub(crate) fn at_turn_start(state: &GameState, active: PlayerId) -> Vec<Event> {
    let Some(player) = state.player(active) else {
        return Vec::new();
    };
    if player.schemes.is_empty() || !player.is_in_game() || player.is_spirit() {
        return Vec::new();
    }
    let mut events: Vec<Event> = player
        .scheme
        .filter(|old| state.object(*old).is_some())
        .map(|object| Event::ObjectLeftGame { object })
        .into_iter()
        .collect();
    if crate::spotlight::lull_ahead(state, active)
        || !is_scheme_turn(
            state,
            active,
            state.rmt8_turns_taken(active).saturating_add(1),
        )
    {
        return events;
    }
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
