//! Goal: Reconfigure plays as the rules say, at tables of two, three and four seats: an Equipment creature is offered the attach ability while it is attached to no creature and the unattach ability while it is attached to one, both at sorcery speed for the reconfigure cost; attaching asks for another creature its controller controls (never the Equipment itself); it is a creature while unattached and none while attached, and a creature again once unattached; it comes off a host that stops being a creature, and an Equipment that loses its abilities while attached is a creature and falls off; the corpus cards (Lizard Blades, Bronzeplate Boar, Armguard Familiar) keep what they print while they are creatures and give what they say to the creature they equip; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{forest, free_creature};
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

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

fn has_keyword(driver: &Driver, name: &str, keyword: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|held| held.name() == keyword)
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 has `cards` in play, a Bear (2/2), a Wolf (3/3) and an Elk (1/4) and `mana` Forests; seat 1 has an Ogre.
fn table(seats: usize, cards: &[&str], mana: usize) -> Scenario {
    let mut permanents: Vec<_> = cards
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect();
    permanents.extend([
        ready(free_creature("Grizzly Bears", 2, 2)),
        ready(free_creature("Wolf", 3, 3)),
        ready(free_creature("Elk", 1, 4)),
    ]);
    permanents.extend((0..mana).map(|_| ready(forest())));
    Scenario::new(seats)
        .battlefield(0, permanents)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
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

/// Attaches `source` to `target` with its attach ability (the first it adds) and lets it resolve.
fn attach(driver: &mut Driver, source: &str, target: &str) -> Result<(), ScriptError> {
    driver.begin_ability(source, 0)?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

/// Unattaches `source` with its unattach ability (the second it adds), which has no target.
fn unattach(driver: &mut Driver, source: &str) -> Result<(), ScriptError> {
    driver.begin_ability(source, 1)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

#[rstest]
fn the_attach_ability_is_offered_while_unattached_and_the_unattach_ability_while_attached(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Lizard Blades"], 6);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(driver.offered_abilities("Lizard Blades")?, [0]);
    attach(&mut driver, "Lizard Blades", "Wolf")?;
    assert_eq!(host_name(&driver, "Lizard Blades").as_deref(), Some("Wolf"));
    assert_eq!(driver.offered_abilities("Lizard Blades")?, [1]);
    unattach(&mut driver, "Lizard Blades")?;
    assert_eq!(host_name(&driver, "Lizard Blades"), None);
    assert_eq!(driver.offered_abilities("Lizard Blades")?, [0]);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn attaching_asks_for_another_creature_its_controller_controls(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Lizard Blades"], 3))?;
    driver.begin_ability("Lizard Blades", 0)?;
    assert_eq!(offered(&driver), ["Elk", "Grizzly Bears", "Wolf"]);
    Ok(())
}

#[rstest]
fn the_equipment_is_a_creature_while_unattached_and_none_while_attached(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Lizard Blades"], 6))?;
    assert!(is_creature(&driver, "Lizard Blades"));
    assert_eq!(stats(&driver, "Lizard Blades"), (1, 1));
    assert!(!has_keyword(&driver, "Wolf", "Double strike"));
    attach(&mut driver, "Lizard Blades", "Wolf")?;
    assert!(!is_creature(&driver, "Lizard Blades"));
    assert!(has_keyword(&driver, "Wolf", "Double strike"));
    unattach(&mut driver, "Lizard Blades")?;
    assert!(is_creature(&driver, "Lizard Blades"));
    assert_eq!(stats(&driver, "Lizard Blades"), (1, 1));
    assert!(!has_keyword(&driver, "Wolf", "Double strike"));
    Ok(())
}

#[rstest]
#[case::the_first_main_step(Step::Main1, true)]
#[case::the_second_main_step(Step::Main2, true)]
#[case::the_upkeep(Step::Upkeep, false)]
#[case::the_start_of_combat(Step::BeginCombat, false)]
#[case::the_end_step(Step::End, false)]
fn reconfigure_is_offered_at_sorcery_timing(
    #[values(2, 3, 4)] seats: usize,
    #[case] step: Step,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, &["Lizard Blades"], 3).at(1, 0, step))?;
    assert_eq!(
        driver.offered_abilities("Lizard Blades")?.len(),
        usize::from(expected)
    );
    Ok(())
}

#[rstest]
#[case::lizard_blades_short("Lizard Blades", 1, false)]
#[case::lizard_blades_enough("Lizard Blades", 2, true)]
#[case::armguard_familiar_short("Armguard Familiar", 3, false)]
#[case::armguard_familiar_enough("Armguard Familiar", 4, true)]
#[case::bronzeplate_boar_short("Bronzeplate Boar", 4, false)]
#[case::bronzeplate_boar_enough("Bronzeplate Boar", 5, true)]
fn reconfigure_is_offered_when_its_cost_can_be_paid(
    #[values(2, 3)] seats: usize,
    #[case] name: &str,
    #[case] mana: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, &[name], mana))?;
    assert_eq!(driver.offered_abilities(name)?.len(), usize::from(expected));
    Ok(())
}

#[rstest]
fn an_equipment_that_loses_its_abilities_while_attached_keeps_the_layer_4_effect_that_applied_before(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Abilities are lost in layer 6, after the layer 4 effect of the static ability has made the Equipment no creature.
    let mut driver = Driver::from_scenario(table(seats, &["Lizard Blades"], 3))?;
    attach(&mut driver, "Lizard Blades", "Wolf")?;
    let blades = driver.id_of("Lizard Blades")?;
    driver.add_effect(
        driver.on(blades),
        Modification::LoseAbilities,
        Duration::Forever,
    )?;
    driver.pass()?;
    assert_eq!(host_name(&driver, "Lizard Blades").as_deref(), Some("Wolf"));
    assert!(!is_creature(&driver, "Lizard Blades"));
    assert!(!has_keyword(&driver, "Wolf", "Double strike"));
    Ok(())
}

#[rstest]
fn a_host_that_stops_being_a_creature_loses_its_equipment_which_is_a_creature_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Lizard Blades"], 3))?;
    attach(&mut driver, "Lizard Blades", "Wolf")?;
    let wolf = driver.id_of("Wolf")?;
    driver.add_effect(
        driver.on(wolf),
        Modification::RemoveType(CardType::Creature),
        Duration::Forever,
    )?;
    driver.pass()?;
    assert_eq!(host_name(&driver, "Lizard Blades"), None);
    assert!(is_creature(&driver, "Lizard Blades"));
    Ok(())
}

#[rstest]
fn the_corpus_cards_keep_what_they_print_as_creatures_and_give_what_they_say_to_the_creature_they_equip(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("Lizard Blades", (1, 1), (0, 0), "Double strike"),
        ("Bronzeplate Boar", (3, 2), (3, 2), "Trample"),
        ("Armguard Familiar", (2, 1), (2, 1), "Ward")
    )]
    expected: (&str, (i32, i32), (i32, i32), &str),
) -> Result<(), ScriptError> {
    let (name, printed, (power, toughness), keyword) = expected;
    let scenario = || table(seats, &[name], 24);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert!(is_creature(&driver, name));
    assert_eq!(stats(&driver, name), printed);
    assert!(
        has_keyword(&driver, name, keyword),
        "{name} prints {keyword}"
    );
    attach(&mut driver, name, "Wolf")?;
    assert!(!is_creature(&driver, name));
    assert_eq!(stats(&driver, "Wolf"), (3 + power, 3 + toughness));
    assert!(has_keyword(&driver, "Wolf", keyword));
    // It moves by being unattached first and attached again, one payment each.
    unattach(&mut driver, name)?;
    attach(&mut driver, name, "Elk")?;
    assert_eq!(stats(&driver, "Wolf"), (3, 3));
    assert_eq!(stats(&driver, "Elk"), (1 + power, 4 + toughness));
    assert!(!has_keyword(&driver, "Wolf", keyword));
    unattach(&mut driver, name)?;
    assert!(is_creature(&driver, name));
    assert_eq!(stats(&driver, name), printed);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn the_cards_are_equipment_creatures() {
    let cards: Vec<Arc<CardDefinition>> =
        ["Lizard Blades", "Bronzeplate Boar", "Armguard Familiar"]
            .iter()
            .map(|name| card(name).unwrap())
            .collect();
    for card in cards {
        assert!(card.types.contains(&CardType::Creature), "{}", card.name);
        assert!(
            card.subtypes.iter().any(|s| s.0 == "Equipment"),
            "{}",
            card.name
        );
    }
}
