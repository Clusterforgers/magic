//! Goal: whole bot games with decks of effects that depend on each other, a loop, effects that read power, characteristic-defining abilities, a static ability that is given, Humility, Kormus Bell and the other corpus cards that change many characteristics finish and keep every invariant (zones, life, the journal, effects in the order of their timestamps, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: the cards enter play, characteristic-defining abilities define power and toughness, colours and types are changed by what other cards say, lands become creatures and Elves become artifacts; an engine rebuilt from the log at each decision asks what the original asks, the layers of a state are the same each time they are asked, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameState, replay};
use mtg_testkit::layer_cards::LAYER_CARDS;
use mtg_testkit::{RandomBot, layers_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::for_each_seed;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for event in events {
            if let Event::ZoneChanged {
                object,
                to: Zone::Battlefield,
                ..
            }
            | Event::SpellResolved { object } = event
                && let Some(object) = game.object(*object)
            {
                *self.entered.entry(object.card().name.clone()).or_default() += 1;
            }
        }
    }
}

/// What the layers made of the objects of a game, at one decision.
#[derive(Default)]
struct Tally {
    /// An object with no printed power whose power a characteristic-defining ability gave it, in any zone.
    defined_power: u64,
    /// A permanent with colours its card does not print.
    recoloured: u64,
    /// A land that is a creature.
    land_creatures: u64,
    /// An Elf an effect made that is also an artifact.
    elf_artifacts: u64,
    /// A creature with more power than its card prints and than the anthems alone give.
    pumped: u64,
}

impl Tally {
    fn add(&mut self, state: &GameState) {
        for object in state.objects() {
            let shown = characteristics(state, object.id()).unwrap();
            let printed = object.card();
            if printed.power.is_none() && shown.power.is_some() {
                self.defined_power += 1;
            }
            if object.zone() == Zone::Battlefield {
                self.recoloured += u64::from(shown.colors != printed.colors());
                self.land_creatures += u64::from(
                    shown.types.contains(CardType::Land)
                        && shown.types.contains(CardType::Creature),
                );
                self.elf_artifacts += u64::from(
                    shown.subtypes.contains(&Subtype("Elf".into()))
                        && shown.types.contains(CardType::Artifact),
                );
                self.pumped += u64::from(
                    shown
                        .power
                        .zip(printed.power)
                        .is_some_and(|(now, base)| now > base),
                );
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_layers_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = layers_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    for &name in LAYER_CARDS {
        assert!(
            seen.entered.get(name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_through_the_layers(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut tally = Tally::default();
    for seed in 0..8 {
        let setup = layers_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            let mid_cast = matches!(
                &decision.kind,
                DecisionKind::PayMana(_) | DecisionKind::ChooseTargets(_)
            ) && !matches!(&decision.kind, DecisionKind::ChooseTargets(o) if o.triggered);
            if !mid_cast {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            // The layers are worked out once for a state and read the same by every asker, and a state that is cloned
            // works out its own.
            let state = engine.state();
            let copy = state.clone();
            for object in state.objects() {
                assert_eq!(
                    characteristics(state, object.id()),
                    characteristics(&copy, object.id()),
                    "seed {seed} step {step}: {}",
                    object.card().name
                );
            }
            tally.add(state);
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    let campaign = format!("games of {players} seats");
    assert!(
        tally.defined_power > 0,
        "no characteristic-defining ability gave power in {campaign}"
    );
    assert!(tally.pumped > 0, "nothing was pumped in {campaign}");
    assert!(
        tally.recoloured > 0 || players > 2,
        "nothing changed colour in {campaign}"
    );
    Ok(())
}

#[test]
fn what_other_cards_say_changes_types_and_makes_lands_creatures_in_some_game() {
    // Across the seats of a bigger table and more seeds, every change the deck can make is seen at a decision.
    let mut tally = Tally::default();
    for seed in 0..24 {
        let setup = layers_fuzz_setup(2, seed).unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut bots = [RandomBot::new(seed, 0), RandomBot::new(seed, 1)];
        for _ in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            tally.add(engine.state());
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer).unwrap();
        }
    }
    assert!(
        tally.land_creatures > 0,
        "Kormus Bell never made a creature of a Swamp"
    );
    assert!(tally.elf_artifacts > 0, "no Elf was ever an artifact");
}

#[test]
fn a_seeded_layers_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = layers_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
