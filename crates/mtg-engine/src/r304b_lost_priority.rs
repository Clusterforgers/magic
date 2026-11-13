//! Priority granted in the batch in which its player leaves the game: a draw step's draw, replaced by "you lose the
//! game" (Forbidden Crypt with an empty graveyard), is in one batch with the grant of priority to the active player,
//! worked out before the draw. A player who has left gets no priority (CR 800.4a), so the grant passes to the next
//! player in turn order who is still in the game. A player leaves once: a second loss in the batch (two damage events
//! each replaced by "you lose the game") is dropped.

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;

use crate::events::Event;
use crate::state::GameState;

/// `events` with each grant of priority to a player who left the game earlier in the batch passed on to the next player
/// in turn order still in it, and dropped when none is; a repeated loss of the same player is dropped.
pub(crate) fn r304b_passed_on(state: &GameState, events: Vec<Event>) -> Vec<Event> {
    if !events
        .iter()
        .any(|event| matches!(event, Event::PlayerLost { .. }))
    {
        return events;
    }
    let mut lost: BTreeSet<PlayerId> = BTreeSet::new();
    let mut kept = Vec::with_capacity(events.len());
    for event in events {
        match event {
            Event::PlayerLost { player, .. } => {
                if lost.insert(player) {
                    kept.push(event);
                }
            }
            Event::PriorityGranted { player } if lost.contains(&player) => {
                if let Some(next) = state
                    .turn_order_from(player)
                    .into_iter()
                    .find(|seat| !lost.contains(seat))
                {
                    kept.push(Event::PriorityGranted { player: next });
                }
            }
            other => kept.push(other),
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    //! Goal: a grant of priority to a player who lost earlier in the same batch goes to the next player in turn order
    //! still in the game, at two to four seats, and a grant to a player still in it or before the loss is kept; a player
    //! loses once in a batch.
    use super::*;
    use crate::fixtures::state;
    use crate::player::LossReason;
    use rstest::rstest;

    fn lost(seat: usize) -> Event {
        Event::PlayerLost {
            player: PlayerId::from_index(seat),
            reason: LossReason::Format("test".into()),
        }
    }

    fn granted(seat: usize) -> Event {
        Event::PriorityGranted {
            player: PlayerId::from_index(seat),
        }
    }

    #[rstest]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn a_second_loss_of_the_same_player_is_dropped(#[case] seats: usize) {
        let game = state(vec![vec![]; seats], vec![vec![]; seats]);
        let after = r304b_passed_on(&game, vec![lost(1), lost(0), lost(1), granted(0)]);
        assert_eq!(after, vec![lost(1), lost(0), granted(2)]);
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn a_grant_to_a_lost_player_passes_on(#[case] seats: usize) {
        let game = state(vec![vec![]; seats], vec![vec![]; seats]);
        let after = r304b_passed_on(&game, vec![lost(0), granted(0)]);
        assert_eq!(after, vec![lost(0), granted(1)]);
        let both = r304b_passed_on(&game, vec![lost(0), lost(1), granted(0)]);
        let expected = if seats > 2 {
            vec![lost(0), lost(1), granted(2)]
        } else {
            vec![lost(0), lost(1)]
        };
        assert_eq!(both, expected);
    }

    #[rstest]
    #[case::before_the_loss(vec![granted(0), lost(0)])]
    #[case::another_player(vec![lost(1), granted(0)])]
    #[case::no_loss(vec![granted(0)])]
    fn other_grants_are_kept(#[case] events: Vec<Event>) {
        let game = state(vec![vec![], vec![], vec![]], vec![vec![], vec![], vec![]]);
        assert_eq!(r304b_passed_on(&game, events.clone()), events);
    }
}
