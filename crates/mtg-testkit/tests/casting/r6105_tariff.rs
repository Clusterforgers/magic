//! Goal: Tariff makes each player, in turn order, keep the creature they control with the greatest mana value by paying
//! its mana cost or sacrifice it, all sacrificed together; a tie is the player's choice and a creature with no mana cost
//! can't be kept (CR 202.1b), at two to four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A 1/1 creature with no mana cost.
fn r6105_costless() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Costless Golem")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// What `seat` controls: seat 0 casts Tariff with its two Plains and keeps a bear it can't pay for; seat 1 can pay for
/// its Serra Angel; seat 2 could but declines; seat 3 has two tied bears and a costless creature.
fn r6105_board(seat: usize) -> Vec<PresetPermanent> {
    let lands = |count: usize| (0..count).map(|_| ready(plains())).collect::<Vec<_>>();
    match seat {
        0 => [vec![ready(bear())], lands(2)].concat(),
        1 | 2 => [
            vec![ready(card("Serra Angel").unwrap()), ready(bear())],
            lands(5),
        ]
        .concat(),
        _ => vec![ready(bear()), ready(bear()), ready(r6105_costless())],
    }
}

/// The names of the creatures `seat` controls, sorted.
fn r6105_creatures(driver: &Driver, seat: usize) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name != "Plains")
        .map(|object| object.card().name.clone())
        .collect();
    names.sort();
    names
}

#[rstest]
fn r6105_tariff_keeps_what_is_paid_for(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Tariff").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, r6105_board(seat));
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Tariff")?;
    let mut asked = Vec::new();
    for _ in 0..120 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        let seat = (0..seats)
            .find(|seat| pid(*seat) == decision.player)
            .unwrap();
        match &decision.kind {
            DecisionKind::YesNo(options) => {
                asked.push((seat, options.only_no));
                driver.answer(Answer::YesNo(YesNoChoice(seat == 1)))?;
            }
            DecisionKind::ChooseCards(options) => {
                assert_eq!(seat, 3, "only the tie is a choice");
                assert_eq!(options.from.len(), 2, "the two bears tie");
                driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(driver.state().stack().is_empty());
    // The caster tapped out: they can only decline.
    assert!(
        asked
            .iter()
            .all(|(seat, only_no)| *only_no == (*seat == 0 || *seat == 3))
    );
    assert_eq!(r6105_creatures(&driver, 0), Vec::<String>::new());
    assert_eq!(
        r6105_creatures(&driver, 1),
        ["Grizzly Bears", "Serra Angel"]
    );
    if seats > 2 {
        assert_eq!(r6105_creatures(&driver, 2), ["Grizzly Bears"]);
    }
    if seats > 3 {
        assert_eq!(
            r6105_creatures(&driver, 3),
            ["Costless Golem", "Grizzly Bears"]
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r6105_a_creature_without_mana_cost_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Tariff").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(plains())).collect::<Vec<_>>())
        .battlefield(1, [ready(r6105_costless())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Tariff")?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::YesNo(options) = &decision.kind {
            assert!(options.only_no, "no mana cost is an unpayable cost");
            assert!(driver.answer(Answer::YesNo(YesNoChoice(true))).is_err());
            driver.answer(Answer::YesNo(YesNoChoice(false)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(r6105_creatures(&driver, 1), Vec::<String>::new());
    Ok(())
}
