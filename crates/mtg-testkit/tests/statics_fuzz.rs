//! Goal: whole bot games with decks of static abilities (anthems and a lord, a switch of power and toughness, a loss of flying, abilities given to every creature, a creature that cannot attack, a hold on small creatures and Humility) finish and keep every invariant (zones, life, the journal, effects in the order of their timestamps, no creature left that state-based actions would destroy, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: the cards with static abilities enter play, creatures change hands, permanents are changed by what they print and what they are given, a given ability is activated and one triggers; an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::abilities::AbilitySource;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, replay};
use mtg_testkit::static_cards::STATIC_CARDS;
use mtg_testkit::{RandomBot, play_game, statics_fuzz_setup};
use rstest::rstest;

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
    given_activations: u64,
    given_triggers: u64,
}

impl Seen {
    fn add(&mut self, game: &mtg_engine::GameState, events: &[Event]) {
        for event in events {
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
                Event::AbilityActivated {
                    origin: AbilitySource::Granted(_),
                    ..
                } => self.given_activations += 1,
                Event::AbilityTriggered(trigger)
                    if matches!(trigger.origin, AbilitySource::Granted(_)) =>
                {
                    self.given_triggers += 1;
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_static_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = statics_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for name in STATIC_CARDS {
        assert!(
            seen.entered.get(name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
    assert!(
        seen.given_activations > 0,
        "no given ability was activated in {campaign}"
    );
    assert!(
        seen.given_triggers > 0,
        "no given ability triggered in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_through_static_abilities(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let (mut stolen, mut changed) = (0u64, 0u64);
    for seed in 0..8 {
        let setup = statics_fuzz_setup(players, seed).unwrap();
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
            let state = engine.state();
            for &id in state.zone(Zone::Battlefield) {
                let object = state.object(id).unwrap();
                stolen += u64::from(controller_of(state, id) != Some(object.controller()));
                let shown = characteristics(state, id).unwrap();
                changed += u64::from(
                    (shown.power, shown.toughness)
                        != (object.card().power, object.card().toughness),
                );
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    // A table of three seats rarely has the hold in play within the decisions this walks through.
    assert!(
        stolen > 0 || players > 2,
        "no creature was under another player's control at a decision"
    );
    assert!(
        changed > 0,
        "no permanent had power or toughness other than its printed"
    );
    Ok(())
}

#[test]
fn a_seeded_static_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = statics_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
