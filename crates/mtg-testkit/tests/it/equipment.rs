//! Goal: Equip plays as the rules say, at tables of two, three and four seats: the ability is offered only to the controller of the Equipment, at sorcery speed and when the cost can be paid and a creature is there; it asks for a creature its controller controls (never an opponent's, never the Equipment itself); it pays its cost, waits on the stack and attaches when it resolves; every Equipment moves onto every creature in turn, a creature that is already equipped keeps its Equipment and its timestamp, and the bonus follows the Equipment; the ability does nothing when its creature or its Equipment is gone as it resolves, an Equipment stays and comes off when its creature leaves, and its bonus ends with it; a creature that changes control keeps its Equipment, and the new controller of an Equipment may equip it; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::attach_cards::{heavy_plate, iron_blade};
use mtg_testkit::cards::{burn, cost, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

fn host_name(driver: &Driver, name: &str) -> Option<String> {
    let id = driver.id_of(name).ok()?;
    driver.state().host_of(id).map(|host| name_of(driver, host))
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn timestamp(driver: &Driver, name: &str) -> u64 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .timestamp()
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn bonesplitter() -> Arc<CardDefinition> {
    card("Bonesplitter").unwrap()
}

/// `{1}` instant: destroy target artifact.
fn shatter() -> Arc<CardDefinition> {
    let destroy = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Shatter")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(serde_json::from_str::<Ability>(destroy).unwrap()),
    )
}

/// Seat 0 has `equipment` in play, a Bear (2/2), a Wolf (3/3) and an Elk (1/4) and `mana` Forests; seat 1 has an Ogre (3/4),
/// a Mountain, a Forest and a hand of one Shock and one Shatter. Seats beyond the second have nothing.
fn table(seats: usize, equipment: &[Arc<CardDefinition>], mana: usize) -> Scenario {
    let mut permanents: Vec<_> = equipment.iter().cloned().map(ready).collect();
    permanents.extend([
        ready(free_creature("Grizzly Bears", 2, 2)),
        ready(free_creature("Wolf", 3, 3)),
        ready(free_creature("Elk", 1, 4)),
    ]);
    permanents.extend((0..mana).map(|_| ready(forest())));
    Scenario::new(seats)
        .battlefield(0, permanents)
        .battlefield(
            1,
            [
                ready(free_creature("Ogre", 3, 4)),
                ready(mountain()),
                ready(forest()),
            ],
        )
        .hand(1, [burn("Shock", 3), shatter()])
}

/// The same table with a Mountain and a Spark for seat 0, so it can kill one of its own creatures.
fn burning(seats: usize, equipment: &[Arc<CardDefinition>], mana: usize) -> Scenario {
    let mut permanents: Vec<_> = equipment.iter().cloned().map(ready).collect();
    permanents.extend([
        ready(free_creature("Grizzly Bears", 2, 2)),
        ready(free_creature("Wolf", 3, 3)),
        ready(free_creature("Elk", 1, 4)),
        ready(mountain()),
    ]);
    permanents.extend((0..mana).map(|_| ready(forest())));
    table(seats, equipment, mana)
        .battlefield(0, permanents)
        .hand(0, [burn("Spark", 2)])
}

/// The names the pending target decision offers, sorted.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("the ability asks for its target");
    };
    let mut names: Vec<String> = options
        .legal
        .iter()
        .map(|target| name_of(driver, target.object().unwrap().id))
        .collect();
    names.sort();
    names
}

/// Activates the first ability of `source` at `target` and pays for it, leaving it on the stack.
fn start_equip(driver: &mut Driver, source: &str, target: &str) -> Result<(), ScriptError> {
    driver.begin_activation(source)?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

/// Equips `source` to `target` and lets the ability resolve.
fn equip(driver: &mut Driver, source: &str, target: &str) -> Result<(), ScriptError> {
    start_equip(driver, source, target)?;
    settle(driver)
}

fn attached_events(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Attached { .. }))
        .count()
}

// Asking and paying

#[rstest]
fn equip_asks_for_a_creature_its_controller_controls_and_never_the_equipment(
    #[values(2, 3, 4)] seats: usize,
    #[values("Bonesplitter", "Iron Blade", "Heavy Plate")] equipment: &str,
) -> Result<(), ScriptError> {
    let cards = [
        bonesplitter(),
        iron_blade().unwrap(),
        heavy_plate().unwrap(),
    ];
    let mut driver = Driver::from_scenario(table(seats, &cards, 4))?;
    driver.begin_activation(equipment)?;
    assert_eq!(offered(&driver), ["Elk", "Grizzly Bears", "Wolf"]);
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        unreachable!()
    };
    assert_eq!((options.min, options.max), (1, 1));
    assert!(options.can_cancel, "backing out of an activation is free");
    Ok(())
}

#[rstest]
fn equip_pays_its_cost_waits_on_the_stack_and_attaches_when_it_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &[bonesplitter()], 2);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    start_equip(&mut driver, "Bonesplitter", "Wolf")?;
    // The cost is paid, the ability is an object on the stack and nothing is attached yet.
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    assert_eq!(stats(&driver, "Wolf"), (3, 3));
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.card().name == "Forest" && object.permanent().unwrap().tapped()
        })
        .count();
    assert_eq!(tapped, 1, "Equip {{1}} taps one Forest");
    settle(&mut driver)?;
    assert_eq!(host_name(&driver, "Bonesplitter").as_deref(), Some("Wolf"));
    assert_eq!(stats(&driver, "Wolf"), (5, 3));
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    replays(&driver, &setup);
    Ok(())
}

// When it is offered

#[rstest]
#[case::the_first_main_step(1, 0, Step::Main1, true)]
#[case::the_second_main_step(1, 0, Step::Main2, true)]
#[case::the_upkeep(1, 0, Step::Upkeep, false)]
#[case::the_start_of_combat(1, 0, Step::BeginCombat, false)]
#[case::the_end_of_combat(1, 0, Step::EndCombat, false)]
#[case::the_end_step(1, 0, Step::End, false)]
fn equip_is_offered_at_sorcery_timing_in_the_turn_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] turn: u32,
    #[case] active: usize,
    #[case] step: Step,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, &[bonesplitter()], 3).at(turn, active, step))?;
    assert_eq!(
        driver.offered_abilities("Bonesplitter")?.len(),
        usize::from(expected)
    );
    Ok(())
}

#[rstest]
fn equip_is_not_offered_in_the_turn_of_another_player(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 has priority first in its own main step; seat 0 is handed priority after it passes.
    let mut driver =
        Driver::from_scenario(table(seats, &[bonesplitter()], 3).at(2, 1, Step::Main1))?;
    assert_eq!(driver.decision()?.player, pid(1));
    for _ in 0..seats {
        if driver.decision()?.player == pid(0) {
            assert_eq!(
                driver.offered_abilities("Bonesplitter")?,
                Vec::<usize>::new()
            );
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("seat 0 was never handed priority");
}

#[test]
fn equip_is_not_offered_while_anything_is_on_the_stack() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(burning(2, &[bonesplitter()], 3))?;
    assert_eq!(driver.offered_abilities("Bonesplitter")?.len(), 1);
    let target = driver.target_named("Ogre")?;
    driver.cast_at("Spark", &[target])?;
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(
        driver.offered_abilities("Bonesplitter")?,
        Vec::<usize>::new()
    );
    settle(&mut driver)?;
    assert_eq!(driver.offered_abilities("Bonesplitter")?.len(), 1);
    Ok(())
}

#[rstest]
#[case::enough_mana_for_one("Bonesplitter", 1, true)]
#[case::none_for_one("Bonesplitter", 0, false)]
#[case::enough_for_three("Heavy Plate", 3, true)]
#[case::two_short_of_three("Heavy Plate", 1, false)]
#[case::one_short_of_three("Heavy Plate", 2, false)]
fn equip_is_offered_when_its_cost_can_be_paid(
    #[values(2, 3)] seats: usize,
    #[case] equipment: &str,
    #[case] mana: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let cards = [bonesplitter(), heavy_plate().unwrap()];
    let driver = Driver::from_scenario(table(seats, &cards, mana))?;
    assert_eq!(
        driver.offered_abilities(equipment)?.len(),
        usize::from(expected)
    );
    Ok(())
}

#[test]
fn equip_is_not_offered_without_a_creature_you_control() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(bonesplitter()), ready(forest()), ready(forest())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.offered_abilities("Bonesplitter")?,
        Vec::<usize>::new()
    );
    Ok(())
}

// Moving

#[rstest]
fn every_equipment_moves_onto_every_creature_in_turn_and_the_bonus_follows(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("Bonesplitter", (2, 0)),
        ("Iron Blade", (1, 1)),
        ("Heavy Plate", (0, 3))
    )]
    equipment: (&str, (i32, i32)),
) -> Result<(), ScriptError> {
    let (name, (power, toughness)) = equipment;
    let cards = [
        bonesplitter(),
        iron_blade().unwrap(),
        heavy_plate().unwrap(),
    ];
    let scenario = || table(seats, &cards, 20);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let base = [("Grizzly Bears", (2, 2)), ("Wolf", (3, 3)), ("Elk", (1, 4))];
    // Some creature is equipped, then another, then the same one twice.
    let mut at: Option<&str> = None;
    for target in [
        "Grizzly Bears",
        "Wolf",
        "Elk",
        "Wolf",
        "Wolf",
        "Grizzly Bears",
    ] {
        let begun = driver.engine().events().len();
        let before = at.map(|_| timestamp(&driver, name));
        equip(&mut driver, name, target)?;
        let events = &driver.engine().events()[begun..];
        if at == Some(target) {
            assert_eq!(
                attached_events(events),
                0,
                "equipping the creature it equips attaches nothing"
            );
            assert_eq!(
                Some(timestamp(&driver, name)),
                before,
                "and keeps its timestamp"
            );
        } else {
            assert_eq!(attached_events(events), 1);
            assert!(before.is_none_or(|before| timestamp(&driver, name) > before));
        }
        at = Some(target);
        assert_eq!(host_name(&driver, name).as_deref(), Some(target));
        for (creature, (p, t)) in base {
            let expected = if creature == target {
                (p + power, t + toughness)
            } else {
                (p, t)
            };
            // The other Equipment is not attached, so only this one counts.
            assert_eq!(
                stats(&driver, creature),
                expected,
                "{name} on {target}: {creature}"
            );
        }
    }
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn two_equipment_on_one_creature_add_up_and_one_moving_leaves_the_other() -> Result<(), ScriptError>
{
    let cards = [bonesplitter(), iron_blade().unwrap()];
    let mut driver = Driver::from_scenario(table(2, &cards, 6))?;
    equip(&mut driver, "Bonesplitter", "Wolf")?;
    equip(&mut driver, "Iron Blade", "Wolf")?;
    assert_eq!(stats(&driver, "Wolf"), (6, 4));
    equip(&mut driver, "Iron Blade", "Elk")?;
    assert_eq!(stats(&driver, "Wolf"), (5, 3));
    assert_eq!(stats(&driver, "Elk"), (2, 5));
    Ok(())
}

// Gone as it resolves

#[rstest]
fn the_ability_does_nothing_when_its_creature_is_gone_as_it_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[bonesplitter()], 2))?;
    start_equip(&mut driver, "Bonesplitter", "Wolf")?;
    // Seat 0 passes with the ability on the stack; the next seat answers with a Shock on the Wolf.
    driver.pass()?;
    let mut shocked = false;
    for _ in 0..seats {
        if driver.decision()?.player == pid(1) {
            let wolf = driver.target_named("Wolf")?;
            driver.cast_at("Shock", &[wolf])?;
            shocked = true;
            break;
        }
        driver.pass()?;
    }
    assert!(shocked, "seat 1 got priority with the ability on the stack");
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ResolutionEnded {
            outcome: ResolutionOutcome::Fizzled,
            ..
        }
    )));
    assert_eq!(attached_events(events), 0);
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    assert!(driver.id_of("Wolf").is_err(), "the Wolf died");
    Ok(())
}

#[rstest]
fn the_ability_does_nothing_when_its_equipment_is_gone_as_it_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[bonesplitter()], 2))?;
    start_equip(&mut driver, "Bonesplitter", "Wolf")?;
    driver.pass()?;
    for _ in 0..seats {
        if driver.decision()?.player == pid(1) {
            let target = driver.target_named("Bonesplitter")?;
            driver.cast_at("Shatter", &[target])?;
            break;
        }
        driver.pass()?;
    }
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    assert_eq!(attached_events(&driver.engine().events()[begun..]), 0);
    assert!(
        driver.id_of("Bonesplitter").is_err(),
        "the Equipment was destroyed"
    );
    assert_eq!(stats(&driver, "Wolf"), (3, 3));
    Ok(())
}

// Leaving

#[rstest]
fn an_equipment_stays_and_comes_off_when_its_creature_leaves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || burning(seats, &[bonesplitter()], 2);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    equip(&mut driver, "Bonesplitter", "Grizzly Bears")?;
    assert_eq!(stats(&driver, "Grizzly Bears"), (4, 2));
    let bears = driver.target_named("Grizzly Bears")?;
    let begun = driver.engine().events().len();
    driver.cast_at("Spark", &[bears])?;
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Unattached { .. }))
    );
    assert!(driver.id_of("Grizzly Bears").is_err());
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    // It stays and can be attached again.
    assert_eq!(driver.offered_abilities("Bonesplitter")?.len(), 1);
    equip(&mut driver, "Bonesplitter", "Elk")?;
    assert_eq!(stats(&driver, "Elk"), (3, 4));
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn the_bonus_ends_with_an_equipment_that_is_destroyed() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, &[bonesplitter()], 2))?;
    equip(&mut driver, "Bonesplitter", "Wolf")?;
    assert_eq!(stats(&driver, "Wolf"), (5, 3));
    driver.pass()?;
    let target = driver.target_named("Bonesplitter")?;
    driver.cast_at("Shatter", &[target])?;
    settle(&mut driver)?;
    assert_eq!(stats(&driver, "Wolf"), (3, 3));
    Ok(())
}

// Control

#[test]
fn a_creature_that_changes_control_keeps_its_equipment_and_the_equipment_stays_with_its_controller()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, &[bonesplitter()], 3))?;
    equip(&mut driver, "Bonesplitter", "Wolf")?;
    let wolf = driver.id_of("Wolf")?;
    driver.add_effect(
        driver.on(wolf),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    assert_eq!(host_name(&driver, "Bonesplitter").as_deref(), Some("Wolf"));
    assert_eq!(stats(&driver, "Wolf"), (5, 3));
    // The Equipment is still seat 0's, who may equip it to a creature of its own and no longer to the Wolf.
    driver.begin_activation("Bonesplitter")?;
    assert_eq!(offered(&driver), ["Elk", "Grizzly Bears"]);
    Ok(())
}

#[test]
fn the_new_controller_of_an_equipment_equips_it_and_the_old_one_cannot() -> Result<(), ScriptError>
{
    // Seat 0 has the Equipment on its Wolf in seat 1's turn, seat 1 takes control of the Equipment and has priority.
    let mut driver = Driver::from_scenario(table(2, &[bonesplitter()], 3).at(2, 1, Step::Main1))?;
    let splitter = driver.id_of("Bonesplitter")?;
    assert_eq!(
        driver.offered_abilities("Bonesplitter")?,
        Vec::<usize>::new()
    );
    driver.add_effect(
        driver.on(splitter),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    assert_eq!(driver.offered_abilities("Bonesplitter")?.len(), 1);
    driver.begin_activation("Bonesplitter")?;
    assert_eq!(offered(&driver), ["Ogre"]);
    let ogre = driver.target_named("Ogre")?;
    driver.choose_targets(&[ogre])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(host_name(&driver, "Bonesplitter").as_deref(), Some("Ogre"));
    assert_eq!(stats(&driver, "Ogre"), (5, 4));
    Ok(())
}
