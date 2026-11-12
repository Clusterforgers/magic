//! Goal: mana that costs a mill (Millikin, The Warring Triad), counts the hand of a target player (Rousing Refrain), removes X counters for X mana of one color (Haruspex) or comes with the attack of a creature or the one it equips (Tanuki Transplanter) is worked out when the ability is offered or resolves, pays its costs in the same step, and replays the same at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, with_counters};
use rstest::rstest;

/// A creature spell with this cost.
fn spell(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 holds `hand` and controls `permanents`; every seat has a library of `library` cards; turn 1, precombat main.
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<mtg_engine::PresetPermanent>,
    library: usize,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, permanents);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..library).map(|_| forest()));
    }
    Driver::from_scenario(scenario)
}

fn size(driver: &Driver, zone: Zone) -> usize {
    driver.state().zone(zone).len()
}

fn pool(driver: &Driver, seat: usize, mana: ManaType) -> u32 {
    driver.state().player(pid(seat)).unwrap().mana().get(mana)
}

/// Passes until nothing is on the stack, answering whatever the game asks passively.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not resolve");
}

#[rstest]
fn millikin_mills_the_top_card_for_one_colorless_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, Vec::new(), vec![ready(card("Millikin").unwrap())], 6)?;
    let top = driver.state().zone(Zone::Library(pid(0))).last().copied();
    driver.tap_for_mana("Millikin")?;
    assert_eq!(pool(&driver, 0, ManaType::Colorless), 1);
    assert_eq!(size(&driver, Zone::Library(pid(0))), 5);
    assert_eq!(size(&driver, Zone::Graveyard(pid(0))), 1);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).last().copied(),
        top,
        "the top card"
    );
    assert_eq!(size(&driver, Zone::Library(pid(1))), 6, "only your library");
    Ok(())
}

#[rstest]
fn millikin_is_not_offered_with_an_empty_library(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, Vec::new(), vec![ready(card("Millikin").unwrap())], 0)?;
    assert!(driver.tap_for_mana("Millikin").is_err());
    Ok(())
}

#[rstest]
#[case::two(2, 2)]
#[case::three(3, 3)]
fn several_millikins_in_one_payment_mill_different_cards(
    #[case] millikins: usize,
    #[case] cost: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let permanents = (0..millikins)
        .map(|_| ready(card("Millikin").unwrap()))
        .collect();
    let mut driver = table(
        seats,
        vec![spell("Pair", &format!("{{{cost}}}"))],
        permanents,
        8,
    )?;
    driver.cast("Pair")?;
    assert_eq!(size(&driver, Zone::Library(pid(0))), 8 - millikins);
    assert_eq!(size(&driver, Zone::Graveyard(pid(0))), millikins);
    assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
    Ok(())
}

#[rstest]
#[case::to_an_opponent(1)]
#[case::to_yourself(0)]
fn the_warring_triad_mills_a_card_and_gives_the_target_player_mana(
    #[case] target: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        Vec::new(),
        vec![ready(card("The Warring Triad").unwrap())],
        5,
    )?;
    let ability = *driver
        .offered_abilities("The Warring Triad")?
        .first()
        .expect("the ability is offered");
    driver.begin_ability("The Warring Triad", ability)?;
    let player = driver.target_player(target);
    driver.choose_targets(&[player])?;
    resolve(&mut driver)?;
    assert_eq!(
        size(&driver, Zone::Graveyard(pid(0))),
        1,
        "milled as a cost"
    );
    let mana = |seat| {
        [
            ManaType::White,
            ManaType::Blue,
            ManaType::Black,
            ManaType::Red,
            ManaType::Green,
        ]
        .iter()
        .map(|kind| pool(&driver, seat, *kind))
        .sum::<u32>()
    };
    assert_eq!(mana(target), 1, "one mana of a color for the target player");
    assert_eq!(mana(1 - target), 0);
    Ok(())
}

#[rstest]
#[case::empty_hand(0)]
#[case::one_card(1)]
#[case::four_cards(4)]
fn rousing_refrain_adds_red_for_each_card_in_the_targets_hand_keeps_it_and_suspends_itself(
    #[case] cards: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Rousing Refrain").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(mountain())))
        .hand(1, (0..cards).map(|_| forest()));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(1);
    driver.cast_at("Rousing Refrain", &[target])?;
    resolve(&mut driver)?;
    let cards = u32::try_from(cards).unwrap();
    assert_eq!(pool(&driver, 0, ManaType::Red), cards);
    // The mana stays through the steps of the turn.
    driver.pass_until(Step::End)?;
    assert_eq!(
        pool(&driver, 0, ManaType::Red),
        cards,
        "kept to the end of the turn"
    );
    let refrain = driver
        .state()
        .objects()
        .find(|object| object.card().name == "Rousing Refrain")
        .expect("the card exists");
    assert!(
        driver.state().zone(Zone::Exile).contains(&refrain.id()),
        "exiled, not in the graveyard"
    );
    assert_eq!(
        refrain.card_counters_of(&CounterKind::Other("time".to_owned())),
        3,
        "three time counters"
    );
    Ok(())
}

#[rstest]
fn a_refrain_exiled_with_time_counters_loses_one_each_upkeep_of_its_owner(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Rousing Refrain").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(mountain())));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(1);
    driver.cast_at("Rousing Refrain", &[target])?;
    resolve(&mut driver)?;
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    let refrain = driver
        .state()
        .objects()
        .find(|object| object.card().name == "Rousing Refrain")
        .expect("the card exists");
    assert_eq!(
        refrain.card_counters_of(&CounterKind::Other("time".to_owned())),
        2,
        "one counter removed in the upkeep of its owner"
    );
    Ok(())
}

#[rstest]
#[case::the_creature_itself(false, 2)]
#[case::the_creature_it_equips(true, 5)]
fn tanuki_transplanter_adds_green_equal_to_the_power_of_the_attacker_and_keeps_it(
    #[case] equipped: bool,
    #[case] power: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = Vec::new();
    let attacker = if equipped {
        permanents.push(ready(free_creature("Host", 5, 5)));
        permanents.push(attached_to(
            ready(card("Tanuki Transplanter").unwrap()),
            0,
            0,
        ));
        "Host"
    } else {
        permanents.push(ready(card("Tanuki Transplanter").unwrap()));
        "Tanuki Transplanter"
    };
    let mut driver = table(seats, Vec::new(), permanents, 6)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    resolve(&mut driver)?;
    assert_eq!(pool(&driver, 0, ManaType::Green), power);
    driver.pass_until(Step::End)?;
    assert_eq!(
        pool(&driver, 0, ManaType::Green),
        power,
        "kept to the end of the turn"
    );
    Ok(())
}

#[rstest]
#[case::all_three(3)]
#[case::two_of_three(2)]
#[case::one(1)]
fn haruspex_removes_x_counters_for_x_mana_of_one_color(
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        Vec::new(),
        vec![with_counters(
            ready(card("Haruspex").unwrap()),
            CounterKind::PlusOnePlusOne,
            3,
        )],
        6,
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    let source = options
        .mana_abilities
        .iter()
        .find(|option| option.produces == ManaType::Green && option.count == x)
        .map(|option| option.source)
        .expect("an option for this X");
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(source)))?;
    assert_eq!(pool(&driver, 0, ManaType::Green), x);
    let haruspex = driver.id_of("Haruspex")?;
    let left = driver
        .state()
        .object(haruspex)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(left, 3 - x, "X counters removed");
    Ok(())
}
