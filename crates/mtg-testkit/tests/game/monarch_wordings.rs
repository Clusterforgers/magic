//! Goal: the Monarch wordings work at tables of two, three and four seats: a trigger on becoming the monarch fires for the players its pattern names and for no others; a static ability under "you're the monarch", "an opponent is the monarch" or "there is no monarch" holds exactly while that is true; "if there is no monarch" lets a trigger resolve only while nobody holds the designation; creatures the monarch controls cannot block a creature that says so, and only while they are the monarch's; and an attack at the monarch triggers the Spear of Bashenga for the monarch's permanents alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::combat::to_blockers;
use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island, monarcher, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An enchantment whose only ability is `json`.
fn enchantment(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_rules(ability(json)),
    )
}

/// An enchantment that gains its controller 1 life whenever `when` happens.
fn watcher(when: &str) -> Arc<CardDefinition> {
    enchantment(
        "Watcher",
        &format!(r#"{{"Triggered":{{"when":{when},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#),
    )
}

/// An enchantment that gives creatures its controller controls +1/+1 while `condition` holds.
fn banner(condition: &str) -> Arc<CardDefinition> {
    enchantment(
        "Banner",
        &format!(
            r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}],"condition":{condition}}}}}"#
        ),
    )
}

/// Each seat holds a Crown (an instant that makes its caster the monarch) and an Island; seat 0 also holds `extra`
/// on its battlefield, and every other seat a Wall (0/5) of its own.
fn table(seats: usize, extra: &[PresetPermanent]) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut permanents = vec![ready(island())];
        if seat == 0 {
            permanents.extend(extra.iter().cloned());
        } else {
            permanents.push(ready(free_creature(&format!("Wall {seat}"), 0, 5)));
        }
        scenario = scenario
            .hand(seat, [monarcher(&format!("Crown {seat}"))])
            .battlefield(seat, permanents);
    }
    scenario
}

fn monarch(driver: &Driver) -> Option<usize> {
    driver.state().monarch().map(PlayerId::index)
}

/// Seat `seat` casts its Crown on its own turn and everything it triggers resolves.
fn crown(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    // Seat 0 plays the first turn of a round, seat 1 the second.
    driver.idle_until(u32::try_from(seat).unwrap() + 1, Step::Main1)?;
    driver.cast(&format!("Crown {seat}"))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

#[rstest]
#[case::you_when_i_become_it(r#"{"BecomesMonarch":{"who":"You"}}"#, 0, true)]
#[case::you_when_another_seat_becomes_it(r#"{"BecomesMonarch":{"who":"You"}}"#, 1, false)]
#[case::an_opponent_when_another_seat_becomes_it(
    r#"{"BecomesMonarch":{"who":"Opponents"}}"#,
    1,
    true
)]
#[case::an_opponent_when_i_become_it(r#"{"BecomesMonarch":{"who":"Opponents"}}"#, 0, false)]
#[case::a_player_when_i_become_it(r#"{"BecomesMonarch":{}}"#, 0, true)]
#[case::a_player_when_another_seat_becomes_it(r#"{"BecomesMonarch":{}}"#, 1, true)]
fn a_trigger_on_becoming_the_monarch_fires_for_the_players_its_pattern_names(
    #[case] when: &str,
    #[case] becomer: usize,
    #[case] fires: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[ready(watcher(when))]))?;
    crown(&mut driver, becomer)?;
    assert_eq!(monarch(&driver), Some(becomer));
    assert_eq!(life(&driver, 0), 20 + i32::from(fires));
    Ok(())
}

#[rstest]
fn a_designation_that_does_not_change_hands_triggers_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let when = r#"{"BecomesMonarch":{"who":"You"}}"#;
    let mut driver = Driver::from_scenario(
        table(seats, &[ready(watcher(when)), ready(island())])
            .hand(0, [monarcher("Crown 0"), monarcher("Second Crown")]),
    )?;
    crown(&mut driver, 0)?;
    driver.cast("Second Crown")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver, 0), 21, "becoming it again is no event");
    Ok(())
}

#[rstest]
#[case::you_with_nobody(r#"{"Monarch":"You"}"#, None, 0)]
#[case::you_as_the_monarch(r#"{"Monarch":"You"}"#, Some(0), 1)]
#[case::you_with_another_the_monarch(r#"{"Monarch":"You"}"#, Some(1), 0)]
#[case::an_opponent_with_nobody(r#"{"Monarch":"Opponents"}"#, None, 0)]
#[case::an_opponent_with_me_the_monarch(r#"{"Monarch":"Opponents"}"#, Some(0), 0)]
#[case::an_opponent_as_the_monarch(r#"{"Monarch":"Opponents"}"#, Some(1), 1)]
#[case::nobody_with_nobody(r#"{"Not":{"Monarch":"EachPlayer"}}"#, None, 1)]
#[case::nobody_with_me_the_monarch(r#"{"Not":{"Monarch":"EachPlayer"}}"#, Some(0), 0)]
#[case::nobody_with_another_the_monarch(r#"{"Not":{"Monarch":"EachPlayer"}}"#, Some(1), 0)]
fn a_static_ability_under_a_monarch_condition_holds_exactly_while_it_is_true(
    #[case] condition: &str,
    #[case] designated: Option<usize>,
    #[case] bonus: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wall = ready(free_creature("Mine", 0, 5));
    let mut driver = Driver::from_scenario(table(seats, &[wall, ready(banner(condition))]))?;
    if let Some(seat) = designated {
        crown(&mut driver, seat)?;
    }
    assert_eq!(monarch(&driver), designated);
    assert_eq!(power(&driver, "Mine"), bonus);
    Ok(())
}

#[rstest]
fn a_static_ability_follows_the_designation_when_it_changes_hands(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wall = ready(free_creature("Mine", 0, 5));
    let condition = r#"{"Monarch":"You"}"#;
    let mut driver = Driver::from_scenario(table(seats, &[wall, ready(banner(condition))]))?;
    crown(&mut driver, 0)?;
    assert_eq!(power(&driver, "Mine"), 1);
    crown(&mut driver, 1)?;
    assert_eq!(power(&driver, "Mine"), 0, "seat 1 took the designation");
    Ok(())
}

/// Seat 0 holds the Spear of Bashenga, with five Plains for it.
fn spear_table(seats: usize) -> Scenario {
    let lands: Vec<_> = (0..5).map(|_| ready(plains())).collect();
    table(seats, &lands).hand(
        0,
        [monarcher("Crown 0"), card("The Spear of Bashenga").unwrap()],
    )
}

fn changes(driver: &Driver) -> Vec<usize> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::MonarchChanged { player } => Some(player.index()),
            _ => None,
        })
        .collect()
}

fn spear_triggers(driver: &Driver) -> usize {
    let spear = driver.id_of("The Spear of Bashenga").unwrap();
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered(t) if t.source.id == spear))
        .count()
}

#[rstest]
fn the_spear_makes_its_controller_the_monarch_while_there_is_none(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(spear_table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    driver.cast("The Spear of Bashenga")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(monarch(&driver), Some(0));
    assert_eq!(changes(&driver), [0]);
    Ok(())
}

#[rstest]
fn the_spear_does_not_trigger_while_another_seat_is_the_monarch(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(spear_table(seats))?;
    crown(&mut driver, 1)?;
    // Seat 0 has the next turn of its own, where nobody has taken the designation from seat 1.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    driver.cast("The Spear of Bashenga")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(monarch(&driver), Some(1));
    assert_eq!(changes(&driver), [1]);
    assert_eq!(
        spear_triggers(&driver),
        0,
        "the condition fails as it enters"
    );
    Ok(())
}

/// Seat 0's Raider carries the Spear and can attack on its first turn; every other seat has a Wall of its own.
fn attack_table(seats: usize) -> Scenario {
    let mut scenario = table(
        seats,
        &[
            ready(free_creature("Raider", 2, 2)),
            attached_to(ready(card("The Spear of Bashenga").unwrap()), 0, 1),
            ready(card("Azure Fleet Admiral").unwrap()),
        ],
    );
    // Seat 1 also has a tapped Golem; with three seats or more seat 2 has one too, which the monarch does not control.
    scenario = scenario.battlefield(
        1,
        vec![
            ready(island()),
            ready(free_creature("Wall 1", 0, 5)),
            tapped(free_creature("Golem 1", 1, 1)),
        ],
    );
    if seats > 2 {
        scenario = scenario.battlefield(
            2,
            vec![
                ready(island()),
                ready(free_creature("Wall 2", 0, 5)),
                tapped(free_creature("Golem 2", 1, 1)),
            ],
        );
    }
    scenario
}

/// Seat `seat` casts its Crown the first time it has priority in the first main step of turn 1 and it resolves.
fn instant_crown(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    driver.idle_until(1, Step::Main1)?;
    for _ in 0..8 {
        if driver.decision()?.player == pid(seat) {
            break;
        }
        driver.pass()?;
    }
    driver.cast(&format!("Crown {seat}"))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 1 becomes the monarch in seat 0's first main step, and seat 0 then attacks with `attackers`.
fn attack_at(driver: &mut Driver, attackers: &[(&str, usize)]) -> Result<(), ScriptError> {
    instant_crown(driver, 1)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(attackers)?;
    Ok(())
}

#[rstest]
fn an_attack_at_the_monarch_destroys_a_tapped_permanent_the_monarch_controls(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(attack_table(seats))?;
    attack_at(&mut driver, &[("Raider", 1)])?;
    for _ in 0..12 {
        if let DecisionKind::ChooseTargets(_) = &driver.decision()?.kind {
            let golem = driver.target_named("Golem 1")?;
            driver.choose_targets(&[golem])?;
        } else if driver.state().stack().is_empty() {
            break;
        } else {
            driver.pass()?;
        }
    }
    assert!(
        driver.id_of("Golem 1").is_err(),
        "the monarch's tapped permanent was destroyed"
    );
    if seats > 2 {
        assert!(driver.id_of("Golem 2").is_ok(), "another seat's was not");
    }
    Ok(())
}

#[rstest]
fn an_attack_at_a_seat_that_is_not_the_monarch_does_not_trigger_the_spear(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(attack_table(seats))?;
    attack_at(&mut driver, &[("Raider", 2)])?;
    assert_eq!(spear_triggers(&driver), 0);
    assert!(driver.id_of("Golem 2").is_ok());
    Ok(())
}

#[rstest]
#[case::the_monarchs_creature_cannot_block(1, false)]
#[case::another_seats_creature_can(2, true)]
fn creatures_the_monarch_controls_cannot_block_the_admiral(
    #[case] defender: usize,
    #[case] may_block: bool,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(attack_table(seats))?;
    attack_at(&mut driver, &[("Azure Fleet Admiral", defender)])?;
    to_blockers(&mut driver, defender)?;
    let wall = format!("Wall {defender}");
    let blocked = driver.block(&[(wall.as_str(), "Azure Fleet Admiral")]);
    assert_eq!(blocked.is_ok(), may_block, "{blocked:?}");
    Ok(())
}

#[rstest]
fn the_admiral_is_blocked_by_the_monarch_once_the_designation_has_moved_on(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(attack_table(seats))?;
    // Seat 1 takes it and seat 0 takes it back, so seat 1's Wall may block.
    instant_crown(&mut driver, 1)?;
    instant_crown(&mut driver, 0)?;
    assert_eq!(monarch(&driver), Some(0));
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Azure Fleet Admiral", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Wall 1", "Azure Fleet Admiral")])?;
    Ok(())
}

#[rstest]
fn the_log_of_a_game_with_monarch_wordings_replays_to_the_state_it_was_played_to(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let when = r#"{"BecomesMonarch":{}}"#;
    let scenario = table(
        seats,
        &[ready(watcher(when)), ready(banner(r#"{"Monarch":"You"}"#))],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    crown(&mut driver, 0)?;
    crown(&mut driver, 1)?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(
        life(&driver, 0),
        22,
        "both designations triggered the Watcher"
    );
    Ok(())
}
