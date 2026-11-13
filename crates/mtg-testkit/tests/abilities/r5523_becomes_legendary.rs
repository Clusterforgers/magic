//! Goal: "becomes a legendary N/N <types> creature ... until end of turn" makes the permanent legendary as well as a
//! creature of that size and those creature types for the turn, keeping its other types: Genju of the Realm's land
//! stays a land, Cacophony Unleashed stays an enchantment and animates whenever it or another enchantment its
//! controller controls enters (an opponent's enchantment is a near miss); at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{CardType, Supertype};
use mtg_engine::characteristics;
use mtg_engine::characteristics::has_supertype;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// Answers passively until `done` holds, while the first main phase lasts.
fn r5523_until(driver: &mut Driver, done: impl Fn(&Driver) -> bool) -> Result<bool, ScriptError> {
    for _ in 0..60 {
        if done(driver) {
            return Ok(true);
        }
        if driver.state().turn().step() != Step::Main1 {
            return Ok(false);
        }
        driver.answer_passively()?;
    }
    Ok(done(driver))
}

fn r5523_power(driver: &Driver, name: &str) -> Option<i32> {
    let id = driver.id_of(name).ok()?;
    characteristics(driver.state(), id)?.power
}

#[rstest]
fn r5523_genju_land_becomes_a_legendary_spirit(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(forest()),
            attached_to(ready(card("Genju of the Realm").unwrap()), 0, 0),
            ready(plains()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let land = driver.id_of("Forest")?;
    assert!(!has_supertype(driver.state(), land, Supertype::Legendary));
    driver.begin_activation("Genju of the Realm")?;
    assert!(
        r5523_until(&mut driver, |d| r5523_power(d, "Forest") == Some(8))?,
        "the land animates"
    );
    let found = characteristics(driver.state(), land).unwrap();
    assert_eq!((found.power, found.toughness), (Some(8), Some(12)));
    assert!(found.types.contains(CardType::Land) && found.types.contains(CardType::Creature));
    assert!(has_supertype(driver.state(), land, Supertype::Legendary));
    assert!(found.subtypes.iter().any(|subtype| subtype.0 == "Spirit"));
    Ok(())
}

#[rstest]
#[case::its_own_controller(0, true)]
#[case::near_miss_an_opponent(1, false)]
fn r5523_cacophony_animates_on_an_enchantment(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] animates: bool,
) -> Result<(), ScriptError> {
    let charm = card("Glorious Anthem").unwrap();
    let lands = || [ready(plains()), ready(plains()), ready(plains())];
    let mut own = vec![ready(card("Cacophony Unleashed").unwrap())];
    let mut scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .hand(caster, [charm]);
    if caster == 0 {
        own.extend(lands());
    } else {
        scenario = scenario.battlefield(caster, lands());
    }
    let scenario = scenario.battlefield(0, own);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        r5523_power(&driver, "Cacophony Unleashed"),
        None,
        "an enchantment only"
    );
    driver.begin_cast("Glorious Anthem")?;
    let found = r5523_until(&mut driver, |d| {
        r5523_power(d, "Cacophony Unleashed") == Some(6)
    })?;
    assert_eq!(found, animates);
    if animates {
        let id = driver.id_of("Cacophony Unleashed")?;
        let traits = characteristics(driver.state(), id).unwrap();
        assert!(
            traits.types.contains(CardType::Enchantment)
                && traits.types.contains(CardType::Creature)
        );
        assert!(has_supertype(driver.state(), id, Supertype::Legendary));
    }
    Ok(())
}
