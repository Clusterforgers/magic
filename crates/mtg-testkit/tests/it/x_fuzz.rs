//! Goal: whole bot games with decks of X spells, X abilities and a `-X` planeswalker finish and keep every invariant at tables of two, three and four seats, and the games really use X: spells and abilities are announced with X above nothing and with nothing, the mana an X spell or ability spends is its cost plus X for each `{X}`, a `-X` takes exactly the counters X names, every X announced is kept in the choices of the object on the stack, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::for_each_seed;

use std::sync::Mutex;

use mtg_cards::value::CounterKind;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, play_game_with, x_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    spells_with_x: u64,
    spells_with_x_above_nothing: u64,
    abilities_with_x: u64,
    abilities_with_x_above_nothing: u64,
    minus_x: u64,
    minus_x_above_nothing: u64,
    resolved_with_x: u64,
}

fn play(players: usize, seed: u64) -> Vec<Event> {
    let setup = x_fuzz_setup(players, seed).unwrap();
    play_game_with(setup, seed, PlayOptions::default())
        .unwrap_or_else(|failure| panic!("{failure}"))
        .events
}

/// The mana spent with the cast or activation at `from`: what `ManaSpent` names before anything else happens.
fn spent_after(events: &[Event], from: usize) -> usize {
    for event in &events[from + 1..] {
        match event {
            Event::ManaSpent { mana, .. } => return mana.len(),
            Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::ResolutionBegan { .. }
            | Event::SpellCast { .. }
            | Event::AbilityActivated { .. } => break,
            _ => {}
        }
    }
    0
}

fn judge(seen: &mut Seen, setup: &mtg_engine::GameSetup, events: &[Event]) {
    let end = replay(setup, events).unwrap();
    for (at, event) in events.iter().enumerate() {
        match event {
            Event::SpellCast {
                object, choices, ..
            } => {
                // A player who has left the game takes their cards with them.
                let Some(card) = end.object(*object).map(|object| object.card()) else {
                    continue;
                };
                // A spell announces X exactly when its cost has an {X}.
                assert_eq!(choices.x.is_some(), card.x_in_cost > 0, "{}", card.name);
                let Some(x) = choices.x else { continue };
                seen.spells_with_x += 1;
                seen.spells_with_x_above_nothing += u64::from(x > 0);
                let rest = card.mana_cost.as_ref().map_or(0, |cost| cost.mana_value());
                assert_eq!(
                    spent_after(events, at),
                    usize::try_from(rest + x * u32::from(card.x_in_cost)).unwrap(),
                    "{} cast for X = {x}",
                    card.name
                );
            }
            Event::AbilityPutOnStack(put) => {
                let Some(name) = end.object(put.source.id).map(|o| o.card().name.clone()) else {
                    continue;
                };
                // The second ability of the planeswalker is its -X.
                let minus_x = name == "X Walker" && put.ability == 1;
                let wants_x = minus_x || name == "X Pinger" || name == "Twin Well";
                assert_eq!(put.choices.x.is_some(), wants_x, "{name}");
                let Some(x) = put.choices.x else { continue };
                if minus_x {
                    seen.minus_x += 1;
                    seen.minus_x_above_nothing += u64::from(x > 0);
                    // The batch is the cost, `AbilityActivated`, then `AbilityPutOnStack`.
                    let paid = at.checked_sub(2).and_then(|before| match &events[before] {
                        Event::CountersChanged(changed)
                            if changed.counter == CounterKind::Loyalty
                                && changed.on.object().is_some_and(|on| on.id == put.source.id) =>
                        {
                            Some(changed.delta)
                        }
                        _ => None,
                    });
                    assert_eq!(
                        paid,
                        (x > 0).then(|| -i32::try_from(x).unwrap()),
                        "a -X takes X counters"
                    );
                    continue;
                }
                seen.abilities_with_x += 1;
                seen.abilities_with_x_above_nothing += u64::from(x > 0);
                let expected = if name == "X Pinger" { x + 1 } else { 2 * x };
                assert_eq!(
                    spent_after(events, at),
                    usize::try_from(expected).unwrap(),
                    "{name} activated for X = {x}"
                );
            }
            Event::SpellResolved { object } => {
                // The spell kept its X until it resolved.
                if end.object(*object).is_some_and(|o| o.card().x_in_cost > 0) {
                    seen.resolved_with_x += 1;
                }
            }
            _ => {}
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_games_with_x_finish_keep_every_invariant_and_pay_for_x(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = x_fuzz_setup(players, seed).unwrap();
        let events = play(players, seed);
        judge(&mut seen.lock().unwrap(), &setup, &events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.spells_with_x > 0, "no X spell in {campaign}");
    assert!(seen.spells_with_x_above_nothing > 0, "{campaign}");
    assert!(
        seen.spells_with_x > seen.spells_with_x_above_nothing,
        "no X of nothing in {campaign}"
    );
    assert!(seen.abilities_with_x_above_nothing > 0, "{campaign}");
    assert!(seen.minus_x_above_nothing > 0, "{campaign}");
    assert!(seen.resolved_with_x > 0, "{campaign}");
}

#[test]
fn a_seeded_game_with_x_is_the_same_game_every_time() {
    for seed in [3, 11] {
        assert_eq!(play(2, seed), play(2, seed));
    }
}
