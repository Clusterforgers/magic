//! Goal: an Aura that triggers "when enchanted creature dies" finds the creature and itself in the graveyard after a destroy spell, combat damage or a sacrifice, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{edict, free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::{names, on_battlefield, pid};

#[derive(Clone, Copy, Debug)]
enum Death {
    Destroy,
    Combat,
    Sacrifice,
}

/// Seat 0 controls `aura` on `Mine`, a 2/2 of seat 1; `Mine` dies by `death`.
fn kill(aura: &str, seats: usize, death: Death) -> Result<Driver, ScriptError> {
    let mut mine = vec![
        attached_to(ready(card(aura).unwrap()), 1, 0),
        ready(swamp()),
        ready(swamp()),
    ];
    let mut hand = Vec::new();
    match death {
        Death::Destroy => hand.push(card("Doom Blade").unwrap()),
        Death::Sacrifice => hand.push(edict("Edict")),
        Death::Combat => mine.push(ready(free_creature("Brute", 9, 9))),
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(1, [ready(free_creature("Mine", 2, 2))])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    match death {
        Death::Destroy => {
            let target = driver.target_named("Mine")?;
            driver.cast_at("Doom Blade", &[target])?;
        }
        Death::Sacrifice => {
            let target = driver.target_player(1);
            driver.cast_at("Edict", &[target])?;
        }
        Death::Combat => {
            driver.idle_until(1, Step::DeclareAttackers)?;
            driver.attack(&[("Brute", 1)])?;
            driver.pass_until(Step::DeclareBlockers)?;
            driver.block(&[("Mine", "Brute")])?;
        }
    }
    driver.idle_until(1, Step::Main2)?;
    Ok(driver)
}

#[rstest]
fn fungal_fortitude_returns_the_creature_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(Death::Destroy, Death::Combat, Death::Sacrifice)] death: Death,
) -> Result<(), ScriptError> {
    let driver = kill("Fungal Fortitude", seats, death)?;
    let id = on_battlefield(driver.state(), "Mine").expect("Mine returned");
    assert!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert!(on_battlefield(driver.state(), "Fungal Fortitude").is_none());
    Ok(())
}

#[rstest]
fn fools_demise_returns_the_creature_and_itself(
    #[values(2, 3, 4)] seats: usize,
    #[values(Death::Destroy, Death::Combat, Death::Sacrifice)] death: Death,
) -> Result<(), ScriptError> {
    let driver = kill("Fool's Demise", seats, death)?;
    assert!(on_battlefield(driver.state(), "Mine").is_some());
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Fool's Demise"]);
    Ok(())
}

#[rstest]
fn auras_that_return_themselves_to_hand(
    #[values("Angelic Destiny", "Nurgle's Rot")] aura: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(Death::Destroy, Death::Combat, Death::Sacrifice)] death: Death,
) -> Result<(), ScriptError> {
    let driver = kill(aura, seats, death)?;
    assert!(on_battlefield(driver.state(), "Mine").is_none());
    assert!(names(driver.state(), Zone::Hand(pid(0))).contains(&aura.to_owned()));
    Ok(())
}
