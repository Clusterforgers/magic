//! Goal: a fight of two targets plays as the rules say at tables of two to four seats: "another target creature" cannot name the creature the first target took (the decision does not offer it and an answer that names it is refused), the counter or the pump that comes first lands before the fight, a target that is illegal when the spell resolves means no fight while the other target still gets what the spell gives it, both illegal means the spell does nothing, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::burn;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

use crate::fight::{Fate, MINE, THEIRS, creature, fate, replays, settle, table};

/// Casts the spell at the two creatures, one specification after the other, and pays.
fn cast_at_both(driver: &mut Driver, spell: &str) -> Result<(), ScriptError> {
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    driver.begin_cast(spell)?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1, Fate::Marked(2), Fate::Dead)]
#[case::three_seats(3, 2, Fate::Marked(2), Fate::Dead)]
#[case::four_seats(4, 3, Fate::Marked(2), Fate::Dead)]
fn a_counter_comes_before_the_fight(
    #[case] seats: usize,
    #[case] foe: usize,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            vec![card("Hunt the Weak").unwrap()],
            creature(MINE, 2, 4, &[]),
            creature(THEIRS, 2, 3, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    cast_at_both(&mut driver, "Hunt the Weak")?;
    settle(&mut driver)?;
    // 3 power with the counter kills the 3 toughness; the 2 power of the other marks 2 damage.
    assert_eq!(fate(&driver, mine), mine_fate);
    assert_eq!(fate(&driver, theirs), theirs_fate);
    let counters = driver
        .state()
        .object(mine)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, Some(1));
    replays(&driver, &setup);
    Ok(())
}

/// "Target creature fights another target creature": the second decision does not offer the first creature, the cast
/// cannot be finished with it and the fight still works for two other creatures.
#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn another_target_is_not_the_first_target(
    #[case] seats: usize,
    #[case] foe: usize,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            vec![card("Clash of Titans").unwrap()],
            creature(MINE, 3, 3, &[]),
            creature(THEIRS, 1, 1, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let first = driver.target_named(MINE)?;
    driver.begin_cast("Clash of Titans")?;
    driver.choose_targets(&[first])?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the second target is asked");
    };
    assert!(
        !options.legal.contains(&first),
        "the first creature is not offered again"
    );
    assert!(options.legal.contains(&driver.target_named(THEIRS)?));
    // Naming the first creature again is refused.
    assert!(
        driver.choose_targets(&[first]).is_err(),
        "the same creature twice"
    );
    driver.choose_targets(&[driver.target_named(THEIRS)?])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine), Fate::Marked(1));
    assert_eq!(fate(&driver, theirs), Fate::Dead);
    replays(&driver, &setup);
    Ok(())
}

/// A second spell answers the fight on the stack: the creature it kills is the one that is illegal when the fight resolves.
#[rstest]
#[case::their_creature_is_gone(THEIRS, Fate::Marked(0), Fate::Dead, 1)]
#[case::our_creature_is_gone(MINE, Fate::Dead, Fate::Marked(0), 0)]
fn an_illegal_target_means_no_fight_and_the_counter_still_lands(
    #[case] at: &str,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            3,
            1,
            vec![card("Hunt the Weak").unwrap(), burn("Bolt", 9)],
            creature(MINE, 4, 4, &[Keyword::Vigilance]),
            creature(THEIRS, 4, 4, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    cast_at_both(&mut driver, "Hunt the Weak")?;
    let victim = driver.target_named(at)?;
    driver.cast_at("Bolt", &[victim])?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs), theirs_fate, "theirs");
    let held = driver
        .state()
        .object(mine)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(held, counters);
    replays(&driver, &setup);
    Ok(())
}

/// Both creatures are gone when the spell resolves: it does nothing and nothing is left to fight.
#[test]
fn both_targets_illegal_means_the_spell_does_nothing() -> Result<(), ScriptError> {
    let game = || {
        table(
            2,
            1,
            vec![
                card("Prey Upon").unwrap(),
                burn("Bolt", 9),
                burn("Shock", 9),
            ],
            creature(MINE, 2, 2, &[]),
            creature(THEIRS, 2, 2, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    cast_at_both(&mut driver, "Prey Upon")?;
    let (first, second) = (driver.target_named(MINE)?, driver.target_named(THEIRS)?);
    driver.cast_at("Bolt", &[first])?;
    driver.cast_at("Shock", &[second])?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine), Fate::Dead);
    assert_eq!(fate(&driver, theirs), Fate::Dead);
    replays(&driver, &setup);
    Ok(())
}
