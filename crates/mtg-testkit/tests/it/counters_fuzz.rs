//! Goal: whole bot games with decks of counters (Tireless Tracker, Cathars' Crusade, Inspiring Call, Caress of Phyrexia and cards that put, remove and move +1/+1 and -1/-1 counters and give players poison, energy and experience counters) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards, the counters on every permanent and player equal to those put on less those taken off and never fewer than none, no permanent with both a +1/+1 and a -1/-1 counter and no player with ten poison counters at a decision) for many seeds at tables of two, three and four seats, and the games really use them: counters are put on creatures and players, taken off, moved, annihilate in pairs by state-based actions and kill by poison; at every decision the power and toughness of a creature are its card's plus its +1/+1 counters less its -1/-1 counters; an engine rebuilt from the log at each decision asks what the original asks; and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use mtg_engine::player::LossReason;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameState, characteristics, replay};
use mtg_testkit::counter_cards::{COUNTER_CORPUS_CARDS, COUNTER_SYNTHETIC_CARDS};
use mtg_testkit::{RandomBot, counters_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(12)
}

/// What a campaign saw the counters do.
#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    plus_on_creatures: u64,
    minus_on_creatures: u64,
    poison: u64,
    energy: u64,
    experience: u64,
    taken_off: u64,
    pairs: u64,
    poisoned_out: u64,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for (n, event) in events.iter().enumerate() {
            match event {
                Event::ZoneChanged {
                    object,
                    to: Zone::Battlefield,
                    ..
                }
                | Event::SpellResolved { object } => {
                    if let Some(object) = game.object(*object) {
                        *self.entered.entry(object.card().name.clone()).or_default() += 1;
                    }
                }
                Event::CountersChanged(changed) => {
                    let on_a_player = changed.on.player().is_some();
                    match (&changed.counter, on_a_player, changed.delta > 0) {
                        (CounterKind::PlusOnePlusOne, false, true) => self.plus_on_creatures += 1,
                        (CounterKind::MinusOneMinusOne, false, true) => {
                            self.minus_on_creatures += 1;
                        }
                        (CounterKind::Poison, true, true) => self.poison += 1,
                        (CounterKind::Other(name), true, true) if name == "energy" => {
                            self.energy += 1;
                        }
                        (CounterKind::Other(name), true, true) if name == "experience" => {
                            self.experience += 1;
                        }
                        _ => {}
                    }
                    if changed.delta < 0 {
                        self.taken_off += 1;
                    }
                    // A +1/+1 and a -1/-1 counter taken off together, as many of each: state-based actions (CR 704.5q).
                    if changed.counter == CounterKind::PlusOnePlusOne
                        && changed.delta < 0
                        && let Some(Event::CountersChanged(next)) = events.get(n + 1)
                    {
                        self.pairs += u64::from(
                            next.on == changed.on
                                && next.counter == CounterKind::MinusOneMinusOne
                                && next.delta == changed.delta,
                        );
                    }
                }
                Event::PlayerLost {
                    reason: LossReason::Poison,
                    ..
                } => self.poisoned_out += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_counter_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = counters_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for name in COUNTER_CORPUS_CARDS.iter().chain(COUNTER_SYNTHETIC_CARDS) {
        assert!(
            seen.entered.get(*name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
    assert!(seen.plus_on_creatures > 0, "no +1/+1 counter in {campaign}");
    assert!(
        seen.minus_on_creatures > 0,
        "no -1/-1 counter in {campaign}"
    );
    assert!(seen.poison > 0, "no poison counter in {campaign}");
    assert!(seen.energy > 0, "no energy counter in {campaign}");
    assert!(seen.experience > 0, "no experience counter in {campaign}");
    assert!(seen.taken_off > 0, "no counter taken off in {campaign}");
    assert!(seen.pairs > 0, "no pair annihilated in {campaign}");
}

#[test]
fn a_game_is_lost_to_poison() {
    let mut poisoned_out = 0;
    for seed in 0..games() * 3 {
        let setup = counters_fuzz_setup(2, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let mut seen = Seen::default();
        seen.add(&replay(&setup, &outcome.events).unwrap(), &outcome.events);
        poisoned_out += seen.poisoned_out;
    }
    assert!(
        poisoned_out > 0,
        "no player lost to ten poison counters in {} games",
        games() * 3
    );
}

/// What a walk through the decisions of a game saw.
#[derive(Default)]
struct Walk {
    creatures_checked: u64,
    carrying_counters: u64,
}

impl Walk {
    /// Judges the state at a decision: the power and toughness of every creature are what its card prints plus its
    /// +1/+1 counters less its -1/-1 counters (the deck has no other effect on them).
    fn look(&mut self, state: &GameState) {
        for &id in state.zone(Zone::Battlefield) {
            let object = state.object(id).unwrap();
            let Some(chars) =
                characteristics(state, id).filter(mtg_engine::Characteristics::is_creature)
            else {
                continue;
            };
            let permanent = object.permanent().unwrap();
            let net = i32::try_from(permanent.counters_of(&CounterKind::PlusOnePlusOne)).unwrap()
                - i32::try_from(permanent.counters_of(&CounterKind::MinusOneMinusOne)).unwrap();
            let card = object.card();
            assert_eq!(
                (chars.power, chars.toughness),
                (card.power.map(|p| p + net), card.toughness.map(|t| t + net)),
                "{} with {:?}",
                card.name,
                permanent.counters().collect::<Vec<_>>()
            );
            self.creatures_checked += 1;
            self.carrying_counters += u64::from(net != 0);
        }
    }
}

/// Walks the games of `seeds` decision by decision with one bot a seat, judging each decision; with `rebuild` it also
/// resumes an engine from the log at each decision and fails when that engine differs from the live one.
fn walk_games(
    players: usize,
    seeds: std::ops::Range<u64>,
    rebuild: bool,
) -> Result<Walk, EngineError> {
    let mut walk = Walk::default();
    for seed in seeds {
        let setup = counters_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..4000 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            if rebuild && step < 1500 {
                let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
                assert_eq!(
                    resumed.state_hash(),
                    engine.state_hash(),
                    "seed {seed} step {step}"
                );
                for &id in engine.state().zone(Zone::Battlefield) {
                    assert_eq!(
                        characteristics(resumed.state(), id),
                        characteristics(engine.state(), id),
                        "seed {seed} step {step}: a creature differs once rebuilt"
                    );
                }
                let mid_cast = matches!(
                    &decision.kind,
                    mtg_engine::DecisionKind::PayMana(_)
                        | mtg_engine::DecisionKind::ChooseTargets(_)
                ) && !matches!(&decision.kind, mtg_engine::DecisionKind::ChooseTargets(o) if o.triggered);
                if !mid_cast {
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
            }
            walk.look(engine.state());
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    Ok(walk)
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_under_counters(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..8, true)?;
    assert!(walk.carrying_counters > 0, "no creature held a counter");
    Ok(())
}

#[rstest]
fn the_power_and_toughness_of_every_creature_follow_its_counters(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..games(), false)?;
    let seats = format!("{players} seats");
    assert!(walk.creatures_checked > 0, "no creature at {seats}");
    assert!(
        walk.carrying_counters > 0,
        "no creature held a counter at {seats}"
    );
    Ok(())
}

#[test]
fn a_seeded_counter_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = counters_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
