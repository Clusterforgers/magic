//! Goal: the limits on blocking play as their text says at two, three and four seats: an attacker that can't be blocked by more than one creature (Norwood Riders, Challenger Troll for each big creature of its controller, Familiar Ground for each creature of its controller) is offered with its limit and refused a second blocker, a plain attacker beside it is not limited, and the line is judged by the layers when blockers are declared; "can block only creatures with flying" (Cloud Sprite, and fliers under Dense Canopy) offers the creature only against fliers; minimum and maximum numbers of blockers from effects combine with menace, and an attacker that must have more blockers than it may have is not offered; melee given to the creatures of a player (Titania) grows each of them, and the same gift as long as the player controls their commander (Skyhunter Strike Force) is made only while a commander is on the battlefield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use common::combat::to_blockers;
use common::keywords::{Unit, card as unit_card};
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::{BlockOptions, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::format::Commander;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{PresetPermanent, characteristics};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// The corpus card of that name.
fn card(name: &str) -> Result<Arc<CardDefinition>, ScriptError> {
    mtg_testkit::corpus::card(name).map_err(|error| ScriptError::Stuck(error.to_string()))
}

fn units(list: &[Unit]) -> Vec<PresetPermanent> {
    list.iter().map(|unit| ready(unit_card(*unit))).collect()
}

/// Seat 0 about to attack with `attackers`; seat 1 holds `blockers`; the other seats hold a bystander of power 1.
fn table(
    seats: usize,
    attackers: Vec<PresetPermanent>,
    blockers: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers)
        .battlefield(1, blockers);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, units(&[("Bystander", 1, 1, "")]));
    }
    Driver::from_scenario(scenario)
}

fn options(driver: &Driver) -> BlockOptions {
    let DecisionKind::DeclareBlockers(options) = &driver.decision().unwrap().kind else {
        panic!("blockers expected");
    };
    options.clone()
}

/// The blockers the pending decision offers against the attacker `attacker`, sorted.
fn offered(driver: &Driver, attacker: &str) -> Vec<String> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Vec::new();
    };
    let mut names: Vec<String> = options
        .blockers
        .iter()
        .filter(|blocker| {
            blocker
                .can_block
                .iter()
                .any(|id| name_of(driver.state(), *id) == attacker)
        })
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect();
    names.sort();
    names
}

/// The names of the attackers the pending decision limits to some number of blockers, with the number, sorted.
fn limited(driver: &Driver) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = options(driver)
        .max_blockers
        .iter()
        .map(|(attacker, most)| (name_of(driver.state(), *attacker), *most))
        .collect();
    found.sort();
    found
}

fn pump(driver: &mut Driver, name: &str, power: i32) -> Result<(), ScriptError> {
    let id = driver.id_of(name)?;
    driver.add_effect(
        driver.on(id),
        Modification::ModifyPowerToughness {
            power,
            toughness: 0,
        },
        Duration::UntilEndOfTurn,
    )?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Declared {
    Accepted,
    TooMany { allowed: usize, got: usize },
    TooFew { needed: usize, got: usize },
}

/// Declares the blocks and says how the engine took them; a refusal leaves the state as it was and the question open.
fn declare(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<Declared, ScriptError> {
    let before = driver.engine().state_hash();
    let declared = match driver.block(blocks) {
        Ok(_) => Declared::Accepted,
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooManyBlockers {
            allowed,
            got,
        }))) => Declared::TooMany { allowed, got },
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooFewBlockers {
            needed,
            got,
        }))) => Declared::TooFew { needed, got },
        Err(other) => return Err(other),
    };
    if declared != Declared::Accepted {
        assert_eq!(driver.engine().state_hash(), before);
        driver.expect_decision(1, "declare-blockers")?;
    }
    Ok(declared)
}

const FOUR_BLOCKERS: &[Unit] = &[
    ("B1", 1, 5, ""),
    ("B2", 1, 5, ""),
    ("B3", 1, 5, ""),
    ("B4", 1, 5, ""),
];

/// Norwood Riders can't be blocked by more than one creature; a plain attacker beside it takes any number.
#[rstest]
#[case::no_block(&[], Declared::Accepted)]
#[case::one_blocker(&[("B1", "Norwood Riders")], Declared::Accepted)]
#[case::two_blockers(&[("B1", "Norwood Riders"), ("B2", "Norwood Riders")], Declared::TooMany { allowed: 1, got: 2 })]
#[case::three_blockers(&[("B1", "Norwood Riders"), ("B2", "Norwood Riders"), ("B3", "Norwood Riders")], Declared::TooMany { allowed: 1, got: 3 })]
#[case::two_on_the_plain_attacker(&[("B1", "Plain"), ("B2", "Plain"), ("B3", "Norwood Riders")], Declared::Accepted)]
#[case::three_on_the_plain_attacker(&[("B1", "Plain"), ("B2", "Plain"), ("B3", "Plain"), ("B4", "Norwood Riders")], Declared::Accepted)]
fn norwood_riders_takes_one_blocker_at_most(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocks: &[(&str, &str)],
    #[case] expected: Declared,
) -> Result<(), ScriptError> {
    let mut attackers = units(&[("Plain", 2, 2, "")]);
    attackers.push(ready(card("Norwood Riders")?));
    let mut driver = table(seats, attackers, units(FOUR_BLOCKERS))?;
    driver.attack(&[("Norwood Riders", 1), ("Plain", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(limited(&driver), [("Norwood Riders".to_owned(), 1)]);
    assert_eq!(declare(&mut driver, blocks)?, expected);
    Ok(())
}

/// Challenger Troll: each creature its controller controls with power 4 or greater, judged by the layers when the
/// blockers are declared, and no creature of another player.
#[rstest]
#[case::the_troll_alone(&[], 0, &["Challenger Troll"])]
#[case::a_big_creature(&[("Big", 4, 4, "")], 0, &["Big", "Challenger Troll"])]
#[case::a_small_creature(&[("Small", 3, 3, "")], 0, &["Challenger Troll"])]
#[case::a_small_creature_that_grew(&[("Small", 3, 3, "")], 1, &["Challenger Troll", "Small"])]
#[case::a_big_creature_that_shrank(&[("Big", 4, 4, "")], -1, &["Challenger Troll"])]
fn challenger_troll_limits_the_big_creatures_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: &[Unit],
    #[case] change: i32,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut attackers = units(others);
    attackers.push(ready(card("Challenger Troll")?));
    let mut driver = table(seats, attackers, units(&FOUR_BLOCKERS[..2]))?;
    let mut attacks: Vec<(&str, usize)> = others.iter().map(|unit| (unit.0, 1)).collect();
    attacks.push(("Challenger Troll", 1));
    driver.attack(&attacks)?;
    if let Some(unit) = others.first() {
        pump(&mut driver, unit.0, change)?;
    }
    to_blockers(&mut driver, 1)?;
    let names: Vec<String> = limited(&driver).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, expected);
    Ok(())
}

#[rstest]
fn another_players_challenger_troll_limits_none_of_mine(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        units(&[("Big", 5, 5, "")]),
        vec![ready(card("Challenger Troll")?), ready(free_creature("B1", 1, 5))],
    )?;
    driver.attack(&[("Big", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(limited(&driver), []);
    Ok(())
}

/// Familiar Ground limits each creature of its controller, whichever player's it is held by, and none of the others'.
#[rstest]
#[case::mine(0, &["Bear", "Wolf"])]
#[case::the_defenders(1, &[])]
fn familiar_ground_limits_the_creatures_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: usize,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, units(&[("Bear", 2, 2, ""), ("Wolf", 3, 3, "")]))
        .battlefield(1, units(&FOUR_BLOCKERS[..2]));
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, units(&[("Bystander", 1, 1, "")]));
    }
    let ground = ready(card("Familiar Ground")?);
    scenario = if holder == 0 {
        scenario.battlefield(
            0,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Wolf", 3, 3)),
                ground,
            ],
        )
    } else {
        scenario.battlefield(
            1,
            [ready(free_creature("B1", 1, 5)), ready(free_creature("B2", 1, 5)), ground],
        )
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Bear", 1), ("Wolf", 1)])?;
    to_blockers(&mut driver, 1)?;
    let names: Vec<String> = limited(&driver).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, expected);
    Ok(())
}

/// Cloud Sprite can block only creatures with flying.
#[rstest]
#[case::a_ground_attacker(&[("Ground", 2, 2, "")], "Ground", &[])]
#[case::a_flier(&[("Air", 2, 2, "flying")], "Air", &["Cloud Sprite"])]
#[case::a_flier_among_the_ground(&[("Ground", 2, 2, ""), ("Air", 2, 2, "flying")], "Air", &["Cloud Sprite"])]
#[case::the_ground_among_fliers(&[("Ground", 2, 2, ""), ("Air", 2, 2, "flying")], "Ground", &[])]
fn cloud_sprite_blocks_only_fliers(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[Unit],
    #[case] attacker: &str,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = table(seats, units(attackers), vec![ready(card("Cloud Sprite")?)])?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|unit| (unit.0, 1)).collect();
    driver.attack(&attacks)?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, attacker), expected);
    Ok(())
}

/// Dense Canopy, whoever holds it: creatures with flying can block only creatures with flying; the other creatures
/// block as they would.
#[rstest]
#[case::the_attacker_holds_it(0)]
#[case::the_defender_holds_it(1)]
fn dense_canopy_holds_the_fliers_to_the_fliers(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: usize,
) -> Result<(), ScriptError> {
    let canopy = ready(card("Dense Canopy")?);
    let mut attackers = units(&[("Ground", 2, 2, ""), ("Air", 2, 2, "flying")]);
    let mut blockers = units(&[
        ("Flier", 2, 2, "flying"),
        ("Reacher", 2, 2, "reach"),
        ("Walker", 2, 2, ""),
    ]);
    if holder == 0 {
        attackers.push(canopy);
    } else {
        blockers.push(canopy);
    }
    let mut driver = table(seats, attackers, blockers)?;
    driver.attack(&[("Ground", 1), ("Air", 1)])?;
    to_blockers(&mut driver, 1)?;
    // A creature with flying (or reach) can block the flier, and one with flying blocks no one else.
    assert_eq!(offered(&driver, "Air"), ["Flier", "Reacher"]);
    assert_eq!(offered(&driver, "Ground"), ["Reacher", "Walker"]);
    Ok(())
}

#[test]
fn without_dense_canopy_a_flier_blocks_the_ground() -> Result<(), ScriptError> {
    let mut driver = table(
        2,
        units(&[("Ground", 2, 2, "")]),
        units(&[("Flier", 2, 2, "flying")]),
    )?;
    driver.attack(&[("Ground", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, "Ground"), ["Flier"]);
    Ok(())
}

/// Numbers of blockers asked by effects until end of turn, with and without menace.
#[rstest]
#[case::two_with_one_creature(None, Some(2), None, 1, false, None)]
#[case::two_with_two_creatures(None, Some(2), None, 2, true, Some(Declared::Accepted))]
#[case::two_refused_one(None, Some(2), None, 3, true, Some(Declared::TooFew { needed: 2, got: 1 }))]
#[case::one_at_most_refused_two(None, None, Some(1), 3, true, Some(Declared::TooMany { allowed: 1, got: 2 }))]
#[case::one_at_most_takes_one(None, None, Some(1), 3, true, Some(Declared::Accepted))]
#[case::two_at_least_and_one_at_most(None, Some(2), Some(1), 4, false, None)]
#[case::menace_asks_three_of_two(Some("menace"), Some(3), None, 2, false, None)]
#[case::menace_asks_three_of_three(Some("menace"), Some(3), None, 3, true, Some(Declared::TooFew { needed: 3, got: 2 }))]
#[case::menace_asks_more_than_the_effect(Some("menace"), Some(2), None, 2, true, Some(Declared::Accepted))]
fn numbers_of_blockers_from_effects_combine_with_menace(
    #[values(2, 3, 4)] seats: usize,
    #[case] keyword: Option<&'static str>,
    #[case] at_least: Option<u32>,
    #[case] at_most: Option<u32>,
    #[case] creatures: usize,
    #[case] is_offered: bool,
    #[case] declared: Option<Declared>,
) -> Result<(), ScriptError> {
    let attacker: Unit = ("A", 3, 3, keyword.unwrap_or(""));
    let blockers: Vec<Unit> = FOUR_BLOCKERS[..creatures].to_vec();
    let mut driver = table(seats, units(&[attacker]), units(&blockers))?;
    driver.attack(&[("A", 1)])?;
    let id = driver.id_of("A")?;
    for modification in [
        at_least.map(Modification::MinBlockers),
        at_most.map(Modification::MaxBlockers),
    ]
    .into_iter()
    .flatten()
    {
        driver.add_effect(driver.on(id), modification, Duration::UntilEndOfTurn)?;
    }
    to_blockers(&mut driver, 1)?;
    assert_eq!(!offered(&driver, "A").is_empty(), is_offered);
    let Some(declared) = declared else {
        return Ok(());
    };
    let blocks: Vec<(&str, &str)> = match &declared {
        Declared::TooMany { got, .. } | Declared::TooFew { got, .. } => {
            blockers.iter().take(*got).map(|unit| (unit.0, "A")).collect()
        }
        Declared::Accepted => blockers
            .iter()
            .take(at_most.map_or(creatures, |most| most as usize))
            .map(|unit| (unit.0, "A"))
            .collect(),
    };
    assert_eq!(declare(&mut driver, &blocks)?, declared);
    Ok(())
}

/// The size of the creature `name` as the layers make it.
fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Passes priority until the stack is empty, ordering and choosing the first of anything asked.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order)?;
            }
            other => panic!("unexpected question {other:?}"),
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// Titania gives melee to the other creatures of its controller: each grows by the opponents attacked.
#[rstest]
#[case::one_opponent(&[1, 1])]
#[case::two_opponents(&[1, 2])]
fn titania_gives_melee_to_the_other_creatures_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] targets: &[usize],
) -> Result<(), ScriptError> {
    let attackers = vec![
        ready(card("Titania, Proud Pummeler")?),
        ready(free_creature("Cub", 1, 1)),
    ];
    let mut driver = table(seats, attackers, units(&[("Wall", 0, 9, "")]))?;
    let seat = |target: usize| target.min(seats - 1);
    let attacks = [
        ("Titania, Proud Pummeler", seat(targets[0])),
        ("Cub", seat(targets[1])),
    ];
    driver.attack(&attacks)?;
    settle(&mut driver)?;
    let distinct = if seat(targets[0]) == seat(targets[1]) { 1 } else { 2 };
    assert_eq!(size(&driver, "Cub"), (1 + distinct, 1 + distinct));
    let titania = driver.id_of("Titania, Proud Pummeler")?;
    assert_eq!(
        keywords_of(driver.state(), titania)
            .iter()
            .filter(|keyword| ***keyword == Keyword::Melee)
            .count(),
        1,
        "Titania has its own melee and gives none to itself: other creatures only"
    );
    Ok(())
}

/// Skyhunter Strike Force: the other creatures of its controller have melee as long as the controller controls their
/// commander.
#[rstest]
#[case::no_commander_on_the_battlefield(false, false)]
#[case::the_commander_in_play(true, true)]
fn skyhunter_strike_force_gives_melee_while_the_commander_is_in_play(
    #[values(2, 3, 4)] seats: usize,
    #[case] cast: bool,
    #[case] melee: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[mtg_core::color::Color::Red], 2, 2)])
        .battlefield(
            0,
            [
                ready(card("Skyhunter Strike Force")?),
                ready(free_creature("Cub", 1, 1)),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, units(&[("Bystander", 1, 1, "")]));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if cast {
        driver.cast("Cmd")?;
        for _ in 0..seats {
            driver.pass()?;
        }
    }
    let cub = driver.id_of("Cub")?;
    let has = keywords_of(driver.state(), cub).contains(&&Keyword::Melee);
    assert_eq!(has, melee);
    Ok(())
}

/// The commander of another player does not count: the condition names "your commander".
#[rstest]
fn another_players_commander_does_not_satisfy_your_commander(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(1, [legend("Cmd", 1, &[mtg_core::color::Color::Red], 2, 2)])
        .battlefield(
            0,
            [
                ready(card("Skyhunter Strike Force")?),
                ready(free_creature("Cub", 1, 1)),
            ],
        )
        .battlefield(1, [ready(mountain()), ready(mountain())])
        .at(1, 1, Step::Main1);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, units(&[("Bystander", 1, 1, "")]));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    let cub = driver.id_of("Cub")?;
    assert!(!keywords_of(driver.state(), cub).contains(&&Keyword::Melee));
    Ok(())
}
