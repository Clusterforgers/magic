//! Opening procedure: first player, shuffles, opening hands and the London mulligan (CR 103).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::decision::{BottomOptions, Decision, DecisionKind, MulliganChoice, MulliganOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::{Event, ZoneCause};
use crate::flow::Progress;
use crate::library::{Revealed, Scried};
use crate::player::{MulliganStatus, Player};
use crate::rng::GameRng;
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::triggers::{DelayedCreated, DelayedId};
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
    let first = crate::restart::starting_player(state).unwrap_or_else(|| {
        PlayerId::from_index(GameRng::substream(state.seed, state.rng_index).index(seats))
    });
    let mut events = vec![Event::FirstPlayerChosen { player: first }];
    // A companion is revealed before the libraries are shuffled, the first player's first (CR 702.139a).
    events.extend(crate::companions::reveals(state, first));
    for (offset, seat) in (1u64..).zip(0..seats) {
        let player = PlayerId::from_index(seat);
        let order = shuffled(state, offset, state.zone(Zone::Library(player)).to_vec());
        events.push(Event::LibraryShuffled { player, order });
    }
    // Each Attraction deck is shuffled before the game begins too (CR 103.3a).
    let shuffles = crate::un33_attractions::shuffles(
        state,
        u64::try_from(seats).unwrap_or(u64::MAX).saturating_add(1),
    );
    // The sticker sheets are chosen with the substreams after them (CR 103.2d).
    let after = u64::try_from(seats + shuffles.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    events.extend(shuffles);
    events.extend(crate::un34_stickers::choices(state, after));
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

/// The scry `player`, who kept a hand after a mulligan, owes: whether the top card of its library goes to the bottom.
pub(crate) fn scry_decision(state: &GameState, player: PlayerId) -> Option<Decision> {
    let top = *state.zone(Zone::Library(player)).last()?;
    Some(Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
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

/// Whether `ability` lets `player` begin the game with its card on the battlefield: a plain one, or one for a player
/// who is not the starting player.
fn begins_on_battlefield(state: &GameState, player: PlayerId, ability: &Ability) -> bool {
    match ability {
        Ability::OpeningHand => true,
        Ability::OpeningHandWith { not_starting, .. } => {
            !not_starting || state.first_player() != player
        }
        _ => false,
    }
}

/// The cards in the hand of `player` that may begin the game on the battlefield or be revealed (CR 103.6).
pub(crate) fn opening_cards(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    state
        .zone(Zone::Hand(player))
        .iter()
        .copied()
        .filter(|id| {
            crate::ability_reader::abilities_of(state, *id)
                .iter()
                .any(|(_, ability)| {
                    begins_on_battlefield(state, player, ability)
                        || matches!(**ability, Ability::OpeningReveal { .. })
                })
        })
        .collect()
}

/// The counter `card` begins the game with and how many cards its owner then exiles from their hand (Gemstone Caverns).
pub(crate) fn dk26_opening_riders(state: &GameState, card: ObjectId) -> (Option<CounterKind>, u32) {
    crate::ability_reader::abilities_of(state, card)
        .iter()
        .find_map(|(_, ability)| match &**ability {
            Ability::OpeningHandWith {
                counter,
                exile_from_hand,
                ..
            } => Some((counter.clone(), *exile_from_hand)),
            _ => None,
        })
        .unwrap_or((None, 0))
}

/// The ability by which `card` is revealed from the opening hand, with its index, when it has no ability that puts it
/// onto the battlefield.
fn reveal_ability(state: &GameState, card: ObjectId) -> Option<(usize, Arc<Ability>)> {
    let abilities = crate::ability_reader::abilities_of(state, card);
    let owner = state.object(card)?.owner();
    if abilities
        .iter()
        .any(|(_, ability)| begins_on_battlefield(state, owner, ability))
    {
        return None;
    }
    abilities
        .into_iter()
        .find(|(_, ability)| matches!(**ability, Ability::OpeningReveal { .. }))
        .map(|(address, ability)| (address.ability, ability))
}

/// The delayed trigger a card revealed from the opening hand sets up, controlled by its owner; `None` for a card with
/// no such ability.
fn revealed_trigger(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    id: DelayedId,
) -> Option<DelayedCreated> {
    let (ability, rules) = reveal_ability(state, card)?;
    let Ability::OpeningReveal { when, effects } = &*rules else {
        return None;
    };
    let object = state.object(card)?;
    Some(DelayedCreated {
        id,
        source: object.reference(),
        snapshot: snapshot(state, card)?,
        controller: player,
        ability,
        when: when.clone(),
        once: true,
        this_turn: false,
        until: None,
        effects: effects.clone(),
        targets: Vec::new(),
        bindings: Vec::new(),
        kept: None,
    })
}

/// The answer to a `ChooseCards` of the opening: the opening-hand question of a seat that is `Opening`, else the scry.
pub(crate) fn opening_choice(
    state: &GameState,
    player: PlayerId,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    if status(state, player) == Some(MulliganStatus::Opening) && dk26_exile_owed(state, player) > 0
    {
        dk26_exile_answer(player, options, choice)
    } else if status(state, player) == Some(MulliganStatus::Opening) {
        opening_answer(state, player, options, choice)
    } else {
        scry_answer(player, options, choice)
    }
}

/// How many cards `player` still exiles from their hand for a card that began the game on the battlefield.
fn dk26_exile_owed(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).map_or(0, |seat| seat.opening_exile)
}

/// The question of the cards to exile from the hand after a card began the game on the battlefield ("If you do, exile a
/// card from your hand"): exactly as many as are owed, or the whole hand when it holds fewer.
fn dk26_exile_decision(state: &GameState, player: PlayerId) -> Option<Decision> {
    let hand = state.zone(Zone::Hand(player)).to_vec();
    let count = dk26_exile_owed(state, player).min(u32::try_from(hand.len()).unwrap_or(u32::MAX));
    Some(Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
            source: *hand.first()?,
            from: hand,
            min: count,
            max: count,
            text: wording::DK26_OPENING_EXILE.to_owned(),
            can_cancel: false,
        }),
    })
}

/// The answer to the exile question: the chosen cards go from the hand to exile.
fn dk26_exile_answer(
    player: PlayerId,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    Ok(Outcome::Events(
        choice
            .0
            .iter()
            .map(|card| Event::ZoneChanged {
                object: *card,
                from: Zone::Hand(player),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            })
            .collect(),
    ))
}

/// The answer to the opening-hand question: the chosen cards that reveal are revealed and set up their delayed
/// triggers, and the others are put onto the battlefield.
pub(crate) fn opening_answer(
    state: &GameState,
    player: PlayerId,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let (revealed, onto_battlefield): (Vec<ObjectId>, Vec<ObjectId>) = choice
        .0
        .iter()
        .partition(|card| reveal_ability(state, **card).is_some());
    let mut events = Vec::new();
    if !revealed.is_empty() {
        events.push(Event::Revealed(Revealed {
            player,
            cards: revealed.clone(),
        }));
    }
    let mut next = state.next_delayed;
    for card in revealed {
        if let Some(created) = revealed_trigger(state, player, card, DelayedId(next)) {
            next += 1;
            events.push(Event::DelayedTriggerCreated(Box::new(created)));
        }
    }
    // A card that begins the game with a counter on it (Gemstone Caverns) gets it as the new object on the battlefield.
    let counters: Vec<Event> = onto_battlefield
        .iter()
        .filter_map(|card| {
            let (counter, _) = dk26_opening_riders(state, *card);
            let object = state.object(*card)?;
            Some(Event::CountersChanged(crate::counters::CountersChanged {
                on: crate::choose::Target::Object(crate::object::ObjectRef {
                    id: *card,
                    counter: object.reference().counter + 1,
                }),
                counter: counter?,
                delta: 1,
            }))
        })
        .collect();
    events.push(Event::OpeningCardsPlayed {
        player,
        cards: onto_battlefield,
    });
    events.extend(counters);
    Ok(Outcome::Events(events))
}

/// Whether `player` is asked about the cards that begin on the battlefield now: once every seat has a final hand
/// (CR 103.6 comes after the mulligans), one seat at a time in turn order from the first player.
fn opening_turn(state: &GameState, player: PlayerId) -> bool {
    let order = state.turn_order_from(state.first_player());
    let waiting = |seat: &PlayerId| {
        !matches!(
            status(state, *seat),
            None | Some(MulliganStatus::Opening | MulliganStatus::Kept)
        )
    };
    !order.iter().any(waiting)
        && order
            .iter()
            .find(|seat| status(state, **seat) == Some(MulliganStatus::Opening))
            == Some(&player)
}

/// The question the opening asks `player` now, if any: the cards to bottom, the scry, or keep or mulligan.
pub(crate) fn seat_decision(state: &GameState, player: PlayerId) -> Option<Decision> {
    let kind = match status(state, player)? {
        MulliganStatus::Bottoming(count) => DecisionKind::BottomCards(BottomOptions {
            hand: state.zone(Zone::Hand(player)).to_vec(),
            count,
        }),
        MulliganStatus::Scrying => return scry_decision(state, player),
        MulliganStatus::Opening => {
            if !opening_turn(state, player) {
                return None;
            }
            if dk26_exile_owed(state, player) > 0 {
                return dk26_exile_decision(state, player);
            }
            let from = opening_cards(state, player);
            let text = if from
                .iter()
                .any(|card| reveal_ability(state, *card).is_some())
            {
                wording::OPENING_REVEAL
            } else {
                wording::OPENING_HAND
            };
            DecisionKind::ChooseCards(CardsOptions {
                one_per_name: false,
                source: *from.first()?,
                max: u32::try_from(from.len()).unwrap_or(u32::MAX),
                from,
                min: 0,
                text: text.to_owned(),
                can_cancel: false,
            })
        }
        MulliganStatus::Deciding => {
            let taken = state.player(player).map_or(0, Player::mulligans);
            DecisionKind::Mulligan(state.ruleset().mulligan_options(
                player,
                state.zone(Zone::Hand(player)).to_vec(),
                taken,
            ))
        }
        MulliganStatus::NeedsHand | MulliganStatus::Mulliganing | MulliganStatus::Kept => {
            return None;
        }
    };
    Some(Decision { player, kind })
}

/// Every question the opening asks at once, one per seat (CR 103.5: each seat decides about its own hand only, so the
/// answers may come in any order). The first is the engine's own decision: bottoming, then a scry, then keep or
/// mulligan, each in turn order from the first player; the other seats follow in turn order.
pub(crate) fn open_decisions(state: &GameState) -> Vec<Decision> {
    // A commander color still owed is the one question asked: it comes before any hand.
    if let Some(question) = crate::r404_commander_color::question(state) {
        return vec![question];
    }
    let mut asked: Vec<Decision> = state
        .turn_order_from(state.first_player())
        .into_iter()
        .filter_map(|player| seat_decision(state, player))
        .collect();
    let rank = |decision: &Decision| match decision.kind {
        DecisionKind::BottomCards(_) => 0,
        DecisionKind::ChooseCards(_) => 1,
        _ => 2,
    };
    if let Some(first) = (0..asked.len()).min_by_key(|at| rank(&asked[*at])) {
        let primary = asked.remove(first);
        asked.insert(0, primary);
    }
    asked
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

    // A commander's color is chosen before the game begins, before any hand is dealt.
    if let Some(decision) = crate::r404_commander_color::question(state) {
        return Progress::Decide(decision);
    }
    let needs_hand = with_status(|s| s == MulliganStatus::NeedsHand);
    if !needs_hand.is_empty() {
        return Progress::Events(deal(state, &needs_hand));
    }
    if let Some(decision) = open_decisions(state).into_iter().next() {
        return Progress::Decide(decision);
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
    // The cards a restart kept in exile enter once the opening is over, before the first turn (CR 727).
    let mut events = crate::restart::kept_enter(state);
    events.insert(0, Event::MulliganPhaseEnded);
    events.extend([
        Event::TurnBegan {
            number: 1,
            active: state.first_player(),
            extra: false,
        },
        Event::StepBegan { step: Step::Untap },
    ]);
    Progress::Events(events)
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
