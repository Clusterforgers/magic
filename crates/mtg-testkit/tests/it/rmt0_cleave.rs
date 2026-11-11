//! Goal: cleave plays whole at two to four seats: the cleave cost is offered beside the mana cost, a spell cast for it
//! announces and resolves as its text without the bracketed words (targets and effects), and one cast for its mana cost
//! keeps them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{names, pid};
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `spell` over three Plains and three Swamps; seat 1 controls a 2/2 of mana value 0 and a 4/4 of mana value 4.
fn table(seats: usize, spell: &str) -> Scenario {
    let mut mine = lands(3, plains);
    mine.extend(lands(3, swamp));
    Scenario::new(seats)
        .hand_named(0, [spell])
        .battlefield(0, mine)
        .battlefield(
            1,
            vec![
                ready(free_creature("Bear", 2, 2)),
                ready(creature("Brute", 4, &[], 4, 4)),
            ],
        )
}

/// The life of seat 0.
fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Passes priority until the stack is empty.
fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().zone(Zone::Stack).is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_cleaved_spell_drops_the_bracketed_words_and_one_cast_for_its_mana_cost_keeps_them(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    // Path of Peril: "Destroy all creatures [with mana value 2 or less]."
    for (way, left) in [("Cast for", vec!["Brute"]), ("Cleave", vec![])] {
        let mut driver = Driver::from_scenario(table(seats, "Path of Peril"))?;
        driver.begin_cast("Path of Peril")?;
        assert_eq!(ways(&driver)?, ["Cast for {1}{W}{B}", "Cleave {4}{W}{B}"]);
        choose_way(&mut driver, way)?;
        pay(&mut driver)?;
        resolve_all(&mut driver)?;
        let mut theirs = names(driver.state(), Zone::Battlefield);
        theirs.retain(|name| name == "Bear" || name == "Brute");
        assert_eq!(theirs, left, "{way}");
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn a_cleaved_spell_targets_what_its_bracketed_words_would_leave_out(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    // Fierce Retribution: "Destroy target [attacking] creature." Nothing attacks, so only the cleave cost has a target.
    let mut driver = Driver::from_scenario(table(seats, "Fierce Retribution"))?;
    driver.begin_cast("Fierce Retribution")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("cast for its cleave cost alone, the spell asks its target");
    };
    assert_eq!(options.legal.len(), 2);
    let brute = driver.target_named("Brute")?;
    driver.choose_targets(&[brute])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{5}{W}");
    pay(&mut driver)?;
    resolve_all(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Brute"]);
    Ok(())
}

#[test]
fn a_spell_cast_for_its_mana_cost_keeps_its_target_restriction() -> Result<(), ScriptError> {
    // Parasitic Grasp: "~ deals 3 damage to target [Human] creature. You gain 3 life." No Human: only the cleave cost.
    let mut driver = Driver::from_scenario(table(2, "Parasitic Grasp"))?;
    let before = life(&driver);
    driver.begin_cast("Parasitic Grasp")?;
    let DecisionKind::ChooseTargets(_) = &driver.decision()?.kind else {
        panic!("no way is asked: the mana cost has no legal target");
    };
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{1}{B}{B}");
    pay(&mut driver)?;
    resolve_all(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Bear"]);
    assert_eq!(life(&driver), before + 3);
    Ok(())
}
