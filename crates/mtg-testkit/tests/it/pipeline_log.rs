//! Goal: the log of a game that uses spells and abilities replays to the live state at every point, an engine rebuilt from a decision point (or from the middle of a resolution) stands where the original stood and goes on the same way, every new event shows up in real games, and games without spells never contain one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::{Answer, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{burn, forest, free_creature, mountain, pinger, salve};
use mtg_testkit::{RandomBot, Scenario, fuzz_setup, ready, spell_fuzz_setup};
use rstest::rstest;

/// A game played by bots: its setup, its log and the (log length, state hash) after every answer.
struct Played {
    setup: GameSetup,
    events: Vec<Event>,
    marks: Vec<(usize, u64)>,
}

fn play(setup: GameSetup, seed: u64) -> Result<Played, EngineError> {
    let seats = u64::try_from(setup.players.len()).unwrap();
    let mut engine = Engine::new(setup.clone())?;
    let mut bots: Vec<RandomBot> = (0..seats).map(|seat| RandomBot::new(seed, seat)).collect();
    let mut marks = vec![(engine.events().len(), engine.state_hash())];
    while let Some(decision) = engine.decision().cloned() {
        let answer = bots[decision.player.index()].choose(&decision);
        engine.apply(answer)?;
        marks.push((engine.events().len(), engine.state_hash()));
    }
    Ok(Played {
        setup,
        events: engine.events().to_vec(),
        marks,
    })
}

#[rstest]
fn the_log_of_a_spell_game_replays_to_the_live_state_after_every_answer(
    #[values(2, 3)] players: usize,
    #[values(1, 9)] seed: u64,
) -> Result<(), EngineError> {
    let game = play(spell_fuzz_setup(players, seed), seed)?;
    for (len, hash) in &game.marks {
        let rebuilt = replay(&game.setup, &game.events[..*len])?;
        assert_eq!(rebuilt.state_hash(), *hash, "prefix of {len} events");
    }
    Ok(())
}

#[rstest]
fn an_engine_rebuilt_at_a_decision_stands_where_the_original_stood(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let seed = 4;
    let setup = spell_fuzz_setup(players, seed);
    let mut engine = Engine::new(setup.clone())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut announcing = 0;
    for step in 0..600 {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
        assert_eq!(resumed.state_hash(), engine.state_hash(), "step {step}");
        if matches!(
            decision.kind,
            DecisionKind::PayMana(_) | DecisionKind::ChooseTargets(_)
        ) {
            // A half-finished cast lives outside game state, so a rebuilt engine asks for priority again.
            announcing += 1;
            let again = resumed.decision().map(|d| (d.player, d.kind.name()));
            assert_eq!(again, Some((decision.player, "priority")), "step {step}");
        } else {
            assert_eq!(resumed.decision(), engine.decision(), "step {step}");
        }
        let answer = bots[decision.player.index()].choose(&decision);
        engine.apply(answer)?;
    }
    assert!(announcing > 0, "the game announced no cast or activation");
    Ok(())
}

/// Seat 0 casts a spell of three steps at a creature and the log of the resolution is the one under test.
fn salve_setup() -> GameSetup {
    Scenario::new(2)
        .hand(0, [salve("Salve")])
        .battlefield(0, [ready(mountain()), ready(forest())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
        .setup()
        .unwrap()
}

#[test]
fn an_engine_rebuilt_in_the_middle_of_a_resolution_goes_on_from_the_step_it_was_in()
-> Result<(), EngineError> {
    let setup = salve_setup();
    let mut engine = Engine::new(setup.clone())?;
    // Cast the spell at the creature, pay, and pass until it has resolved.
    let mut decision_points = vec![(engine.events().len(), engine.state_hash())];
    let mut script = |engine: &mut Engine, answer| -> Result<(), EngineError> {
        engine.apply(answer)?;
        decision_points.push((engine.events().len(), engine.state_hash()));
        Ok(())
    };
    let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
        panic!("priority");
    };
    let salve = options.castable[0];
    script(
        &mut engine,
        Answer::Priority(PriorityAction::CastSpell(salve)),
    )?;
    let bear = engine
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| engine.state().object(*id).unwrap().card().name == "Bear")
        .unwrap();
    let at = Target::Object(engine.state().object(bear).unwrap().reference());
    script(
        &mut engine,
        Answer::ChooseTargets(TargetChoice::Targets(vec![at])),
    )?;
    script(&mut engine, Answer::PayMana(PayChoice::Auto))?;
    while !engine.state().stack().is_empty() {
        script(&mut engine, Answer::Priority(PriorityAction::Pass))?;
    }
    let log = engine.events().to_vec();
    // The resolution has three steps; every prefix that ends at a step boundary is a state the log can be cut at.
    let boundaries: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::ResolutionBegan { .. } | Event::StepDone { .. }))
        .map(|(at, _)| at + 1)
        .collect();
    assert_eq!(boundaries.len(), 4, "the start and three steps");
    for cut in boundaries {
        let resumed = Engine::resume(setup.clone(), log[..cut].to_vec())?;
        // It went on from the step it was in, to the next decision, with the log the original had.
        let (len, hash) = *decision_points.iter().find(|(len, _)| *len >= cut).unwrap();
        assert_eq!(resumed.events(), &log[..len], "cut at {cut}");
        assert_eq!(resumed.state_hash(), hash, "cut at {cut}");
    }
    Ok(())
}

#[test]
fn every_event_of_the_pipeline_appears_in_real_games() -> Result<(), EngineError> {
    let mut seen = BTreeSet::new();
    let mut outcomes = BTreeSet::new();
    let mut announced = false;
    for seed in 0..40 {
        let game = play(spell_fuzz_setup(2, seed), seed)?;
        for event in &game.events {
            let name = match event {
                Event::AbilityPutOnStack(_) => "AbilityPutOnStack",
                Event::AbilityActivated { .. } => "AbilityActivated",
                Event::ResolutionBegan { .. } => "ResolutionBegan",
                Event::StepDone { .. } => "StepDone",
                Event::ResolutionEnded { outcome, .. } => {
                    outcomes.insert(format!("{outcome:?}"));
                    "ResolutionEnded"
                }
                Event::LifeGained { .. } => "LifeGained",
                Event::LifeLost { .. } => "LifeLost",
                Event::DamageDealt { .. } => "DamageDealt",
                Event::SpellCast { choices, .. } => {
                    announced |= !choices.targets.is_empty();
                    "SpellCast"
                }
                _ => continue,
            };
            seen.insert(name);
        }
    }
    let expected = [
        "AbilityActivated",
        "AbilityPutOnStack",
        "DamageDealt",
        "LifeGained",
        "LifeLost",
        "ResolutionBegan",
        "ResolutionEnded",
        "SpellCast",
        "StepDone",
    ];
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
    let resolved = format!("{:?}", ResolutionOutcome::Resolved);
    let fizzled = format!("{:?}", ResolutionOutcome::Fizzled);
    assert!(
        outcomes.contains(&resolved) && outcomes.contains(&fizzled),
        "{outcomes:?}"
    );
    assert!(announced, "a spell was cast with targets");
    Ok(())
}

#[test]
fn games_without_spells_hold_none_of_the_new_events_and_log_a_cast_as_it_always_was()
-> Result<(), EngineError> {
    for seed in 0..20 {
        let game = play(fuzz_setup(2, seed), seed)?;
        for event in &game.events {
            assert!(
                !matches!(
                    event,
                    Event::AbilityPutOnStack(_)
                        | Event::AbilityActivated { .. }
                        | Event::AbilityTriggered(_)
                        | Event::TriggersOrdered { .. }
                        | Event::TriggerRemoved { .. }
                        | Event::DelayedTriggerCreated(_)
                        | Event::DelayedTriggerExpired { .. }
                        | Event::ResolutionBegan { .. }
                        | Event::ChoiceMade { .. }
                        | Event::StepDone { .. }
                        | Event::ResolutionEnded { .. }
                        | Event::ZoneChanged { .. }
                        | Event::LifeGained { .. }
                        | Event::LifeLost { .. }
                        | Event::CountersChanged(_)
                        | Event::TokenCreated(_)
                        | Event::LibraryLooked(_)
                        | Event::CardsFound(_)
                        | Event::Scried(_)
                        | Event::Revealed(_)
                ),
                "seed {seed}: {event:?}"
            );
            if let Event::SpellCast { .. } = event {
                // Without announcements the event has the two fields it always had, and the only choice it carries is the
                // mana the cast spent.
                let json = serde_json::to_value(event).unwrap();
                let fields = json["SpellCast"].as_object().unwrap();
                let choices = fields.get("choices").and_then(|c| c.as_object());
                assert_eq!(fields.len(), 2 + usize::from(choices.is_some()), "{json}");
                assert!(
                    choices.is_none_or(|c| c.len() == 1 && c.contains_key("spent")),
                    "{json}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn a_spell_with_targets_logs_what_was_announced_in_the_spell_cast_event() {
    let scenario = Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(pinger("Pinger", 0))]);
    let setup = scenario.setup().unwrap();
    let mut driver = mtg_testkit::Driver::new(Engine::new(setup).unwrap()).unwrap();
    let at = driver.target_player(1);
    let events = driver.cast_at("Bolt", &[at]).unwrap();
    let cast = events
        .iter()
        .find(|e| matches!(e, Event::SpellCast { .. }))
        .unwrap();
    let json = serde_json::to_value(cast).unwrap();
    assert_eq!(
        json["SpellCast"]["choices"]["targets"],
        serde_json::json!([[{ "Player": 1 }]])
    );
    assert_eq!(
        json["SpellCast"]["player"],
        serde_json::json!(PlayerId(0).get())
    );
    // And it reads back to the same event.
    let again: Event = serde_json::from_value(json).unwrap();
    assert_eq!(&again, cast);
}
