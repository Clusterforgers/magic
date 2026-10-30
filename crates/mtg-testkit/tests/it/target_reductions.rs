//! Goal: a reduction that reads the targets of a spell ("spells you cast that target a creature cost {2} less") makes the priority decision offer only spells that can find such a target, the announcement offers only the targets with which the cost can be paid and reaches the stack and a spell with no target is judged at its full cost, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::pid;
use mtg_core::ids::ObjectId;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityOptions, Unplayable};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 in its main phase with Killian, Ink Duelist, `islands` Islands, `plains` Plains and `hand`; seat 1 has a
/// Grizzly Bears.
fn table(seats: usize, islands: usize, plains_count: usize, hand: &[&str]) -> Driver {
    let mut battlefield: Vec<PresetPermanent> = lands(islands, island);
    battlefield.extend(lands(plains_count, plains));
    battlefield.push(ready(card("Killian, Ink Duelist").unwrap()));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, battlefield)
        .battlefield(1, vec![ready(card("Grizzly Bears").unwrap())]);
    Driver::from_scenario(scenario).unwrap()
}

/// The card `name` in seat 0's hand.
fn in_hand(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

fn priority(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

/// Whether the card `name` is offered, or else the reason the offer gives.
fn offer(driver: &Driver, name: &str) -> Result<(), Option<Unplayable>> {
    let id = in_hand(driver, name);
    let options = priority(driver);
    if options.castable.contains(&id) {
        return Ok(());
    }
    Err(options
        .unplayable
        .iter()
        .find(|card| card.card == id)
        .map(|card| card.reason))
}

#[rstest]
// Imprisoned in the Moon ({2}{U}) is an Aura that may enchant a creature: Killian takes {2} off.
#[case::aura_with_one_island("Imprisoned in the Moon", 1, 0, Ok(()))]
#[case::aura_with_no_blue("Imprisoned in the Moon", 0, 3, Err(Some(Unplayable::Mana)))]
// Duelist's Heritage ({2}{W}) targets nothing: Killian takes nothing off.
#[case::no_target_one_land_short("Duelist's Heritage", 0, 2, Err(Some(Unplayable::Mana)))]
#[case::no_target_one_plains("Duelist's Heritage", 0, 1, Err(Some(Unplayable::Mana)))]
#[case::no_target_paid_in_full("Duelist's Heritage", 0, 3, Ok(()))]
fn the_offer_counts_a_target_reduction_only_for_a_spell_that_can_find_its_target(
    #[case] name: &str,
    #[case] islands: usize,
    #[case] plains_count: usize,
    #[case] expected: Result<(), Option<Unplayable>>,
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats, islands, plains_count, &[name]);
    assert_eq!(offer(&driver, name), expected);
}

#[rstest]
fn the_targets_offered_are_those_the_reduction_pays_for_and_the_spell_reaches_the_stack(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = table(seats, 1, 0, &["Imprisoned in the Moon"]);
    let aura = in_hand(&driver, "Imprisoned in the Moon");
    driver.begin_cast("Imprisoned in the Moon").unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("the Aura asks for its target");
    };
    let mut offered: Vec<String> = options
        .legal
        .iter()
        .filter_map(|target| target.object())
        .map(|object| {
            driver
                .state()
                .object(object.id)
                .unwrap()
                .card()
                .name
                .clone()
        })
        .collect();
    offered.sort();
    // The Island would leave the cost at {2}{U}: only the creatures are offered.
    assert_eq!(offered, ["Grizzly Bears", "Killian, Ink Duelist"]);
    let bears = driver.target_named("Grizzly Bears").unwrap();
    driver.choose_targets(&[bears]).unwrap();
    assert_eq!(driver.pay_options().unwrap().cost.to_string(), "{U}");
    let events = driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { object, .. } if *object == aura))
    );
}
