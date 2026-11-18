//! Goal: the passives and Enrages of the two-colour bosses of generation 5 do what their words say in real games:
//! Hollowmere's dead return as Spirits, Korrath's creatures grow when hurt, Sahrra's pride grows when a Cat falls,
//! Valdric's Vampires feed on the heroes' dead, and each Enrage waits for the heroes' seventh turn and then grows.
use std::sync::Arc;

use mtg_campaign::content;
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The hero (a Bear, two Mountains, three Bears in the graveyard) against the boss (a Brute, a Mountain and the corpus
/// cards `own`) with the content cards `ids` in its command zone; both draw from libraries of Mountains.
fn table(ids: &[&str], own: &[&str]) -> Scenario {
    let mut boss = vec![ready(free_creature("Brute", 3, 3)), ready(mountain())];
    boss.extend(own.iter().map(|name| ready(corpus::card(name).unwrap())));
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(
            HERO,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .graveyard(HERO, (0..3).map(|_| free_creature("Fallen", 2, 2)))
        .battlefield(BOSS, boss)
        .library(HERO, (0..40).map(|_| mountain()))
        .library(BOSS, (0..40).map(|_| mountain()))
        .emblems(BOSS, ids.iter().map(|id| custom(id)))
}

/// The creatures named `name` that `seat` controls.
fn count(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == PlayerId::from_index(seat) && o.card().name == name)
        .count()
}

fn power(driver: &Driver, name: &str) -> Option<i32> {
    let id = driver.id_of(name).ok()?;
    characteristics(driver.state(), id).and_then(|c| c.power)
}

/// The boss casts a burn spell of `amount` at `victim` in its main phase and the stack resolves.
fn burn_at(ids: &[&str], own: &[&str], victim: &str, amount: i32) -> Result<Driver, ScriptError> {
    let scenario = table(ids, own)
        .hand(BOSS, [burn("Bolt", amount)])
        .at(2, BOSS, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Bolt", &[target])?;
    for _ in 0..6 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
#[case::its_own_creature_returns_as_a_spirit("Brute", 1)]
#[case::a_hero_creature_does_not("Bear", 0)]
fn hollowmeres_dead_return_as_spirits(
    #[case] victim: &str,
    #[case] spirits: usize,
) -> Result<(), ScriptError> {
    let driver = burn_at(&["hollowmere"], &[], victim, 3)?;
    assert_eq!(count(&driver, BOSS, "Spirit"), spirits);
    Ok(())
}

#[rstest]
#[case::hurt_and_alive_it_grows("Brute", 1, Some(4))]
#[case::a_hero_creature_does_not("Bear", 1, Some(3))]
fn korraths_creatures_grow_when_hurt(
    #[case] victim: &str,
    #[case] amount: i32,
    #[case] brute: Option<i32>,
) -> Result<(), ScriptError> {
    let driver = burn_at(&["korrath"], &[], victim, amount)?;
    assert_eq!(power(&driver, "Brute"), brute);
    Ok(())
}

#[test]
fn sahrras_pride_grows_when_a_cat_falls() -> Result<(), ScriptError> {
    let driver = burn_at(&["sahrra"], &["Mesa Lynx", "Jungle Lion"], "Jungle Lion", 3)?;
    assert_eq!(power(&driver, "Mesa Lynx"), Some(3));
    assert_eq!(power(&driver, "Brute"), Some(3), "only Cats");
    Ok(())
}

#[rstest]
#[case::a_hero_creature_feeds_them("Bear", Some(2))]
#[case::its_own_creature_does_not("Brute", Some(1))]
fn valdrics_vampires_feed_on_the_heroes_dead(
    #[case] victim: &str,
    #[case] vampire: Option<i32>,
) -> Result<(), ScriptError> {
    let driver = burn_at(&["valdric"], &["Vampire Cutthroat"], victim, 3)?;
    assert_eq!(power(&driver, "Vampire Cutthroat"), vampire);
    Ok(())
}

// Game turn 2n is the boss's nth; the hero has had n turns by then.
#[rstest]
#[case::the_drowned_wait("tolling_of_the_drowned", "Spirit", 12, 0)]
#[case::the_drowned_toll_once("tolling_of_the_drowned", "Spirit", 14, 1)]
#[case::the_drowned_grow("tolling_of_the_drowned", "Spirit", 16, 3)]
#[case::the_stampede_waits("the_great_stampede", "Dinosaur", 12, 0)]
#[case::the_stampede_grows("the_great_stampede", "Dinosaur", 16, 3)]
#[case::the_pride_waits("the_pride_hunts", "Cat", 12, 0)]
#[case::the_pride_grows("the_pride_hunts", "Cat", 16, 3)]
#[case::the_feast_waits("the_crimson_feast", "Vampire", 12, 0)]
#[case::the_feast_grows("the_crimson_feast", "Vampire", 16, 3)]
#[case::no_dream_is_stolen_yet("stolen_dreams", "Fallen", 12, 0)]
#[case::one_dream_is_stolen("stolen_dreams", "Fallen", 14, 1)]
#[case::the_graveyard_runs_dry("stolen_dreams", "Fallen", 16, 3)]
fn a_new_enrage_waits_for_the_heroes_seventh_turn_then_grows(
    #[case] id: &str,
    #[case] name: &str,
    #[case] turn: u32,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(&[id], &[]))?;
    driver.idle_until(turn, Step::Main1)?;
    assert_eq!(count(&driver, BOSS, name), expected);
    Ok(())
}
