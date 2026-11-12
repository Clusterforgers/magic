//! Goal: the pumps, group changes and animations the compiler reads (a group with a "non-Elf" filter, the group of a targeted player for a turn and until the caster's next turn, a base size under later pumps, lost abilities and a new look, several targets, a target that leaves, a land that becomes a creature with the size it sets and an artifact that keeps its size) play as the cards say at tables of two, three and four seats, last exactly their duration, and replay to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::{corpus, resolve_the_stack};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp, wastes};
use mtg_testkit::pump_cards::unsummon;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free 3/3 creature of this creature type.
fn kin(name: &str, kind: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_power_toughness(3, 3),
    )
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap()).contains(&keyword)
}

fn types(driver: &Driver, name: &str) -> (bool, bool, bool) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (
        found.types.contains(CardType::Creature),
        found.types.contains(CardType::Land),
        found.types.contains(CardType::Artifact),
    )
}

fn assert_replays(driver: &Driver, scenario: &Scenario) {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds `hand` and controls `mine`; every other seat controls a 3/3 named "Guard <seat>".
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, hand).battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Guard {seat}"), 3, 3))],
        );
    }
    scenario
}

#[rstest]
fn a_group_without_one_creature_type_shrinks_and_recovers_at_the_end_of_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(kin("Elf Guard", "Elf")), ready(kin("Brute", "Ogre"))];
    mine.extend((0..4).map(|_| ready(swamp())));
    let scenario = table(seats, vec![corpus("Eyeblight Massacre")], mine).battlefield(
        1,
        vec![
            ready(free_creature("Guard 1", 3, 3)),
            ready(kin("Rival Elf", "Elf")),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Eyeblight Massacre")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Elf Guard"), (3, 3), "an Elf is left alone");
    assert_eq!(stats(&driver, "Rival Elf"), (3, 3));
    assert_eq!(stats(&driver, "Brute"), (1, 1));
    assert_eq!(stats(&driver, "Guard 1"), (1, 1), "every seat is shrunk");
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(stats(&driver, "Brute"), (3, 3), "until end of turn");
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn the_creatures_of_a_player_stay_small_until_the_casters_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(free_creature("Mine", 3, 3))];
    mine.extend((0..2).map(|_| ready(island())));
    let scenario = table(seats, vec![corpus("Mass Diminish")], mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let victim = driver.target_player(1);
    driver.cast_at("Mass Diminish", &[victim])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Guard 1"), (1, 1));
    assert_eq!(
        stats(&driver, "Mine"),
        (3, 3),
        "the caster's creatures keep their size"
    );
    if seats > 2 {
        assert_eq!(
            stats(&driver, "Guard 2"),
            (3, 3),
            "another player is not named"
        );
    }
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(stats(&driver, "Guard 1"), (1, 1), "through the other turns");
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Upkeep)?;
    assert_eq!(
        stats(&driver, "Guard 1"),
        (3, 3),
        "the caster's next turn has begun"
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::the_pump_first(true)]
#[case::the_base_size_first(false)]
fn a_base_size_and_a_pump_stack_in_the_layers_in_either_order(
    #[values(2, 3, 4)] seats: usize,
    #[case] pump_first: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(free_creature("Mine", 5, 5))];
    mine.extend((0..3).map(|_| ready(island())));
    mine.push(ready(forest()));
    let scenario = table(
        seats,
        vec![corpus("Flatline"), corpus("Giant Growth")],
        mine,
    )
    .battlefield(
        1,
        vec![
            ready(free_creature("Guard 1", 3, 3)),
            ready(mtg_testkit::cards::forest()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let growth = |driver: &mut Driver| -> Result<(), ScriptError> {
        let target = driver.target_named("Guard 1")?;
        driver.cast_at("Giant Growth", &[target])?;
        resolve_the_stack(driver)
    };
    if pump_first {
        growth(&mut driver)?;
    }
    driver.cast("Flatline")?;
    resolve_the_stack(&mut driver)?;
    if !pump_first {
        growth(&mut driver)?;
    }
    assert_eq!(stats(&driver, "Guard 1"), (3, 4), "0/1 base, then +3/+3");
    assert_eq!(stats(&driver, "Mine"), (5, 5), "only the opponents are set");
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn a_creature_loses_its_abilities_and_looks_like_a_frog_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let flier = Arc::new(
        CardDefinition::new("Sky Guard")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_rules(mtg_cards::ability::Ability::Keyword(Keyword::Flying)),
    );
    let mut mine = vec![ready(flier)];
    mine.extend((0..2).map(|_| ready(island())));
    let scenario = table(seats, vec![corpus("Turn to Frog")], mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert!(has(&driver, "Sky Guard", &Keyword::Flying));
    let target = driver.target_named("Sky Guard")?;
    driver.cast_at("Turn to Frog", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Sky Guard"), (1, 1));
    assert!(!has(&driver, "Sky Guard", &Keyword::Flying));
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(stats(&driver, "Sky Guard"), (4, 4));
    assert!(
        has(&driver, "Sky Guard", &Keyword::Flying),
        "the abilities return"
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn a_pump_does_nothing_to_a_creature_that_left_before_it_resolved(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![
        ready(corpus("Korozda Guildmage")),
        ready(free_creature("Bear", 2, 2)),
    ];
    mine.extend([swamp(), forest(), swamp(), forest()].map(ready));
    let scenario = table(seats, vec![], mine)
        .hand(1, vec![unsummon("Unsummon")])
        .battlefield(
            1,
            vec![ready(island()), ready(free_creature("Guard 1", 3, 3))],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let index = *driver
        .offered_abilities("Korozda Guildmage")?
        .first()
        .unwrap();
    driver.begin_ability("Korozda Guildmage", index)?;
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    driver.pass()?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Unsummon", &[bear])?;
    resolve_the_stack(&mut driver)?;
    assert!(
        driver.id_of("Bear").is_err(),
        "the Bear went back to its owner's hand"
    );
    assert!(driver.state().effects().is_empty(), "no pump began");
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn up_to_four_targets_gain_indestructible_together(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![
        ready(free_creature("Bear", 2, 2)),
        ready(free_creature("Wolf", 2, 2)),
    ];
    mine.extend([plains(), plains()].map(ready));
    let scenario = table(seats, vec![corpus("Invisible Force Field")], mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let targets = [driver.target_named("Bear")?, driver.target_named("Wolf")?];
    driver.cast_at("Invisible Force Field", &targets)?;
    resolve_the_stack(&mut driver)?;
    for name in ["Bear", "Wolf"] {
        assert!(has(&driver, name, &Keyword::Indestructible), "{name}");
    }
    assert!(!has(&driver, "Guard 1", &Keyword::Indestructible));
    driver.idle_until(2, Step::Upkeep)?;
    assert!(
        !has(&driver, "Bear", &Keyword::Indestructible),
        "until end of turn"
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn a_land_becomes_a_creature_with_the_size_it_sets_and_stays_a_land(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(corpus("Llanowar Loamspeaker")),
        ready(mtg_testkit::cards::forest()),
    ];
    let scenario = table(seats, vec![], mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert_eq!(types(&driver, "Forest"), (false, true, false));
    let index = *driver
        .offered_abilities("Llanowar Loamspeaker")?
        .last()
        .unwrap();
    driver.begin_ability("Llanowar Loamspeaker", index)?;
    let target = driver.target_named("Forest")?;
    driver.choose_targets(&[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        types(&driver, "Forest"),
        (true, true, false),
        "a land creature"
    );
    assert_eq!(stats(&driver, "Forest"), (3, 3));
    assert!(has(&driver, "Forest", &Keyword::Haste));
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(
        types(&driver, "Forest"),
        (false, true, false),
        "a plain land again"
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn a_permanent_becomes_an_artifact_and_keeps_its_size(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(corpus("Liquimetal Torque")),
        ready(free_creature("Bear", 2, 2)),
        ready(wastes()),
    ];
    let scenario = table(seats, vec![], mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let index = *driver
        .offered_abilities("Liquimetal Torque")?
        .last()
        .unwrap();
    driver.begin_ability("Liquimetal Torque", index)?;
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        types(&driver, "Bear"),
        (true, false, true),
        "an artifact creature"
    );
    assert_eq!(stats(&driver, "Bear"), (2, 2), "it keeps the size it has");
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(types(&driver, "Bear"), (true, false, false));
    assert_replays(&driver, &scenario);
    Ok(())
}
