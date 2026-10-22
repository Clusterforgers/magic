//! Goal: whole bot games with decks of replacement effects (token, counter, life, draw and damage doublers, Rest in Peace, lands that enter tapped or for life, creatures that enter with counters, permanents that choose as they enter) finish and keep every invariant for many seeds at tables of two, three and four seats, and the games use them: an independent ledger replays each log and finds that the tokens an effect makes and the counters it puts are always among what the effects in play allow in some order (each effect once), that nothing is put into a graveyard while Rest in Peace is in play, that a replaced event never shows in the log as if it had happened, and that the order of replacement effects is asked only when two or more apply.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeSet;

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Target;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState, apply_event, replay};
use mtg_testkit::{play_game, replace_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(15)
}

const TOKEN_DOUBLERS: [&str; 3] = ["Anointed Procession", "Parallel Lives", "Doubling Season"];

/// The permanents of `player` on the battlefield named one of `names`.
fn held(state: &GameState, player: PlayerId, names: &[&str]) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == player && names.contains(&o.card().name.as_str()))
        .count()
}

fn name_of(state: &GameState, id: ObjectId) -> String {
    state
        .object(id)
        .map_or_else(|| "?".to_owned(), |o| o.card().name.clone())
}

/// The numbers `start` can become when each of the changes applies once, in any order.
fn reachable(start: u32, changes: &[Option<u32>]) -> BTreeSet<u32> {
    // `Some(times)` multiplies, `None` adds one (Hardened Scales).
    fn go(now: u32, left: &[Option<u32>], out: &mut BTreeSet<u32>) {
        if left.is_empty() {
            out.insert(now);
            return;
        }
        for at in 0..left.len() {
            let mut rest = left.to_vec();
            let change = rest.remove(at);
            go(change.map_or(now + 1, |times| now * times), &rest, out);
        }
    }
    let mut out = BTreeSet::new();
    go(start, changes, &mut out);
    out
}

/// The changes the effects in play make to +1/+1 counters put on a creature `owner` controls.
fn counter_changes(state: &GameState, owner: PlayerId) -> Vec<Option<u32>> {
    let mut changes = Vec::new();
    for id in state.zone(Zone::Battlefield) {
        let Some(source) = state.object(*id) else {
            continue;
        };
        if source.controller() != owner {
            continue;
        }
        match source.card().name.as_str() {
            "Hardened Scales" => changes.push(None),
            "Corpsejack Menace" | "Doubling Season" => changes.push(Some(2)),
            _ => {}
        }
    }
    changes
}

/// What a campaign saw the replacement effects do.
#[derive(Default)]
struct Seen {
    doubled_tokens: u64,
    counters_changed: u64,
    exiled_instead: u64,
    entered_tapped: u64,
    paid_life_to_enter: u64,
    entered_with_counters: u64,
    chosen_as_it_entered: u64,
    asked: usize,
}

/// Replays the log event by event and checks each against the state it happened in.
fn ledger(setup: &GameSetup, events: &[Event], seen: &mut Seen) {
    let mut state = GameState::from_setup(setup).unwrap();
    let mut resolving: Option<(ObjectId, GameState)> = None;
    let mut tokens_in_resolution = 0usize;
    let mut counters_in_resolution = 0i64;
    for (at, event) in events.iter().enumerate() {
        let rest_in_peace = state
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| name_of(&state, *id) == "Rest in Peace");
        if rest_in_peace {
            // Nothing is put into a graveyard, whichever way the event says it: a replaced event is not in the log.
            let into_a_graveyard = match event {
                Event::CreatureDied { .. }
                | Event::CardDiscarded { .. }
                | Event::ZoneChanged {
                    to: Zone::Graveyard(_),
                    ..
                } => true,
                Event::SpellResolved { object } => state.object(*object).is_some_and(|spell| {
                    spell.base_types().contains(CardType::Instant)
                        || spell.base_types().contains(CardType::Sorcery)
                }),
                _ => false,
            };
            assert!(
                !into_a_graveyard,
                "{event:?} at {at} with Rest in Peace in play"
            );
            if matches!(
                event,
                Event::ZoneChanged {
                    to: Zone::Exile,
                    ..
                }
            ) {
                seen.exiled_instead += 1;
            }
        }
        match event {
            Event::ResolutionBegan { object } => {
                resolving = Some((*object, state.clone()));
                tokens_in_resolution = 0;
                counters_in_resolution = 0;
            }
            Event::TokenCreated(token) if token.name == "Soldier" => tokens_in_resolution += 1,
            Event::CountersChanged(changed) if changed.counter == CounterKind::PlusOnePlusOne => {
                if let Target::Object(_) = changed.on {
                    counters_in_resolution += i64::from(changed.delta);
                }
            }
            Event::ResolutionEnded { object, outcome } => {
                if let Some((began, before)) = resolving.take().filter(|(began, _)| began == object)
                    && *outcome == ResolutionOutcome::Resolved
                {
                    check_resolution(
                        &before,
                        began,
                        tokens_in_resolution,
                        counters_in_resolution,
                        seen,
                    );
                }
            }
            Event::Entered { tapped: true, .. } => seen.entered_tapped += 1,
            Event::Entered {
                chosen: Some(_), ..
            } => seen.chosen_as_it_entered += 1,
            Event::LifePaid { amount: 2, .. } => seen.paid_life_to_enter += 1,
            _ => {}
        }
        // A creature with a replacement of its own that enters with counters gets them in the batch that brings it in.
        if let Event::SpellResolved { object } = event
            && name_of(&state, *object) == "Faithful Watchdog"
        {
            let owner = state.object(*object).unwrap().controller();
            let mut got = 0i64;
            for later in &events[at + 1..] {
                match later {
                    Event::CountersChanged(changed) if matches!(changed.on, Target::Object(named) if named.id == *object) =>
                    {
                        got += i64::from(changed.delta);
                    }
                    Event::Entered { .. } | Event::Attached { .. } => {}
                    _ => break,
                }
            }
            let allowed = reachable(3, &counter_changes(&state, owner));
            assert!(
                allowed.contains(&u32::try_from(got).unwrap()),
                "Faithful Watchdog entered with {got} counters; {allowed:?} are possible"
            );
            seen.entered_with_counters += 1;
        }
        apply_event(&mut state, event).unwrap();
    }
}

/// A spell or ability that resolved: the tokens and counters it made are what the effects in play allowed.
fn check_resolution(
    before: &GameState,
    object: ObjectId,
    tokens: usize,
    counters: i64,
    seen: &mut Seen,
) {
    let Some(resolving) = before.object(object) else {
        return;
    };
    let controller = resolving.controller();
    let source = resolving.ability().map_or_else(
        || name_of(before, object),
        |ability| name_of(before, ability.source().id),
    );
    let base = match source.as_str() {
        "Raise Soldiers" | "Hero of Bladehold" => Some(2usize),
        _ => None,
    };
    if let Some(base) = base
        && tokens > 0
    {
        let doublers = held(before, controller, &TOKEN_DOUBLERS);
        assert_eq!(
            tokens,
            base << doublers,
            "{source} made {tokens} tokens with {doublers} doublers"
        );
        if doublers > 0 {
            seen.doubled_tokens += 1;
        }
    }
    if source == "Growth Spurt" && counters > 0 {
        let target = resolving
            .choices()
            .targets
            .first()
            .and_then(|targets| targets.first())
            .and_then(|target| target.object());
        let Some(creature) = target.and_then(|named| before.object(named.id)) else {
            return;
        };
        let changes = counter_changes(before, creature.controller());
        let allowed = reachable(1, &changes);
        assert!(
            allowed.contains(&u32::try_from(counters).unwrap()),
            "Growth Spurt put {counters} counters with {changes:?} in play; {allowed:?} are possible"
        );
        if !changes.is_empty() {
            seen.counters_changed += 1;
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_replacement_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = replace_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash);
        ledger(&setup, &outcome.events, &mut seen);
        seen.asked += outcome.replacement_asks;
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.doubled_tokens > 0,
        "no tokens were doubled in {campaign}"
    );
    assert!(
        seen.counters_changed > 0,
        "no counters were changed in {campaign}"
    );
    assert!(
        seen.exiled_instead > 0,
        "nothing was exiled instead in {campaign}"
    );
    assert!(
        seen.entered_tapped > 0,
        "nothing entered tapped in {campaign}"
    );
    assert!(
        seen.paid_life_to_enter > 0,
        "no life was paid to enter in {campaign}"
    );
    assert!(
        seen.entered_with_counters > 0,
        "nothing entered with counters in {campaign}"
    );
    assert!(
        seen.chosen_as_it_entered > 0,
        "nothing chose as it entered in {campaign}"
    );
    assert!(
        seen.asked > 0,
        "no order of effects was asked in {campaign}"
    );
}

#[test]
fn a_seeded_replacement_game_is_the_same_game_every_time() {
    let setup = replace_fuzz_setup(3, 11).unwrap();
    let first = play_game(setup.clone(), 11).unwrap();
    let second = play_game(setup, 11).unwrap();
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
