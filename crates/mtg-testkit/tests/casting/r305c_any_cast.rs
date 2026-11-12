//! Goal: a permission that opens no zone (Dracogenesis, Jodah, Archmage Eternal, Fist of Suns) gives its free cast or its cost to every cast its controller may make anyway, at 2 to 4 seats: a Dragon from the hand or the commander from the command zone is cast free, a Dragon in the graveyard stays uncastable, a spell is offered for {W}{U}{B}{R}{G} beside its mana cost and is paid that way, an opponent gets nothing; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, pay, ways};
use crate::common::{names, pid};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const DRAGON: &str = "Ancient Copper Dragon";
const COMMANDER: &str = "The Ur-Dragon";

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat `holder` controls `permission`; seat 0 holds the Dragon in its hand or graveyard, has The Ur-Dragon as its
/// commander and controls the lands `lands`.
fn table(
    seats: usize,
    holder: usize,
    permission: &str,
    in_hand: bool,
    with_lands: bool,
) -> Scenario {
    let mut lands: Vec<_> = if with_lands {
        vec![
            ready(plains()),
            ready(island()),
            ready(swamp()),
            ready(mountain()),
            ready(mountain()),
            ready(forest()),
        ]
    } else {
        Vec::new()
    };
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [card(COMMANDER).unwrap()]);
    if holder == 0 {
        lands.push(ready(card(permission).unwrap()));
    } else {
        scenario = scenario.battlefield(holder, [ready(card(permission).unwrap())]);
    }
    let scenario = scenario.battlefield(0, lands);
    if in_hand {
        scenario.hand(0, [card(DRAGON).unwrap()])
    } else {
        scenario.graveyard(0, [card(DRAGON).unwrap()])
    }
}

/// Passes until the stack is empty.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts `name` the way whose label starts with `way`, pays and resolves.
fn cast_by(driver: &mut Driver, name: &str, way: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, way)?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        pay(driver)?;
    }
    resolve(driver)
}

#[rstest]
#[case::dragon_from_hand(DRAGON, true, true)]
#[case::dragon_from_graveyard(DRAGON, false, false)]
#[case::commander_from_command_zone(COMMANDER, true, true)]
fn dracogenesis_casts_free_only_where_a_cast_is_open(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] in_hand: bool,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, 0, "Dracogenesis", in_hand, false)
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(table(seats, 0, "Dracogenesis", in_hand, false))?;
    if !castable {
        assert!(
            driver.begin_cast(name).is_err(),
            "nothing opens the graveyard"
        );
        return Ok(());
    }
    cast_by(&mut driver, name, "Cast without paying")?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&name.to_owned()));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_opponents_dracogenesis_gives_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1, "Dracogenesis", true, false))?;
    assert!(driver.begin_cast(DRAGON).is_err());
    Ok(())
}

#[rstest]
fn the_five_color_cost_stands_beside_the_mana_cost(
    #[values(2, 3, 4)] seats: usize,
    #[values("Jodah, Archmage Eternal", "Fist of Suns")] permission: &str,
) -> Result<(), ScriptError> {
    let setup = table(seats, 0, permission, true, true).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, 0, permission, true, true))?;
    driver.begin_cast(DRAGON)?;
    assert_eq!(
        ways(&driver)?,
        ["Cast for {4}{R}{R}", "Cast for {W}{U}{B}{R}{G}"]
    );
    choose_way(&mut driver, "Cast for {W}")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{W}{U}{B}{R}{G}");
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&DRAGON.to_owned()));
    // Five lands paid: the spell cost {W}{U}{B}{R}{G}, not six mana.
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(0)
                && object
                    .permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(tapped, 5);
    replays(&driver, &setup);
    Ok(())
}
