//! Redealing hidden cards for a search: one possible world a seat cannot tell from the real game.
//!
//! A seat knows every deck list, so the cards it cannot see in a player's hand and library are that player's deck less
//! every card it has seen. Shuffling those cards among the hidden places of hand and library (hand sizes kept) deals a
//! world drawn from what the seat knows, never from what the hidden cards really are.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::engine::Engine;
use crate::rng::GameRng;
use crate::zones::Zone;

impl Engine {
    /// Deals the cards `viewer` cannot identify in every hand and library anew from `seed`: each player's unseen hand
    /// and library cards are shuffled together and dealt back to the same places, the cards `viewer` may identify
    /// (its own hand, revealed cards, a known top) staying where they are; the game's random stream is reseeded too.
    ///
    /// For searches on a copy: the log no longer replays to the state, and the pending decision must be `viewer`'s.
    pub fn redeal(&mut self, viewer: PlayerId, seed: u64) {
        let mut rng = GameRng::substream(seed, 0x0005_EA1E);
        let state = self.state_mut();
        let players: Vec<PlayerId> = state
            .players()
            .iter()
            .map(crate::player::Player::id)
            .collect();
        for player in players {
            let hand = Zone::Hand(player);
            let library = Zone::Library(player);
            let hidden =
                |state: &crate::state::GameState, id: ObjectId| !state.is_shown_to(viewer, id);
            let mut slots: Vec<(Zone, usize)> = Vec::new();
            let mut pool: Vec<ObjectId> = Vec::new();
            if player != viewer {
                for (at, id) in state.zone(hand).iter().enumerate() {
                    if hidden(state, *id) {
                        slots.push((hand, at));
                        pool.push(*id);
                    }
                }
            }
            for (at, id) in state.zone(library).iter().enumerate() {
                if hidden(state, *id) {
                    slots.push((library, at));
                    pool.push(*id);
                }
            }
            if pool.len() < 2 {
                continue;
            }
            // The deal starts from the cards in name order (ids break ties), so the world never follows where they
            // really lie: two tables that differ only in which hidden cards are where deal the same names to each slot.
            pool.sort_by_cached_key(|id| {
                (
                    state
                        .objects
                        .get(id)
                        .map(|object| object.card().name.clone())
                        .unwrap_or_default(),
                    *id,
                )
            });
            rng.shuffle(&mut pool);
            let mut hand_ids = state.zone(hand).to_vec();
            let mut library_ids = state.zone(library).to_vec();
            for ((zone, at), id) in slots.into_iter().zip(pool) {
                let list = if zone == hand {
                    &mut hand_ids
                } else {
                    &mut library_ids
                };
                if let Some(slot) = list.get_mut(at) {
                    *slot = id;
                }
                if let Some(object) = state.objects.get_mut(&id) {
                    object.zone = zone;
                }
            }
            state.zones.set(hand, hand_ids);
            state.zones.set(library, library_ids);
        }
        // Replaced, never mixed, so the world's random stream says nothing of the game's.
        state.seed = GameRng::substream(seed, 0x0005_EED5).next_u64();
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a redeal keeps every zone's size and the viewer's own hand, moves only hidden cards between a player's own
    //! hand and library, differs with the seed and never depends on where the hidden cards really lie.
    use std::collections::BTreeSet;

    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};

    /// A game of two seats with three cards in each hand and five in each library, at seat 0's main phase.
    fn game() -> Engine {
        let hand = |seat: u32| {
            (0..3)
                .map(|n| creature(&format!("Hand {seat} {n}"), 2, 2))
                .collect()
        };
        Engine::new(setup(vec![hand(0), hand(1)], vec![Vec::new(), Vec::new()]))
            .expect("the fixture game starts")
    }

    fn cards(engine: &Engine, zone: Zone) -> Vec<ObjectId> {
        engine.state().zone(zone).to_vec()
    }

    #[rstest]
    fn a_redeal_keeps_sizes_owners_and_the_viewers_hand(#[values(1, 2, 3, 99)] seed: u64) {
        let engine = game();
        let viewer = PlayerId(0);
        let mut world = engine.clone();
        world.redeal(viewer, seed);
        assert_eq!(
            cards(&engine, Zone::Hand(viewer)),
            cards(&world, Zone::Hand(viewer))
        );
        for player in engine
            .state()
            .players()
            .iter()
            .map(crate::player::Player::id)
        {
            let before: BTreeSet<ObjectId> = cards(&engine, Zone::Hand(player))
                .into_iter()
                .chain(cards(&engine, Zone::Library(player)))
                .collect();
            let after: BTreeSet<ObjectId> = cards(&world, Zone::Hand(player))
                .into_iter()
                .chain(cards(&world, Zone::Library(player)))
                .collect();
            assert_eq!(before, after);
            for zone in [Zone::Hand(player), Zone::Library(player)] {
                assert_eq!(cards(&engine, zone).len(), cards(&world, zone).len());
                for id in cards(&world, zone) {
                    assert_eq!(
                        world
                            .state()
                            .object(id)
                            .map(crate::object::GameObject::zone),
                        Some(zone)
                    );
                }
            }
        }
        let opponent = PlayerId(1);
        let mut other = engine.clone();
        other.redeal(viewer, seed + 1000);
        assert_ne!(
            cards(&world, Zone::Library(opponent)),
            cards(&other, Zone::Library(opponent))
        );
    }

    #[rstest]
    fn a_redeal_ignores_where_hidden_cards_really_lie(#[values(1, 2, 3, 99)] seed: u64) {
        let engine = game();
        let viewer = PlayerId(0);
        let opponent = PlayerId(1);
        let mut moved = engine.clone();
        {
            let state = moved.state_mut();
            let mut hidden: Vec<ObjectId> = state
                .zone(Zone::Hand(opponent))
                .iter()
                .chain(state.zone(Zone::Library(opponent)))
                .copied()
                .collect();
            hidden.rotate_left(2);
            let in_hand = state.zone(Zone::Hand(opponent)).len();
            let library = hidden.split_off(in_hand);
            for (zone, ids) in [
                (Zone::Hand(opponent), &hidden),
                (Zone::Library(opponent), &library),
            ] {
                for id in ids {
                    if let Some(object) = state.objects.get_mut(id) {
                        object.zone = zone;
                    }
                }
            }
            state.zones.set(Zone::Hand(opponent), hidden);
            state.zones.set(Zone::Library(opponent), library);
        }
        let mut world = engine.clone();
        world.redeal(viewer, seed);
        moved.redeal(viewer, seed);
        for zone in [Zone::Hand(opponent), Zone::Library(opponent)] {
            assert_eq!(cards(&world, zone), cards(&moved, zone));
        }
        assert_eq!(world.state().seed, moved.state().seed);
    }
}
