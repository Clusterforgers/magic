//! Why the cards a player could play that the priority offer leaves out cannot be played now (`PriorityOptions.unplayable`,
//! `docs/engine.md`, Priority options): their hand and command zone, and the cards of other zones a way or a permission
//! names for them. The spells are judged once, and the same judgement decides `castable`, so the offer runs the rules of
//! a card once.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::decision::{FaceOption, Unplayable, UnplayableCard, Why};
use crate::mana::{ManaOption, ManaPool};
use crate::object::ObjectKind;
use crate::priority::{land_play_left, sorcery_timing};
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// The cards of `player`'s hand and command zone in zone order: the cards [`judge`] judges.
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
            (
                id,
                reason_now(state, player, id, (pool, sources, life), restricted),
            )
        })
        .collect()
}

/// Why `player` cannot cast `id` now, `None` when they can; `restricted` is the last reason looked for.
fn reason_now(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    (pool, sources, life): (&ManaPool, &[ManaOption], i32),
    restricted: bool,
) -> Option<Unplayable> {
    ways::hindrance_now(state, player, id, pool, sources, life)
        .or(restricted.then_some(Unplayable::Restricted))
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

/// What the offer lists for the cards a player could play: the cards in it count as playable now.
pub(crate) struct Offered<'a> {
    /// `PriorityOptions.playable_lands`.
    pub(crate) lands: &'a [ObjectId],
    /// `PriorityOptions.castable`.
    pub(crate) castable: &'a [ObjectId],
    /// `PriorityOptions.faces`.
    pub(crate) faces: &'a [FaceOption],
    /// `PriorityOptions.suspendable`, `foretellable` and `plottable`.
    pub(crate) special: [&'a [ObjectId]; 3],
}

/// The cards `unplayable` speaks of, in order: `player`'s hand and command zone, then each card of another zone that
/// `player` sees and has a way to cast or a permission to play (a flashback card in the graveyard, a foretold card in
/// exile, the top of the library under Future Sight). A card of another zone with no way stays out, so a graveyard full
/// of spent spells says nothing, and a hidden card is never named.
fn explained_cards(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    let own = [Zone::Hand(player), Zone::Command(player)];
    let mut cards: Vec<ObjectId> = own
        .iter()
        .flat_map(|zone| state.zone(*zone).iter().copied())
        .collect();
    let lands = ways::lands_elsewhere(state, player);
    let others = ways::elsewhere(state, player).into_iter().filter(|id| {
        state.object(*id).is_some_and(|card| {
            !own.contains(&card.zone())
                && seen_by(state, player, *id)
                && if card.base_types().contains(CardType::Land) {
                    lands.contains(id)
                } else {
                    has_way(state, player, *id)
                }
        })
    });
    cards.extend(others);
    let mut seen = BTreeSet::new();
    cards.retain(|id| seen.insert(*id));
    cards
}

/// Whether `player` sees the card `id` of a zone besides their hand: face up, or face down and their own; the top of a
/// library only under a permission that covers it (Future Sight plays with it revealed).
fn seen_by(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    if card.face_down().is_some() && card.owner() != player {
        return false;
    }
    !matches!(card.zone(), Zone::Library(_))
        || crate::rules::cast_permissions(state)
            .iter()
            .any(|permission| permission.covers(state, player, card))
}

/// Whether `player` has a way to cast `id` from where it is, whatever the timing and the mana: of any face it may be
/// cast as.
fn has_way(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    if !crate::faces::is_multi_face(state, id) {
        return !ways::of(state, player, id).is_empty();
    }
    crate::faces::candidates(state, id)
        .into_iter()
        .any(|face| !ways::of(&state.with_face(id, face), player, id).is_empty())
}

/// The cards of [`explained_cards`] that `offered` leaves out, in order, each with its first reason: a land waits for the
/// timing and a land play, a spell gives its judgement.
pub(crate) fn unplayable(
    state: &GameState,
    player: PlayerId,
    judged: &BTreeMap<ObjectId, Option<Unplayable>>,
    offered: &Offered<'_>,
    at_hand: (&ManaPool, &[ManaOption], i32),
    restricted: bool,
) -> Vec<UnplayableCard> {
    let listed: BTreeSet<ObjectId> = offered
        .lands
        .iter()
        .chain(offered.castable)
        .chain(offered.faces.iter().map(|face| &face.card))
        .chain(offered.special.iter().flat_map(|ids| ids.iter()))
        .copied()
        .collect();
    explained_cards(state, player)
        .into_iter()
        .filter(|id| !listed.contains(id))
        .filter_map(|card| {
            let object = state.object(card)?;
            if object.kind() != ObjectKind::Card {
                return None;
            }
            let land = object.base_types().contains(CardType::Land);
            let reason = if land {
                land_reason(state, player, card)
            } else {
                // A card of another zone is judged here, once it is known to have a way.
                judged
                    .get(&card)
                    .copied()
                    .unwrap_or_else(|| reason_now(state, player, card, at_hand, restricted))
            };
            let why = match reason {
                Some(reason) if land => land_why(state, player, card, reason),
                Some(reason) => ways::why_now(state, player, card, reason, at_hand, restricted),
                None => None,
            };
            Some(UnplayableCard {
                card,
                reason: reason.unwrap_or(Unplayable::Restricted),
                why,
            })
        })
        .collect()
}

/// The detail of `reason`, which [`land_reason`] gave: the timing, or the rule that forbids the land play.
fn land_why(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    reason: Unplayable,
) -> Option<Why> {
    match reason {
        Unplayable::Timing => Some(Why::SorcerySpeed),
        Unplayable::Restricted => {
            crate::prohibit::forbidder(state, crate::prohibit::Doing::PlayLand { player })
                .or_else(|| {
                    crate::prohibit::forbidder(
                        state,
                        crate::prohibit::Doing::PlayLandCard { player, card },
                    )
                })
                .map(|source| crate::why::forbidden_by(state, source))
        }
        _ => None,
    }
}

/// Why `player` cannot play a land now: not their main phase with an empty stack, or no land play left (extra land
/// plays counted); `None` when both allow it.
fn land_reason(state: &GameState, player: PlayerId, card: ObjectId) -> Option<Unplayable> {
    if !sorcery_timing(state, player) {
        Some(Unplayable::Timing)
    } else if crate::prohibit::forbidden(state, crate::prohibit::Doing::PlayLand { player })
        || !crate::priority::land_card_open(state, player, card)
    {
        Some(Unplayable::Restricted)
    } else if !land_play_left(state, player) {
        Some(Unplayable::LandPlayed)
    } else {
        None
    }
}
