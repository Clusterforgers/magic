//! Goal: the Auras that remake their creature (a Treefolk with a base size and no abilities, an Elk, an Insect that keeps indestructible), the bonuses counted by a set of kinds or by the opponents, the loss of a keyword beside a bonus and a protection from a creature type reach exactly the objects their words name at two to four seats, end when the source leaves, and replay to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{GameSetup, characteristics, replay};
use mtg_testkit::cards::{bear, burn, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SPARK: &str = "Spark";

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// The names of the keywords the permanent has.
fn keyword_names(driver: &Driver, name: &str) -> Vec<&'static str> {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .map(|held| held.name())
        .collect()
}

/// A creature of one subtype with nothing else.
fn kind(name: &str, subtype: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(power, toughness),
    )
}

/// Seat 0 casts a burn spell of `damage` at the start of its first main phase; each seat has a library to draw from.
fn table(seats: usize, lists: Vec<Vec<PresetPermanent>>, damage: i32) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for (seat, mut list) in lists.into_iter().enumerate() {
        if seat == 0 {
            list.push(ready(mountain()));
        }
        scenario = scenario
            .battlefield(seat, list)
            .library(seat, vec![bear(), bear(), bear(), bear()]);
    }
    scenario
        .hand(0, vec![burn(SPARK, damage)])
        .at(1, 0, Step::Main1)
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 burns the named permanent with the spell in its hand.
fn burn_it(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let at = driver.target_named(name)?;
    driver.cast_at(SPARK, &[at])?;
    settle(driver)
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The other seats each control one creature of a subtype of their own.
fn foes(seats: usize, subtype: &str) -> Vec<Vec<PresetPermanent>> {
    (1..seats)
        .map(|seat| vec![ready(kind(&format!("Foe {subtype} {seat}"), subtype, 1, 1))])
        .collect()
}

#[rstest]
#[case::treefolk("Lignify", (0, 4), &[])]
#[case::elk("Kenrith's Transformation", (3, 3), &[])]
#[case::insect("Darksteel Mutation", (0, 1), &["Indestructible"])]
fn an_aura_remakes_the_creature_it_enchants_and_no_other(
    #[case] aura: &str,
    #[case] size: (i32, i32),
    #[case] keywords: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lists = vec![vec![
        ready(card("Serra Angel").unwrap()),
        attached_to(ready(card(aura).unwrap()), 0, 0),
        ready(bear()),
    ]];
    lists.extend(foes(seats, "Elf"));
    let game = || table(seats, lists.clone(), 1);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    // Serra Angel flies and has vigilance; the Aura takes both and sets the size.
    assert_eq!(stats(&driver, "Serra Angel"), size);
    assert_eq!(keyword_names(&driver, "Serra Angel"), keywords);
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    for seat in 1..seats {
        assert_eq!(stats(&driver, &format!("Foe Elf {seat}")), (1, 1));
    }
    // The Aura ends with the permanent it enchants: burning the host leaves the rest as it was.
    burn_it(&mut driver, "Foe Elf 1")?;
    assert_eq!(stats(&driver, "Serra Angel"), size);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_bonus_counted_by_opponents_and_by_two_kinds_follows_the_table(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lists = vec![vec![
        ready(bear()),
        attached_to(ready(card("Blazing Sunsteel").unwrap()), 0, 0),
        ready(card("Serra Angel").unwrap()),
        attached_to(ready(card("All That Glitters").unwrap()), 0, 2),
        attached_to(ready(card("Colossus Hammer").unwrap()), 0, 2),
    ]];
    lists.extend(foes(seats, "Elf"));
    let game = || table(seats, lists.clone(), 1);
    let setup = game().setup().unwrap();
    let driver = Driver::from_scenario(game())?;
    let opponents = i32::try_from(seats - 1).unwrap();
    // +1/+0 for each opponent: one for each seat but its own.
    assert_eq!(stats(&driver, "Grizzly Bears"), (2 + opponents, 2));
    // The Angel counts the artifacts and enchantments its controller has: Sunsteel, Glitters and Hammer.
    assert_eq!(stats(&driver, "Serra Angel"), (4 + 3 + 10, 4 + 3 + 10));
    assert!(!keyword_names(&driver, "Serra Angel").contains(&"Flying"));
    assert!(keyword_names(&driver, "Serra Angel").contains(&"Vigilance"));
    // The bonus is the controller's: a foe's creature gets none of it.
    for seat in 1..seats {
        assert_eq!(stats(&driver, &format!("Foe Elf {seat}")), (1, 1));
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn protection_from_a_creature_type_belongs_to_the_cats_of_its_controller_until_the_lynx_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lists = vec![vec![
        ready(card("Hungry Lynx").unwrap()),
        ready(kind("Tabby", "Cat", 1, 1)),
        ready(bear()),
    ]];
    lists.extend(foes(seats, "Cat"));
    let game = || table(seats, lists.clone(), 3);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    assert!(keyword_names(&driver, "Tabby").contains(&"Protection"));
    assert!(keyword_names(&driver, "Hungry Lynx").contains(&"Protection"));
    assert!(!keyword_names(&driver, "Grizzly Bears").contains(&"Protection"));
    for seat in 1..seats {
        let foe = format!("Foe Cat {seat}");
        assert!(
            !keyword_names(&driver, &foe).contains(&"Protection"),
            "{foe}"
        );
    }
    burn_it(&mut driver, "Hungry Lynx")?;
    assert!(!keyword_names(&driver, "Tabby").contains(&"Protection"));
    replays(&driver, &setup);
    Ok(())
}
