//! Opening procedure: first player, shuffles, opening hands and the London mulligan (CR 103).

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::decision::{BottomOptions, Decision, DecisionKind, MulliganChoice, MulliganOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::flow::Progress;
use crate::format::MulliganRule;
use crate::player::{MulliganStatus, Player};
use crate::rng::GameRng;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

/// Library order after shuffling `cards` with the `offset`-th RNG substream of this batch of events.
fn shuffled(state: &GameState, offset: u64, mut cards: Vec<ObjectId>) -> Vec<ObjectId> {
    GameRng::substream(state.seed, state.rng_index + offset).shuffle(&mut cards);
    cards
}

/// Events that pick the first player, shuffle every library and open the mulligan phase.
pub(crate) fn pregame(state: &GameState) -> Vec<Event> {
    let seats = state.players().len();
    let first = PlayerId::from_index(GameRng::substream(state.seed, state.rng_index).index(seats));
    let mut events = vec![Event::FirstPlayerChosen { player: first }];
    for (offset, seat) in (1u64..).zip(0..seats) {
        let player = PlayerId::from_index(seat);
        let order = shuffled(state, offset, state.zone(Zone::Library(player)).to_vec());
        events.push(Event::LibraryShuffled { player, order });
    }
    events.push(Event::MulliganPhaseBegan);
    events
}

fn status(state: &GameState, player: PlayerId) -> Option<MulliganStatus> {
    state.player(player).map(Player::mulligan_status)
}

/// The keep-or-mulligan options for `hand` after `taken` mulligans, with the bottom counts worked out from `rule`.
fn mulligan_options(rule: &MulliganRule, hand: Vec<ObjectId>, taken: u32) -> MulliganOptions {
    MulliganOptions {
        hand,
        mulligans_taken: taken,
        can_mulligan: usize::try_from(taken).is_ok_and(|t| t < rule.hand_size),
        free_mulligans: rule.free_mulligans,
        bottom_if_keep: rule.bottom_count(taken),
        bottom_if_mulligan: rule.bottom_count(taken.saturating_add(1)),
    }
}

/// The next step of the opening procedure: deal, ask, shuffle back, or begin turn one.
pub(crate) fn next(state: &GameState) -> Progress {
    let rule = state.format().mulligan_rule();
    let order = state.turn_order_from(state.first_player());
    let with_status = |wanted: fn(MulliganStatus) -> bool| -> Vec<PlayerId> {
        order
            .iter()
            .copied()
            .filter(|p| status(state, *p).is_some_and(wanted))
            .collect()
    };

    let needs_hand = with_status(|s| s == MulliganStatus::NeedsHand);
    if !needs_hand.is_empty() {
        return Progress::Events(
            needs_hand
                .into_iter()
                .map(|player| Event::HandDealt {
                    player,
                    cards: state
                        .zone(Zone::Library(player))
                        .iter()
                        .rev()
                        .take(rule.hand_size)
                        .copied()
                        .collect(),
                })
                .collect(),
        );
    }
    if let Some(player) = with_status(|s| matches!(s, MulliganStatus::Bottoming(_))).first() {
        let count = match status(state, *player) {
            Some(MulliganStatus::Bottoming(n)) => n,
            _ => 0,
        };
        return Progress::Decide(Decision {
            player: *player,
            kind: DecisionKind::BottomCards(BottomOptions {
                hand: state.zone(Zone::Hand(*player)).to_vec(),
                count,
            }),
        });
    }
    if let Some(player) = with_status(|s| s == MulliganStatus::Deciding).first() {
        let taken = state.player(*player).map_or(0, Player::mulligans);
        return Progress::Decide(Decision {
            player: *player,
            kind: DecisionKind::Mulligan(mulligan_options(
                &rule,
                state.zone(Zone::Hand(*player)).to_vec(),
                taken,
            )),
        });
    }
    let takers = with_status(|s| s == MulliganStatus::Mulliganing);
    if !takers.is_empty() {
        return Progress::Events(
            (0u64..)
                .zip(takers)
                .map(|(offset, player)| {
                    let mut cards = state.zone(Zone::Library(player)).to_vec();
                    cards.extend_from_slice(state.zone(Zone::Hand(player)));
                    Event::MulliganTaken {
                        player,
                        library_order: shuffled(state, offset, cards),
                    }
                })
                .collect(),
        );
    }
    Progress::Events(vec![
        Event::MulliganPhaseEnded,
        Event::TurnBegan {
            number: 1,
            active: state.first_player(),
        },
        Event::StepBegan { step: Step::Untap },
    ])
}

/// Turns a keep-or-mulligan choice into an event.
pub(crate) fn mulligan_answer(
    player: PlayerId,
    options: &MulliganOptions,
    choice: MulliganChoice,
) -> Result<Outcome, EngineError> {
    match choice {
        MulliganChoice::Keep => Ok(Outcome::Events(vec![Event::MulliganKept { player }])),
        MulliganChoice::Mulligan if options.can_mulligan => {
            Ok(Outcome::Events(vec![Event::MulliganDeclared { player }]))
        }
        MulliganChoice::Mulligan => Err(IllegalAnswer::NotOffered("mulligan").into()),
    }
}

/// Validates the cards chosen for the bottom of the library.
pub(crate) fn bottom_answer(
    player: PlayerId,
    options: &BottomOptions,
    cards: Vec<ObjectId>,
) -> Result<Outcome, EngineError> {
    check_card_choice(&options.hand, options.count, &cards)?;
    Ok(Outcome::Events(vec![Event::CardsBottomed {
        player,
        cards,
    }]))
}

/// Checks that `chosen` is exactly `count` distinct cards from `hand`.
pub(crate) fn check_card_choice(
    hand: &[ObjectId],
    count: usize,
    chosen: &[ObjectId],
) -> Result<(), IllegalAnswer> {
    if chosen.len() != count {
        return Err(IllegalAnswer::WrongCount {
            expected: count,
            got: chosen.len(),
        });
    }
    for (i, card) in chosen.iter().enumerate() {
        if chosen[..i].contains(card) {
            return Err(IllegalAnswer::Duplicate("card"));
        }
        if !hand.contains(card) {
            return Err(IllegalAnswer::NotOffered("card"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Goal: for every mulligan rule and number of mulligans taken, the options state the cards that go to the bottom now and after one more mulligan, as the bottom step will ask for them.
    use super::*;
    use rstest::rstest;

    fn options(hand_size: usize, free: u32, taken: u32) -> MulliganOptions {
        let rule = MulliganRule {
            hand_size,
            free_mulligans: free,
        };
        mulligan_options(&rule, vec![ObjectId(1)], taken)
    }

    #[rstest]
    #[case::normal_fresh(7, 0, 0, true, 0, 1)]
    #[case::normal_one(7, 0, 1, true, 1, 2)]
    #[case::normal_six(7, 0, 6, true, 6, 7)]
    #[case::normal_seven_is_the_last(7, 0, 7, false, 7, 8)]
    #[case::free_fresh(7, 1, 0, true, 0, 0)]
    #[case::free_spent(7, 1, 1, true, 0, 1)]
    #[case::free_then_paid(7, 1, 2, true, 1, 2)]
    #[case::two_free_fresh(5, 2, 0, true, 0, 0)]
    #[case::two_free_first(5, 2, 1, true, 0, 0)]
    #[case::two_free_second(5, 2, 2, true, 0, 1)]
    #[case::small_hand_last(5, 1, 5, false, 4, 5)]
    fn bottom_counts_follow_the_rule(
        #[case] hand_size: usize,
        #[case] free: u32,
        #[case] taken: u32,
        #[case] can: bool,
        #[case] keep: usize,
        #[case] mulligan: usize,
    ) {
        let got = options(hand_size, free, taken);
        assert_eq!(
            (
                got.mulligans_taken,
                got.free_mulligans,
                got.can_mulligan,
                got.bottom_if_keep,
                got.bottom_if_mulligan
            ),
            (taken, free, can, keep, mulligan)
        );
    }

    #[rstest]
    fn the_next_mulligan_costs_a_card_only_when_no_free_one_is_left(
        #[values(0, 1, 2)] free: u32,
        #[values(0, 1, 2, 3, 4)] taken: u32,
    ) {
        let got = options(7, free, taken);
        assert_eq!(
            got.bottom_if_mulligan - got.bottom_if_keep,
            usize::from(taken >= free)
        );
    }
}
