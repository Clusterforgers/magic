//! The monarch (CR 724): the designation, the card its player draws at the end step and the way combat damage takes it.
//!
//! One player at most is the monarch. A card makes a player the monarch with `Effect::BecomeMonarch`; the monarch draws
//! a card at the beginning of their end step (CR 724.2a), and the controller of a creature that deals combat damage to
//! the monarch becomes the monarch (CR 724.2b). Both are done by the engine as the step begins and the damage is dealt,
//! so neither waits on the stack. When the monarch leaves the game the active player becomes the monarch, or the next
//! player in turn order when the active player is the one leaving (CR 724.4).

use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::state::GameState;
use crate::steps::draw;
use crate::zones::Zone;

impl GameState {
    /// The monarch, or nobody before a card has made a player the monarch.
    pub const fn monarch(&self) -> Option<PlayerId> {
        self.monarch
    }

    /// The monarch as the current turn began, or nobody.
    pub const fn monarch_at_turn_start(&self) -> Option<PlayerId> {
        self.tallies.monarch_began
    }
}

/// Makes `player` the monarch, or nobody, and releases the cards exiled until an opponent of their exiler becomes the
/// monarch (their exile ends the moment the designation moves, CR 610.3).
pub(crate) fn set(state: &mut GameState, player: Option<PlayerId>) {
    state.monarch = player;
    let Some(player) = player else {
        return;
    };
    if !state.possible.linked_exile {
        return;
    }
    for id in state.zone(Zone::Exile).to_vec() {
        if let Some(object) = state.objects.get_mut(&id)
            && let Some((by, released)) = &mut object.monarch_wait
            && by != &player
        {
            *released = true;
        }
    }
}

/// The event that makes `player` the monarch; none when they already are.
pub(crate) fn become_monarch(state: &GameState, player: PlayerId) -> Option<Event> {
    (state.monarch != Some(player)).then_some(Event::MonarchChanged { player })
}

/// The monarch's draw at the beginning of their end step: one card, when the end step is theirs.
pub(crate) fn end_step_draw(state: &GameState) -> Vec<Event> {
    match (state.monarch, state.active_player()) {
        (Some(monarch), Some(active)) if monarch == active => draw(state, monarch, 1),
        _ => Vec::new(),
    }
}

/// Who becomes the monarch after `events` of combat damage: the controller of a creature that dealt damage to the
/// monarch. When several players' creatures did, the first in turn order from the active player wins, since their
/// trigger goes on the stack first and resolves last.
pub(crate) fn taken_by_combat(state: &GameState, events: &[Event]) -> Option<Event> {
    let monarch = state.monarch?;
    let hit_by: Vec<PlayerId> = events
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt {
                source,
                target: DamageTarget::Player(hurt),
                ..
            } if *hurt == monarch => controller_of(state, *source),
            _ => None,
        })
        .collect();
    let start = state.active_player().unwrap_or(monarch);
    state
        .turn_order_from(start)
        .into_iter()
        .find(|player| hit_by.contains(player))
        .and_then(|player| become_monarch(state, player))
}

/// The monarch after `lost` has left the game: the active player, else the next living player after the one leaving.
pub(crate) fn successor(state: &GameState, lost: PlayerId) -> Option<PlayerId> {
    state
        .active_player()
        .filter(|active| *active != lost && state.is_alive(*active))
        .or_else(|| state.next_alive_after(lost))
}

#[cfg(test)]
mod tests {
    //! Goal: the monarch draws a card at the beginning of their own end step and at no other seat's, combat damage to the monarch passes the designation to the controller of the creature (the first in turn order from the active player when several hit, none when the damage is to someone else or nobody is the monarch), and a monarch who leaves the game gives it to the active player or, when that is the one leaving, to the next player in turn order.
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::zones::Zone;

    /// Three seats, each controlling one creature on the battlefield; seat 0 is the active player.
    fn game(monarch: Option<usize>) -> GameState {
        let mut game = state(
            vec![vec![], vec![], vec![]],
            vec![
                vec![creature("Zero", 1, 1)],
                vec![creature("One", 1, 1)],
                vec![creature("Two", 1, 1)],
            ],
        );
        if let Some(seat) = monarch {
            let player = PlayerId::from_index(seat);
            apply_event(&mut game, &Event::MonarchChanged { player }).unwrap();
        }
        game
    }

    fn creature_of(game: &GameState, seat: usize) -> ObjectId {
        *game
            .zone(Zone::Battlefield)
            .iter()
            .find(|id| controller_of(game, **id) == Some(PlayerId::from_index(seat)))
            .unwrap()
    }

    fn hits(game: &GameState, hurt: &[(usize, usize)]) -> Vec<Event> {
        hurt.iter()
            .map(|(by, player)| Event::DamageDealt {
                source: creature_of(game, *by),
                target: DamageTarget::Player(PlayerId::from_index(*player)),
                amount: 1,
                deathtouch: false,
                form: crate::combat::DamageForm::Normal,
                toxic: 0,
            })
            .collect()
    }

    #[rstest]
    #[case::one_creature_hits_the_monarch(Some(0), &[(1, 0)], Some(1))]
    #[case::the_first_in_turn_order_from_the_active_player_wins(Some(0), &[(2, 0), (1, 0)], Some(1))]
    #[case::damage_to_someone_else_changes_nothing(Some(0), &[(1, 2)], None)]
    #[case::nobody_is_the_monarch(None, &[(1, 0)], None)]
    #[case::the_active_player_hits_the_monarch(Some(1), &[(0, 1), (2, 1)], Some(0))]
    #[case::the_next_after_the_active_player_hits(Some(0), &[(2, 0)], Some(2))]
    fn combat_damage_to_the_monarch_passes_the_designation_to_the_controller_of_the_creature(
        #[case] monarch: Option<usize>,
        #[case] hurt: &[(usize, usize)],
        #[case] becomes: Option<usize>,
    ) {
        let game = game(monarch);
        let events = hits(&game, hurt);
        assert_eq!(
            taken_by_combat(&game, &events),
            becomes.map(|seat| Event::MonarchChanged {
                player: PlayerId::from_index(seat)
            })
        );
    }

    #[rstest]
    #[case::the_monarch_has_the_turn(Some(0), true)]
    #[case::another_seat_is_the_monarch(Some(1), false)]
    #[case::nobody_is_the_monarch(None, false)]
    fn only_the_monarch_whose_turn_it_is_draws_at_the_end_step(
        #[case] monarch: Option<usize>,
        #[case] draws: bool,
    ) {
        let game = game(monarch);
        let events = end_step_draw(&game);
        assert_eq!(events.len(), usize::from(draws));
        if draws {
            assert!(matches!(
                events[0],
                Event::CardDrawn {
                    player: PlayerId(0),
                    ..
                }
            ));
        }
    }

    #[rstest]
    #[case::the_active_player_is_the_monarch_and_leaves(0, 1)]
    #[case::another_seat_leaves(2, 0)]
    fn the_monarch_who_leaves_gives_the_designation_to_the_active_player_or_the_next_seat(
        #[case] leaving: usize,
        #[case] expected: usize,
    ) {
        let mut game = game(Some(leaving));
        let event = Event::PlayerLost {
            player: PlayerId::from_index(leaving),
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &event).unwrap();
        assert_eq!(game.monarch(), Some(PlayerId::from_index(expected)));
    }

    #[test]
    fn a_player_who_is_not_the_monarch_leaving_changes_nothing() {
        let mut game = game(Some(0));
        let event = Event::PlayerLost {
            player: PlayerId(2),
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &event).unwrap();
        assert_eq!(game.monarch(), Some(PlayerId(0)));
    }

    #[rstest]
    #[case::held_before_the_turn(Some(1), None, Some(1))]
    #[case::taken_in_the_turn(None, Some(2), None)]
    #[case::held_and_lost_in_the_turn(Some(0), Some(2), Some(0))]
    #[case::nobody(None, None, None)]
    fn the_monarch_as_the_turn_began_is_the_one_before_anything_in_the_turn_changed_it(
        #[case] before: Option<usize>,
        #[case] during: Option<usize>,
        #[case] began: Option<usize>,
    ) {
        let mut game = game(None);
        let seat = |n: usize| PlayerId::from_index(n);
        if let Some(n) = before {
            apply_event(&mut game, &Event::MonarchChanged { player: seat(n) }).unwrap();
        }
        apply_event(
            &mut game,
            &Event::TurnBegan {
                number: 2,
                active: PlayerId(1),
            },
        )
        .unwrap();
        if let Some(n) = during {
            apply_event(&mut game, &Event::MonarchChanged { player: seat(n) }).unwrap();
        }
        assert_eq!(game.monarch_at_turn_start(), began.map(seat));
        // The next turn starts from what this one ended with.
        apply_event(
            &mut game,
            &Event::TurnBegan {
                number: 3,
                active: PlayerId(2),
            },
        )
        .unwrap();
        assert_eq!(game.monarch_at_turn_start(), game.monarch());
    }

    #[test]
    fn the_designation_is_part_of_the_state_hash() {
        assert_ne!(game(None).state_hash(), game(Some(1)).state_hash());
        assert_ne!(game(Some(0)).state_hash(), game(Some(1)).state_hash());
    }
}
