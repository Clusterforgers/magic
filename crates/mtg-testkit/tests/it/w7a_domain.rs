//! Goal: domain, the number of basic land types among the lands a player controls, plays as printed at tables of two,
//! three and four seats: each type counts once however many lands have it, a land of every basic land type counts all
//! five, a type an effect gives a land counts, the opponents' lands never count, Tribal Flames deals that much damage
//! and Territorial Maro is twice that big, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

/// A land that is every basic land type and has no ability.
fn w7a_every_type() -> Arc<CardDefinition> {
    let land = CardDefinition::new("Every Type").with_type(CardType::Land);
    Arc::new(
        ["Plains", "Island", "Swamp", "Mountain", "Forest"]
            .into_iter()
            .fold(land, CardDefinition::with_subtype),
    )
}

/// Lands by name; any other name is the land of every basic land type.
fn w7a_lands(names: &[&str]) -> Vec<Arc<CardDefinition>> {
    names
        .iter()
        .map(|name| match *name {
            "Forest" => forest(),
            "Island" => island(),
            "Swamp" => swamp(),
            "Plains" => plains(),
            "Mountain" => mountain(),
            _ => w7a_every_type(),
        })
        .collect()
}

/// A table where seat 0 holds Tribal Flames and controls two Mountains and `lands`, every opponent controls a Plains and a
/// Swamp, and seat 1 controls `theirs` too.
fn w7a_table(seats: usize, lands: &[&str], theirs: &[&str]) -> Scenario {
    let mut mine: Vec<_> = w7a_lands(lands).into_iter().map(ready).collect();
    mine.extend([ready(mountain()), ready(mountain())]);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Tribal Flames").unwrap()])
        .battlefield(0, mine);
    for seat in 1..seats {
        let mut lands = vec![plains(), swamp()];
        if seat == 1 {
            lands.extend(theirs.iter().map(|name| card(name).unwrap()));
        }
        scenario = scenario.battlefield(seat, lands.into_iter().map(ready));
    }
    scenario
}

#[rstest]
#[case::one_type(&["Mountain"], &[], 1)]
#[case::three_types(&["Forest", "Island", "Island"], &[], 3)]
#[case::every_type_land(&["Every Type"], &[], 5)]
#[case::every_type_and_basics(&["Every Type", "Forest", "Swamp"], &[], 5)]
#[case::a_type_an_effect_gives(&["Island"], &["Yavimaya, Cradle of Growth"], 3)]
fn w7a_tribal_flames_deals_the_domain_count(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] theirs: &[&str],
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = w7a_table(seats, lands, theirs);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let life = |driver: &Driver| driver.state().player(PlayerId(1)).unwrap().life();
    let before = life(&driver);
    let at = driver.target_player(1);
    driver.cast_at("Tribal Flames", &[at])?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(before - life(&driver), damage);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::one_type(&["Island"], 1)]
#[case::two_types(&["Forest", "Forest", "Island"], 2)]
#[case::every_type_land(&["Every Type", "Island"], 5)]
fn w7a_territorial_maro_is_twice_the_domain_count(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] domain: i32,
) -> Result<(), ScriptError> {
    let mut mine = w7a_lands(lands);
    mine.push(card("Territorial Maro").unwrap());
    let mut scenario = Scenario::new(seats).battlefield(0, mine.into_iter().map(ready));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(plains()), ready(mountain())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let chars = characteristics(driver.state(), driver.id_of("Territorial Maro").unwrap()).unwrap();
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(2 * domain), Some(2 * domain))
    );
    Ok(())
}
