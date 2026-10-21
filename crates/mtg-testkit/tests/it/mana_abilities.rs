//! Goal: the mana abilities of several mana, of as many mana as there are creatures, of a choice of colour with damage to their controller and of a sacrifice are sources of a payment: the engine pays with the fewest sources, the fewest sacrifices and the fewest side effects, mana beyond the cost floats, an exact manual payment is accepted and one with a part to spare is refused without a trace.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::mana::Payment;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A table where seat 0 holds the spell and has `mine` in play (cards of the corpus by name; a Mountain or a Plains is a
/// basic land of the testkit) and seat 1 has `theirs`.
fn table(cost: &str, mine: &[&str], theirs: &[&str]) -> Result<Driver, ScriptError> {
    let permanents: Vec<_> = mine
        .iter()
        .map(|name| match *name {
            "Mountain" => ready(mountain()),
            "Plains" => ready(plains()),
            other => ready(card(other).unwrap()),
        })
        .collect();
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [spell(cost)])
            .battlefield(0, permanents)
            .battlefield_named(1, theirs.to_vec()),
    )
}

fn castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => Ok(!options.castable.is_empty()),
        _ => Err(ScriptError::Expectation("priority was expected".into())),
    }
}

/// What a payment did: the permanents it used by name, those it sacrificed, the mana left floating and the life lost.
#[derive(Debug, PartialEq, Eq)]
struct Paid {
    used: Vec<String>,
    sacrificed: Vec<String>,
    floating: u32,
    damage: u32,
}

fn name_of(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// Casts the spell and pays automatically; what the events and the state show of the payment.
fn pay_automatically(driver: &mut Driver) -> Result<Paid, ScriptError> {
    let events = driver.cast("Spell")?;
    let mut used: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::PermanentTapped { object }
            | Event::ZoneChanged {
                object,
                cause: ZoneCause::Sacrificed,
                ..
            } => Some(name_of(driver, *object)),
            _ => None,
        })
        .collect();
    used.sort();
    used.dedup();
    let sacrificed: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::ZoneChanged {
                object,
                cause: ZoneCause::Sacrificed,
                ..
            } => Some(name_of(driver, *object)),
            _ => None,
        })
        .collect();
    let damage = events
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { amount, .. } => Some(*amount),
            _ => None,
        })
        .sum();
    let floating = driver.state().player(pid(0)).unwrap().mana().total();
    Ok(Paid {
        used,
        sacrificed,
        floating,
        damage,
    })
}

fn paid(used: &[&str], sacrificed: &[&str], floating: u32, damage: u32) -> Paid {
    Paid {
        used: used.iter().map(ToString::to_string).collect(),
        sacrificed: sacrificed.iter().map(ToString::to_string).collect(),
        floating,
        damage,
    }
}

#[rstest]
#[case::two_for_two("{2}", &["Sol Ring"], paid(&["Sol Ring"], &[], 0, 0))]
#[case::the_surplus_floats("{1}", &["Sol Ring"], paid(&["Sol Ring"], &[], 1, 0))]
#[case::a_land_with_it("{3}", &["Sol Ring", "Forest"], paid(&["Forest", "Sol Ring"], &[], 0, 0))]
#[case::a_colour_from_the_land("{1}{G}", &["Sol Ring", "Forest"], paid(&["Forest", "Sol Ring"], &[], 1, 0))]
#[case::two_rings("{4}", &["Sol Ring", "Sol Ring"], paid(&["Sol Ring"], &[], 0, 0))]
fn sol_ring_pays_for_two_and_what_it_adds_beyond_the_cost_floats(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] expected: Paid,
) -> Result<(), ScriptError> {
    let mut driver = table(cost, mine, &[])?;
    assert!(castable(&driver)?);
    let result = pay_automatically(&mut driver)?;
    if mine.iter().filter(|name| **name == "Sol Ring").count() == 2 {
        // Either ring pays: one is enough for the cost of two.
        assert_eq!(result.used.len(), 1, "{result:?}");
        assert_eq!((result.floating, result.damage), (0, 0));
    } else {
        assert_eq!(result, expected);
    }
    Ok(())
}

#[rstest]
#[case::the_ring_cannot_make_a_colour("{U}", &["Sol Ring"])]
#[case::too_little("{4}", &["Sol Ring", "Forest"])]
#[case::a_petal_makes_one("{3}", &["Lotus Petal", "Forest"])]
fn what_the_sources_cannot_make_is_not_castable(
    #[case] cost: &str,
    #[case] mine: &[&str],
) -> Result<(), ScriptError> {
    assert!(!castable(&table(cost, mine, &[])?)?);
    Ok(())
}

#[rstest]
#[case::no_creature(&[], &[], "{G}", false, 0)]
#[case::one_creature(&["Grizzly Bears"], &[], "{G}", true, 0)]
#[case::three_creatures(&["Grizzly Bears", "Grizzly Bears", "Grizzly Bears"], &[], "{G}{G}{G}", true, 0)]
#[case::the_surplus_floats(&["Grizzly Bears", "Grizzly Bears", "Grizzly Bears"], &[], "{2}", true, 1)]
#[case::two_are_not_three(&["Grizzly Bears", "Grizzly Bears"], &[], "{G}{G}{G}", false, 0)]
#[case::only_your_creatures_count(&["Grizzly Bears"], &["Grizzly Bears", "Grizzly Bears"], "{G}{G}", false, 0)]
fn gaeas_cradle_adds_a_mana_for_each_creature_its_controller_has(
    #[case] creatures: &[&str],
    #[case] foes: &[&str],
    #[case] cost: &str,
    #[case] can_cast: bool,
    #[case] floating: u32,
) -> Result<(), ScriptError> {
    let mine: Vec<&str> = std::iter::once("Gaea's Cradle")
        .chain(creatures.iter().copied())
        .collect();
    let mut driver = table(cost, &mine, foes)?;
    assert_eq!(castable(&driver)?, can_cast);
    if can_cast {
        let result = pay_automatically(&mut driver)?;
        assert_eq!(result.used, ["Gaea's Cradle"]);
        assert_eq!(result.floating, floating);
    }
    Ok(())
}

#[rstest]
#[case::the_colour_hurts("{R}", &["Battlefield Forge"], paid(&["Battlefield Forge"], &[], 0, 1))]
#[case::the_other_colour_hurts("{W}", &["Battlefield Forge"], paid(&["Battlefield Forge"], &[], 0, 1))]
#[case::colourless_does_not("{1}", &["Battlefield Forge"], paid(&["Battlefield Forge"], &[], 0, 0))]
#[case::a_land_without_pain_pays_first("{R}", &["Battlefield Forge", "Mountain"], paid(&["Mountain"], &[], 0, 0))]
#[case::the_pain_pays_what_the_other_cannot("{R}{W}", &["Battlefield Forge", "Mountain"], paid(&["Battlefield Forge", "Mountain"], &[], 0, 1))]
fn a_painland_deals_its_damage_only_when_its_colour_is_needed(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] expected: Paid,
) -> Result<(), ScriptError> {
    let mut driver = table(cost, mine, &[])?;
    assert!(castable(&driver)?);
    let result = pay_automatically(&mut driver)?;
    assert_eq!(result, expected);
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 - i32::try_from(expected.damage).unwrap());
    Ok(())
}

#[rstest]
#[case::lands_first("{1}", &["Lotus Petal", "Forest"], paid(&["Forest"], &[], 0, 0))]
#[case::the_petal_when_the_land_is_short("{2}", &["Lotus Petal", "Forest"], paid(&["Forest", "Lotus Petal"], &["Lotus Petal"], 0, 0))]
#[case::the_petal_for_a_colour_no_land_makes("{R}", &["Lotus Petal", "Forest"], paid(&["Lotus Petal"], &["Lotus Petal"], 0, 0))]
#[case::alone("{W}", &["Lotus Petal"], paid(&["Lotus Petal"], &["Lotus Petal"], 0, 0))]
#[case::a_ring_before_a_petal("{2}", &["Lotus Petal", "Sol Ring"], paid(&["Sol Ring"], &[], 0, 0))]
#[case::hybrid_beside_a_petal("{W/U}{W/U}", &["Lotus Petal", "Plains"], paid(&["Lotus Petal", "Plains"], &["Lotus Petal"], 0, 0))]
fn a_sacrifice_source_pays_only_when_nothing_else_can(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] expected: Paid,
) -> Result<(), ScriptError> {
    let mut driver = table(cost, mine, &[])?;
    assert!(castable(&driver)?);
    let result = pay_automatically(&mut driver)?;
    assert_eq!(result, expected);
    let state = driver.state();
    let gone = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .any(|o| o.card().name == "Lotus Petal");
    assert_eq!(gone, expected.sacrificed.is_empty());
    Ok(())
}

#[rstest]
#[case::exact(&["Sol Ring"], "{2}", true)]
#[case::the_surplus_of_the_last_source(&["Sol Ring"], "{1}", true)]
#[case::a_part_to_spare(&["Sol Ring", "Forest"], "{2}", false)]
#[case::too_little(&["Sol Ring"], "{3}", false)]
fn a_manual_payment_has_nothing_to_spare(
    #[case] taps: &[&str],
    #[case] cost: &str,
    #[case] accepted: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(cost, &["Sol Ring", "Forest"], &[])?;
    driver.begin_cast("Spell")?;
    let before = driver.engine().state_hash();
    let options = driver.pay_options()?.clone();
    let chosen = taps
        .iter()
        .map(|name| {
            options
                .sources
                .iter()
                .find(|o| name_of(&driver, o.source.object) == *name)
                .map(|o| o.source)
                .unwrap()
        })
        .collect();
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        pool: Vec::new(),
        taps: chosen,
        life: 0,
    })));
    if accepted {
        result?;
        assert_eq!(driver.state().stack().len(), 1);
    } else {
        assert!(result.is_err());
        assert_eq!(
            driver.engine().state_hash(),
            before,
            "a rejected payment changes nothing"
        );
        driver.expect_decision(0, "pay-mana")?;
    }
    Ok(())
}

#[test]
fn a_sol_ring_offers_its_two_mana_to_the_client_as_one_source() -> Result<(), ScriptError> {
    let mut driver = table("{2}", &["Sol Ring", "Forest"], &[])?;
    driver.begin_cast("Spell")?;
    let options = driver.pay_options()?;
    let counts: Vec<(ManaType, u32)> = options
        .sources
        .iter()
        .map(|o| (o.produces, o.count))
        .collect();
    assert!(counts.contains(&(ManaType::Colorless, 2)), "{counts:?}");
    assert!(counts.contains(&(ManaType::Green, 1)), "{counts:?}");
    Ok(())
}
