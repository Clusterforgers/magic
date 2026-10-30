//! Goal: the attach-and-move cards of the owner's deck play as printed at tables of two, three and four seats: Fungal Fortitude returns the creature it enchants tapped when it dies, Unfinished Business returns a creature and attaches the Auras and Equipment it returns, and Danitha puts an Aura or Equipment onto the battlefield attached to herself.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::attach_cards::{heavy_plate, iron_blade, mighty_aura, rune_aura};
use mtg_testkit::cards::{burn, free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield, pid};

/// Passes priority until the stack is empty and nothing is resolving.
fn dk1b_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the battlefield permanent with this name is tapped.
fn dk1b_tapped(driver: &Driver, name: &str) -> bool {
    let id = on_battlefield(driver.state(), name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

/// The host of the permanent with this name, by name.
fn dk1b_host_of(driver: &Driver, name: &str) -> Option<String> {
    let id = on_battlefield(driver.state(), name)?;
    let host: ObjectId = driver.state().host_of(id)?;
    Some(driver.state().object(host)?.card().name.clone())
}

/// The card of this name in the graveyard of `seat`, as a target.
fn dk1b_grave(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap();
    Target::Object(state.object(id).unwrap().reference())
}

/// A table where seat 0 holds Unfinished Business with five Plains, and a graveyard of the Fallen creature, the
/// Mighty Aura, the Iron Blade and the Rune Aura (which enchants lands only).
fn dk1b_business(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Unfinished Business").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(plains())))
        .graveyard(
            0,
            [
                free_creature("Fallen", 2, 2),
                mighty_aura().unwrap(),
                iron_blade().unwrap(),
                rune_aura().unwrap(),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.graveyard(seat, [heavy_plate().unwrap()]);
    }
    scenario
}

#[rstest]
#[case::both_fit(&["Mighty Aura", "Iron Blade"], &["Mighty Aura", "Iron Blade"], &[])]
#[case::an_aura_that_cannot_enchant_stays(&["Rune Aura", "Iron Blade"], &["Iron Blade"], &["Rune Aura"])]
#[case::only_an_equipment(&["Iron Blade"], &["Iron Blade"], &[])]
#[case::no_attachments(&[], &[], &[])]
fn dk1b_unfinished_business_returns_the_creature_and_attaches_what_fits(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &[&str],
    #[case] attached: &[&str],
    #[case] stays: &[&str],
) -> Result<(), ScriptError> {
    let setup = dk1b_business(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(dk1b_business(seats))?;
    let fallen = dk1b_grave(&driver, 0, "Fallen");
    let attachments: Vec<Target> = chosen
        .iter()
        .map(|name| dk1b_grave(&driver, 0, name))
        .collect();
    driver.begin_cast("Unfinished Business")?;
    driver.choose_targets(&[fallen])?;
    driver.choose_targets(&attachments)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    dk1b_settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Fallen").is_some());
    for name in attached {
        assert_eq!(
            dk1b_host_of(&driver, name).as_deref(),
            Some("Fallen"),
            "{name}"
        );
    }
    for name in stays {
        assert!(on_battlefield(driver.state(), name).is_none(), "{name}");
        assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&(*name).to_owned()));
    }
    // What was not chosen stays in the graveyard, and the other seats' graveyards are untouched.
    for name in ["Mighty Aura", "Iron Blade"] {
        if !chosen.contains(&name) {
            assert!(on_battlefield(driver.state(), name).is_none(), "{name}");
        }
    }
    for seat in 1..seats {
        assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(seat))),
            vec!["Heavy Plate".to_owned()]
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn dk1b_fungal_fortitude_returns_the_enchanted_creature_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
) -> Result<(), ScriptError> {
    // The enchanted creature belongs to `owner`; the Aura's controller is seat 0 either way.
    let mut worn = vec![
        ready(free_creature("Mine", 2, 2)),
        attached_to(ready(card("Fungal Fortitude").unwrap()), owner, 0),
    ];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)]);
    if owner == 0 {
        worn.push(ready(mountain()));
    } else {
        scenario = scenario.battlefield(0, [ready(mountain())]);
    }
    let scenario = scenario.battlefield(owner, worn);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Mine")?;
    driver.cast_at("Bolt", &[target])?;
    dk1b_settle(&mut driver)?;
    assert!(dk1b_tapped(&driver, "Mine"));
    assert!(on_battlefield(driver.state(), "Fungal Fortitude").is_none());
    assert!(
        names(driver.state(), Zone::Graveyard(pid(owner))).contains(&"Fungal Fortitude".to_owned())
    );
    replays(&driver, &setup);
    Ok(())
}
