//! Goal: a fight plays as CR 701.14 says at tables of two, three and four seats: both creatures deal damage equal to their power to each other once and not as combat damage, deathtouch and lifelink count, first strike changes nothing, a creature of power 0 deals none, a target that is gone or no longer a creature when the fight resolves means no damage at all, damage prevention applies, the Aura Warbriar Blessing makes the creature it enchants fight up to one creature (none is chosen: no fight), and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{burn, forest, keyworded, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::fight_cards::brawl;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// What became of a creature: still on the battlefield with this much damage, or gone from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fate {
    Marked(u32),
    Dead,
}

pub(crate) const MINE: &str = "Mine";
pub(crate) const THEIRS: &str = "Theirs";

/// Answers passively until the stack is empty and nothing resolves.
pub(crate) fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

pub(crate) fn fate(driver: &Driver, id: ObjectId) -> Fate {
    let object = driver.state().object(id).unwrap();
    match object.permanent() {
        Some(permanent) if object.zone() == Zone::Battlefield => Fate::Marked(permanent.damage()),
        _ => Fate::Dead,
    }
}

pub(crate) fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

pub(crate) fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Every seat gets a library; seat 0 holds `hand`, has `mine` and four Mountains and the Forests and Plains it needs;
/// the seat `foe` has `theirs`; the other seats have a Bear each.
pub(crate) fn table(
    seats: usize,
    foe: usize,
    hand: Vec<Arc<CardDefinition>>,
    mine: Arc<CardDefinition>,
    theirs: Arc<CardDefinition>,
) -> Scenario {
    let mut lots = lands(4, mountain);
    lots.extend(lands(2, forest));
    lots.extend(lands(1, plains));
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| mtg_testkit::cards::bear()));
    }
    lots.push(ready(mine));
    scenario = scenario.battlefield(0, lots);
    scenario = scenario.battlefield(foe, [ready(theirs)]);
    for seat in (1..seats).filter(|seat| *seat != foe) {
        scenario = scenario.battlefield(seat, [ready(mtg_testkit::cards::bear())]);
    }
    scenario
}

pub(crate) fn creature(
    name: &str,
    power: i32,
    toughness: i32,
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    keyworded(name, power, toughness, keywords)
}

/// Casts Brawl at the two creatures and lets the table settle: how each creature fared, and the life of seat 0.
#[rstest]
#[case::trade(2, 1, creature(MINE, 2, 2, &[]), creature(THEIRS, 2, 2, &[]), Fate::Dead, Fate::Dead, 0)]
#[case::the_bigger_one_wins(2, 1, creature(MINE, 3, 3, &[]), creature(THEIRS, 1, 1, &[]), Fate::Marked(1), Fate::Dead, 0)]
#[case::three_seats(3, 2, creature(MINE, 3, 3, &[]), creature(THEIRS, 2, 4, &[]), Fate::Marked(2), Fate::Marked(3), 0)]
#[case::four_seats(4, 3, creature(MINE, 5, 5, &[]), creature(THEIRS, 4, 4, &[]), Fate::Marked(4), Fate::Dead, 0)]
#[case::deathtouch_kills_the_big_one(2, 1, creature(MINE, 1, 1, &[Keyword::Deathtouch]), creature(THEIRS, 5, 5, &[]), Fate::Dead, Fate::Dead, 0)]
#[case::deathtouch_on_the_other_side(3, 1, creature(MINE, 6, 6, &[]), creature(THEIRS, 1, 1, &[Keyword::Deathtouch]), Fate::Dead, Fate::Dead, 0)]
#[case::lifelink_gains_the_damage(2, 1, creature(MINE, 3, 4, &[Keyword::Lifelink]), creature(THEIRS, 2, 2, &[]), Fate::Marked(2), Fate::Dead, 3)]
#[case::lifelink_of_the_fought_one_is_not_ours(3, 1, creature(MINE, 2, 5, &[]), creature(THEIRS, 3, 3, &[Keyword::Lifelink]), Fate::Marked(3), Fate::Marked(2), 0)]
#[case::first_strike_changes_nothing(2, 1, creature(MINE, 2, 2, &[Keyword::FirstStrike]), creature(THEIRS, 2, 2, &[]), Fate::Dead, Fate::Dead, 0)]
#[case::double_strike_deals_it_once(2, 1, creature(MINE, 2, 4, &[Keyword::DoubleStrike]), creature(THEIRS, 3, 3, &[]), Fate::Marked(3), Fate::Marked(2), 0)]
#[case::power_zero_deals_none(2, 1, creature(MINE, 0, 4, &[]), creature(THEIRS, 2, 2, &[]), Fate::Marked(2), Fate::Marked(0), 0)]
#[case::both_power_zero(2, 1, creature(MINE, 0, 1, &[]), creature(THEIRS, 0, 1, &[]), Fate::Marked(0), Fate::Marked(0), 0)]
fn both_creatures_deal_damage_equal_to_their_power_to_each_other_once(
    #[case] seats: usize,
    #[case] foe: usize,
    #[case] mine: Arc<CardDefinition>,
    #[case] theirs: Arc<CardDefinition>,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
    #[case] gain: i32,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            vec![brawl("Brawl").unwrap()],
            mine.clone(),
            theirs.clone(),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine_id, theirs_id) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    let before = life(&driver, 0);
    driver.begin_cast("Brawl")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine_id), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs_id), theirs_fate, "theirs");
    assert_eq!(life(&driver, 0) - before, gain);
    replays(&driver, &setup);
    Ok(())
}

/// The damage events of the log and whether any was combat damage (a fight is not).
#[test]
fn a_fight_is_damage_outside_combat_dealt_by_each_creature_to_the_other() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(table(
        2,
        1,
        vec![brawl("Brawl").unwrap()],
        creature(MINE, 3, 3, &[]),
        creature(THEIRS, 2, 5, &[]),
    ))?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    driver.begin_cast("Brawl")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    settle(&mut driver)?;
    let mut dealt: Vec<(ObjectId, ObjectId, u32)> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt {
                source,
                target: mtg_engine::combat::DamageTarget::Object(to),
                amount,
                ..
            } => Some((*source, *to, *amount)),
            _ => None,
        })
        .collect();
    dealt.sort();
    let mut expected = vec![(mine, theirs, 3), (theirs, mine, 2)];
    expected.sort();
    assert_eq!(dealt, expected);
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::AttackersDeclared { .. }))
    );
    Ok(())
}

/// A second spell answers the fight on the stack: `answer` is cast at `at` (a name) before the fight resolves.
#[rstest]
#[case::their_creature_dies_first(THEIRS, Fate::Marked(0), Fate::Dead)]
#[case::our_creature_dies_first(MINE, Fate::Dead, Fate::Marked(0))]
fn a_creature_that_left_before_the_fight_resolves_means_no_damage_at_all(
    #[case] at: &str,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            3,
            1,
            vec![brawl("Brawl").unwrap(), burn("Bolt", 5)],
            creature(MINE, 4, 4, &[]),
            creature(THEIRS, 4, 4, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine_id, theirs_id) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    driver.begin_cast("Brawl")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    let victim: Target = driver.target_named(at)?;
    driver.cast_at("Bolt", &[victim])?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine_id), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs_id), theirs_fate, "theirs");
    replays(&driver, &setup);
    Ok(())
}

/// Warbriar Blessing: the creature it enchants fights up to one creature its controller does not control.
#[rstest]
#[case::the_fought_creature_dies(2, 1, (3, 2, 2), true, Fate::Marked(2), Fate::Dead)]
#[case::the_enchanted_creature_survives_by_its_toughness(3, 2, (2, 4, 5), true, Fate::Marked(4), Fate::Marked(2))]
#[case::no_creature_chosen_is_no_fight(2, 1, (3, 3, 3), false, Fate::Marked(0), Fate::Marked(0))]
#[case::four_seats(4, 3, (5, 2, 4), true, Fate::Marked(2), Fate::Dead)]
fn warbriar_blessing_makes_the_enchanted_creature_fight_when_it_enters(
    #[case] seats: usize,
    #[case] foe: usize,
    #[case] sizes: (i32, i32, i32),
    #[case] chosen: bool,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
) -> Result<(), ScriptError> {
    // Mine is a creature of this power and a toughness of 3 (5 with the Aura); theirs has this power and toughness.
    let (power, their_power, their_toughness) = sizes;
    let game = || {
        table(
            seats,
            foe,
            vec![card("Warbriar Blessing").unwrap()],
            creature(MINE, power, 3, &[]),
            creature(THEIRS, their_power, their_toughness, &[]),
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine_id, theirs_id) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let mine = driver.target_named(MINE)?;
    let theirs = driver.target_named(THEIRS)?;
    driver.begin_cast("Warbriar Blessing")?;
    driver.choose_targets(&[mine])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    // The Aura resolves; its trigger asks for the creature to fight.
    for _ in 0..12 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    if chosen {
        driver.choose_targets(&[theirs])?;
    } else {
        driver.choose_targets(&[])?;
    }
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine_id), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs_id), theirs_fate, "theirs");
    replays(&driver, &setup);
    Ok(())
}
