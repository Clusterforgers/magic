//! Goal: corpus cards of the r30-5 pack play as their rules say at two and four seats: Aven Interrupter taxes the spells an opponent casts from a graveyard or from exile and no other; Yavimaya Kavu and Fungal Behemoth count their power and toughness; Rampant Frogantua grows for each player who lost; Megatron keeps opponents from casting spells during combat only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::SpendAs;
use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Duration;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// The card named `name` in the zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .unwrap()
}

/// Whether the seat holding priority may cast the card now.
fn castable(driver: &Driver, id: ObjectId) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&id),
        _ => false,
    }
}

/// Lightning Bolt moved from seat 0's hand to `zone` with a rule that lets seat 0 cast it from there; seat 0 has
/// `mountains` Mountains and the Interrupter is on the battlefield of seat `interrupter`.
fn bolt_from(
    seats: usize,
    zone: ZoneKind,
    mountains: usize,
    interrupter: usize,
) -> Result<(Driver, ObjectId), ScriptError> {
    let mut mine: Vec<_> = (0..mountains).map(|_| ready(mountain())).collect();
    let aven = ready(corpus::card("Aven Interrupter").unwrap());
    let scenario = Scenario::new(seats).hand_named(0, ["Lightning Bolt"]);
    let scenario = if interrupter == 0 {
        mine.push(aven);
        scenario.battlefield(0, mine)
    } else {
        scenario
            .battlefield(0, mine)
            .battlefield(interrupter, [aven])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    let bolt = find(&driver, Zone::Hand(pid(0)), "Lightning Bolt");
    let to = if zone == ZoneKind::Exile {
        ManualZone::Exile
    } else {
        ManualZone::Graveyard
    };
    driver.manual(ManualAction::Move { object: bolt, to })?;
    // A permission needs a source; the first Mountain stands in for the card that grants it.
    let granter = find(&driver, Zone::Battlefield, "Mountain");
    driver.add_rule_from(
        granter,
        Rule::MayCast {
            who: vec![pid(0)],
            what: Selector(vec![Pred::In(zone)]),
            from: Some(zone),
            free: false,
            flash: false,
            only: None,
            play: false,
            once: false,
            any_turn: false,
            additional: None,
            rather: None,
            spend_as: SpendAs::Normal,
            when: None,
            any_cast: false,
            riders: Vec::new(),
        },
        Duration::UntilEndOfTurn,
    )?;
    let place = if zone == ZoneKind::Exile {
        Zone::Exile
    } else {
        Zone::Graveyard(pid(0))
    };
    let bolt = find(&driver, place, "Lightning Bolt");
    Ok((driver, bolt))
}

#[rstest]
fn aven_interrupter_taxes_an_opponents_spells_from_graveyards_and_exile(
    #[values(2, 4)] seats: usize,
    #[values(ZoneKind::Exile, ZoneKind::Graveyard)] zone: ZoneKind,
) -> Result<(), ScriptError> {
    let (one, bolt) = bolt_from(seats, zone, 1, 1)?;
    assert!(!castable(&one, bolt), "one Mountain pays no {{2}} more");
    let (three, bolt) = bolt_from(seats, zone, 3, 1)?;
    assert!(castable(&three, bolt), "three Mountains pay the tax");
    // Near miss: its controller's own spells cost no more.
    let (own, bolt) = bolt_from(seats, zone, 1, 0)?;
    assert!(castable(&own, bolt));
    Ok(())
}

#[rstest]
fn yavimaya_kavu_counts_red_and_green_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield_named(0, ["Yavimaya Kavu", "Grizzly Bears", "Grizzly Bears"])
            .battlefield_named(1, ["Goblin Cadets", "Goblin Cadets"]),
    )?;
    let kavu = find(&driver, Zone::Battlefield, "Yavimaya Kavu");
    let seen = mtg_engine::characteristics(driver.state(), kavu).unwrap();
    // Red: the Kavu and both Cadets; green: the Kavu and both Bears.
    assert_eq!((seen.power, seen.toughness), (Some(3), Some(3)));
    Ok(())
}

#[rstest]
fn fungal_behemoth_counts_plus_one_counters_on_your_creatures(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bears = || corpus::card("Grizzly Bears").unwrap();
    let plus = CounterKind::PlusOnePlusOne;
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(corpus::card("Fungal Behemoth").unwrap()),
                    with_counters(ready(bears()), plus.clone(), 2),
                    with_counters(ready(bears()), plus.clone(), 1),
                ],
            )
            .battlefield(1, [with_counters(ready(bears()), plus, 5)]),
    )?;
    let behemoth = find(&driver, Zone::Battlefield, "Fungal Behemoth");
    let seen = mtg_engine::characteristics(driver.state(), behemoth).unwrap();
    // The opponent's counters are not counted.
    assert_eq!((seen.power, seen.toughness), (Some(3), Some(3)));
    Ok(())
}

/// Seat 1 holds priority with Lightning Bolt and a Mountain in the step, Megatron on seat 0's battlefield.
fn bolt_in(seats: usize, step: Step) -> Result<(Driver, ObjectId), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, step)
            .battlefield_named(0, ["Megatron, Tyrant"])
            .hand_named(1, ["Lightning Bolt"])
            .battlefield(1, [ready(mountain())]),
    )?;
    let bolt = find(&driver, Zone::Hand(pid(1)), "Lightning Bolt");
    driver.pass()?;
    Ok((driver, bolt))
}

#[rstest]
fn megatron_stops_opponents_casting_during_combat(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (combat, bolt) = bolt_in(seats, Step::BeginCombat)?;
    assert!(!castable(&combat, bolt));
    let (main, bolt) = bolt_in(seats, Step::Main1)?;
    assert!(castable(&main, bolt));
    Ok(())
}

#[rstest]
#[case::three_seats_one_out(3, 1)]
#[case::four_seats_two_out(4, 2)]
#[case::two_seats_none_out(2, 0)]
fn rampant_frogantua_grows_for_each_player_who_lost(
    #[case] seats: usize,
    #[case] out: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield_named(0, ["Rampant Frogantua"]))?;
    for seat in 0..out {
        driver.concede(pid(seats - 1 - seat))?;
    }
    let frog = find(&driver, Zone::Battlefield, "Rampant Frogantua");
    let printed = corpus::card("Rampant Frogantua").unwrap();
    let seen = mtg_engine::characteristics(driver.state(), frog).unwrap();
    let bonus = i32::try_from(out * 10).unwrap();
    assert_eq!(
        (seen.power, seen.toughness),
        (
            printed.power.map(|p| p + bonus),
            printed.toughness.map(|t| t + bonus)
        )
    );
    Ok(())
}
