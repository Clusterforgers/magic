//! Folding the opening procedure: first player, shuffles and mulligans.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{invalid, player_mut};
use crate::error::EngineError;
use crate::events::Event;
use crate::player::MulliganStatus;
use crate::state::{GameState, Stage};
use crate::zones::{Placement, Zone};

pub(super) fn same_cards(a: &[ObjectId], b: &[ObjectId]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

fn expect_status(
    state: &GameState,
    player: PlayerId,
    wanted: MulliganStatus,
) -> Result<(), EngineError> {
    match state.player(player).map(|p| p.mulligan) {
        Some(s) if s == wanted => Ok(()),
        other => Err(invalid(format!(
            "{player:?} is {other:?}, expected {wanted:?}"
        ))),
    }
}

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::FirstPlayerChosen { player } => {
            player_mut(state, *player)?;
            state.first_player = *player;
            state.turn.active = *player;
            state.rng_index += 1;
        }
        Event::LibraryShuffled { player, order } => {
            let zone = Zone::Library(*player);
            if !same_cards(state.zones.get(zone), order) {
                return Err(invalid("shuffle changed the cards in the library"));
            }
            state.zones.set(zone, order.clone());
            state.rng_index += 1;
            // A shuffled library is known to nobody: the cards looked at by hand are not told apart any more.
            state.looked_by_hand.retain(|(seat, _)| seat != player);
            crate::sight::forget_revealed(state, *player);
        }
        Event::LibraryRefilled { player, order } => {
            let graveyard = state.zones.get(Zone::Graveyard(*player)).to_vec();
            let mut all = state.zones.get(Zone::Library(*player)).to_vec();
            all.extend_from_slice(&graveyard);
            if !same_cards(&all, order) {
                return Err(invalid(
                    "refilled library does not match library plus graveyard",
                ));
            }
            for card in graveyard {
                super::zones::move_object(state, card, Zone::Library(*player), Placement::Top)?;
            }
            state.zones.set(Zone::Library(*player), order.clone());
            player_mut(state, *player)?.failed_draws = 0;
            state.rng_index += 1;
        }
        Event::MulliganPhaseBegan => {
            state.stage = Stage::Mulligan;
            for player in &mut state.players {
                player.mulligan = MulliganStatus::NeedsHand;
            }
        }
        Event::HandDealt { player, cards } => {
            expect_status(state, *player, MulliganStatus::NeedsHand)?;
            for card in cards {
                if state.zones.get(Zone::Library(*player)).last() != Some(card) {
                    return Err(invalid("dealt card is not on top of the library"));
                }
                super::zones::move_object(state, *card, Zone::Hand(*player), Placement::Top)?;
            }
            player_mut(state, *player)?.mulligan = MulliganStatus::Deciding;
        }
        Event::MulliganDeclared { player } => {
            expect_status(state, *player, MulliganStatus::Deciding)?;
            player_mut(state, *player)?.mulligan = MulliganStatus::Mulliganing;
        }
        Event::MulliganTaken {
            player,
            library_order,
        } => take_mulligan(state, *player, library_order)?,
        Event::MulliganKept { player } => {
            expect_status(state, *player, MulliganStatus::Deciding)?;
            let rules = state.ruleset();
            let taken = state.player(*player).map_or(0, |p| p.mulligans);
            let bottom = rules.bottom_count(*player, taken);
            // A library with no card has none to scry.
            let scry = rules
                .scry_after_keep(taken)
                .min(state.zones.get(Zone::Library(*player)).len());
            player_mut(state, *player)?.mulligan = match (bottom, scry) {
                (0, 0) => MulliganStatus::Kept,
                (0, _) => MulliganStatus::Scrying,
                (n, _) => MulliganStatus::Bottoming(n),
            };
        }
        Event::CardsBottomed { player, cards } => {
            match state.player(*player).map(|p| p.mulligan) {
                Some(MulliganStatus::Bottoming(n)) if n == cards.len() => {}
                other => return Err(invalid(format!("cannot bottom now: {other:?}"))),
            }
            for card in cards {
                if !state.zones.get(Zone::Hand(*player)).contains(card) {
                    return Err(invalid("bottomed card is not in hand"));
                }
                super::zones::move_object(state, *card, Zone::Library(*player), Placement::Bottom)?;
            }
            player_mut(state, *player)?.mulligan = MulliganStatus::Kept;
        }
        Event::MulliganPhaseEnded => state.stage = Stage::Turns,
        other => return Err(invalid(format!("not an opening event: {other:?}"))),
    }
    Ok(())
}

/// A scry in the opening ends the one a seat owed after keeping its hand; a scry of a resolution owes nothing.
pub(super) fn scried(state: &mut GameState, player: PlayerId) -> Result<(), EngineError> {
    if state.stage == Stage::Mulligan {
        expect_status(state, player, MulliganStatus::Scrying)?;
        player_mut(state, player)?.mulligan = MulliganStatus::Kept;
    }
    Ok(())
}

fn take_mulligan(
    state: &mut GameState,
    player: PlayerId,
    library_order: &[ObjectId],
) -> Result<(), EngineError> {
    expect_status(state, player, MulliganStatus::Mulliganing)?;
    let hand = state.zones.get(Zone::Hand(player)).to_vec();
    let mut all = state.zones.get(Zone::Library(player)).to_vec();
    all.extend_from_slice(&hand);
    if !same_cards(&all, library_order) {
        return Err(invalid("mulligan shuffle does not match library plus hand"));
    }
    for card in &hand {
        if let Some(object) = state.objects.get_mut(card) {
            object.zone = Zone::Library(player);
            object.counter += 1;
        }
    }
    state.zones.set(Zone::Hand(player), Vec::new());
    state
        .zones
        .set(Zone::Library(player), library_order.to_vec());
    let seat = player_mut(state, player)?;
    seat.mulligans += 1;
    seat.mulligan = MulliganStatus::NeedsHand;
    state.rng_index += 1;
    Ok(())
}
