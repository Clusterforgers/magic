//! Opening procedure: first player, shuffles, opening hands and the London mulligan (CR 103).

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::decision::{BottomOptions, Decision, DecisionKind, MulliganChoice, MulliganOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::flow::Progress;
use crate::player::{MulliganStatus, Player};
use crate::rng::GameRng;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

/// Library order after shuffling `cards` with the `offset`-th RNG substream of this batch of events: the batch's k-th
/// shuffle uses `rng_index + k`, since each folded shuffle moves the index on.
pub(crate) fn shuffled(state: &GameState, offset: u64, mut cards: Vec<ObjectId>) -> Vec<ObjectId> {
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

/// The next step of the opening procedure: deal, ask, shuffle back, or begin turn one.
pub(crate) fn next(state: &GameState) -> Progress {
    let rule = state.ruleset().mulligan_rule();
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
            kind: DecisionKind::Mulligan(
                state
                    .ruleset()
                    .mulligan_options(state.zone(Zone::Hand(*player)).to_vec(), taken),
            ),
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
