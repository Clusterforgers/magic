//! Opening procedure: first player, shuffles, opening hands and the London mulligan (CR 103).

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::decision::{BottomOptions, Decision, DecisionKind, MulliganChoice, MulliganOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::flow::Progress;
use crate::library::Scried;
use crate::player::{MulliganStatus, Player};
use crate::rng::GameRng;
use crate::state::GameState;
use crate::turn::Step;
use crate::wording;
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
    // A companion is revealed before the libraries are shuffled, the first player's first (CR 702.139a).
    events.extend(crate::companions::reveals(state, first));
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

/// The events that deal each of `players` a hand of the size the rules give a hand after the mulligans they have
/// taken, from the top of their library, in turn order.
///
/// With opening-hand smoothing a seat first draws a second hand below the first and keeps the one with the land
/// count closer to the share of lands in its library (the first on a tie). The rest of the library is shuffled, the
/// hand not kept with it, and the kept hand put back on top: one `LibraryShuffled`, which nobody sees the order of, so
/// the hand not kept is never shown, not even to its owner. A library too small for two hands is not smoothed.
fn deal(state: &GameState, players: &[PlayerId]) -> Vec<Event> {
    let rules = state.ruleset();
    let mut events = Vec::new();
    let mut shuffles = 0;
    for player in players.iter().copied() {
        let taken = state.player(player).map_or(0, Player::mulligans);
        let size = rules.dealt_hand_size(player, taken);
        let library = state.zone(Zone::Library(player));
        let mut hand: Vec<ObjectId> = library.iter().rev().take(size).copied().collect();
        if rules.opening_hand_smoothing() && size > 0 && library.len() >= 2 * size {
            let second: Vec<ObjectId> = library
                .iter()
                .rev()
                .skip(size)
                .take(size)
                .copied()
                .collect();
            if closer_to_the_share(state, player, &second, &hand) {
                hand = second;
            }
            let rest: Vec<ObjectId> = library
                .iter()
                .copied()
                .filter(|card| !hand.contains(card))
                .collect();
            let mut order = shuffled(state, shuffles, rest);
            order.extend(hand.iter().rev());
            shuffles += 1;
            events.push(Event::LibraryShuffled { player, order });
        }
        events.push(Event::HandDealt {
            player,
            cards: hand,
        });
    }
    events
}

/// Whether `candidate` has a land count closer than `first` to the share of lands in the library of `player` times the
/// size of a hand; the shares are compared as whole numbers scaled by the library's size.
fn closer_to_the_share(
    state: &GameState,
    player: PlayerId,
    candidate: &[ObjectId],
    first: &[ObjectId],
) -> bool {
    let is_land = |card: &ObjectId| {
        state
            .object(*card)
            .is_some_and(|object| object.base_types().contains(CardType::Land))
    };
    let library = state.zone(Zone::Library(player));
    let lands = library.iter().filter(|card| is_land(card)).count();
    let distance = |hand: &[ObjectId]| {
        let in_hand = hand.iter().filter(|card| is_land(card)).count();
        (in_hand * library.len()).abs_diff(lands * hand.len())
    };
    distance(candidate) < distance(first)
}

/// The scry a seat that kept a hand after a mulligan owes: whether the top card of its library goes to the bottom.
pub(crate) fn scry_decision(state: &GameState) -> Option<Decision> {
    let player = state
        .turn_order_from(state.first_player())
        .into_iter()
        .find(|seat| status(state, *seat) == Some(MulliganStatus::Scrying))?;
    let top = *state.zone(Zone::Library(player)).last()?;
    Some(Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            source: top,
            from: vec![top],
            min: 0,
            max: 1,
            text: wording::SCRY.to_owned(),
            can_cancel: false,
        }),
    })
}

/// The answer to the scry: the card goes to the bottom or stays on top, as the one scry event of the seat.
pub(crate) fn scry_answer(
    player: PlayerId,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let (bottom, top) = options
        .from
        .iter()
        .copied()
        .partition(|card| choice.0.contains(card));
    Ok(Outcome::Events(vec![Event::Scried(Scried {
        player,
        top,
        bottom,
    })]))
}

/// The next step of the opening procedure: deal, ask, shuffle back, or begin turn one.
pub(crate) fn next(state: &GameState) -> Progress {
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
        return Progress::Events(deal(state, &needs_hand));
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
    if let Some(decision) = scry_decision(state) {
        return Progress::Decide(decision);
    }
    if let Some(player) = with_status(|s| s == MulliganStatus::Deciding).first() {
        let taken = state.player(*player).map_or(0, Player::mulligans);
        return Progress::Decide(Decision {
            player: *player,
            kind: DecisionKind::Mulligan(state.ruleset().mulligan_options(
                *player,
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
            extra: false,
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
