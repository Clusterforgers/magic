//! Why the cards of a player's hand and command zone that the priority offer leaves out cannot be played now
//! (`PriorityOptions.unplayable`, `docs/engine.md`, Priority). The spells are judged once, and the same judgement decides
//! `castable`, so the offer runs the rules of a card once.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::decision::{FaceOption, Unplayable, UnplayableCard};
use crate::mana::{ManaOption, ManaPool};
use crate::object::ObjectKind;
use crate::priority::{land_play_left, sorcery_timing};
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// The cards of `player`'s hand and command zone in zone order: the cards `unplayable` speaks of.
fn own_cards(state: &GameState, player: PlayerId) -> impl Iterator<Item = ObjectId> + '_ {
    [Zone::Hand(player), Zone::Command(player)]
        .into_iter()
        .flat_map(|zone| state.zone(zone).iter().copied())
}

/// Each card of `player`'s hand and command zone with why it cannot be cast now, `None` when it can. `restricted` (split
/// second, "can't cast spells") forbids every cast and is the last reason looked for.
pub(crate) fn judge(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
    restricted: bool,
) -> BTreeMap<ObjectId, Option<Unplayable>> {
    own_cards(state, player)
        .map(|id| {
            let reason = ways::hindrance_now(state, player, id, pool, sources, life)
                .or(restricted.then_some(Unplayable::Restricted));
            (id, reason)
        })
        .collect()
}

/// The spells `player` can cast now, from the hand, then the other zones of [`ways::elsewhere`]: the judged cards by their
/// judgement, the others by [`ways::castable_now`].
pub(crate) fn castable(
    state: &GameState,
    player: PlayerId,
    judged: &BTreeMap<ObjectId, Option<Unplayable>>,
    restricted: bool,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<ObjectId> {
    state
        .zone(Zone::Hand(player))
        .iter()
        .copied()
        .chain(ways::elsewhere(state, player))
        .filter(|id| match judged.get(id) {
            Some(reason) => reason.is_none(),
            None => !restricted && ways::castable_now(state, player, *id, pool, sources, life),
        })
        .collect()
}

/// What the offer lists for the cards of a hand and the command zone: the cards in it count as playable now.
pub(crate) struct Offered<'a> {
    /// `PriorityOptions.playable_lands`.
    pub(crate) lands: &'a [ObjectId],
    /// `PriorityOptions.castable`.
    pub(crate) castable: &'a [ObjectId],
    /// `PriorityOptions.faces`.
    pub(crate) faces: &'a [FaceOption],
    /// `PriorityOptions.suspendable` and `foretellable`.
    pub(crate) special: [&'a [ObjectId]; 2],
}

/// The cards of `player`'s hand and command zone that `offered` leaves out, in zone order, each with its first reason: a
/// land waits for the timing and a land play, a spell gives its judgement.
pub(crate) fn unplayable(
    state: &GameState,
    player: PlayerId,
    judged: &BTreeMap<ObjectId, Option<Unplayable>>,
    offered: &Offered<'_>,
) -> Vec<UnplayableCard> {
    let listed: BTreeSet<ObjectId> = offered
        .lands
        .iter()
        .chain(offered.castable)
        .chain(offered.faces.iter().map(|face| &face.card))
        .chain(offered.special.iter().flat_map(|ids| ids.iter()))
        .copied()
        .collect();
    own_cards(state, player)
        .filter(|id| !listed.contains(id))
        .filter_map(|card| {
            let object = state.object(card)?;
            if object.kind() != ObjectKind::Card {
                return None;
            }
            let reason = if object.base_types().contains(CardType::Land) {
                land_reason(state, player)
            } else {
                judged.get(&card).copied().flatten()
            };
            Some(UnplayableCard {
                card,
                reason: reason.unwrap_or(Unplayable::Restricted),
            })
        })
        .collect()
}

/// Why `player` cannot play a land now: not their main phase with an empty stack, or no land play left (extra land
/// plays counted); `None` when both allow it.
fn land_reason(state: &GameState, player: PlayerId) -> Option<Unplayable> {
    if !sorcery_timing(state, player) {
        Some(Unplayable::Timing)
    } else if !land_play_left(state, player) {
        Some(Unplayable::LandPlayed)
    } else {
        None
    }
}
