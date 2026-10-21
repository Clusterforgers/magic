//! Goal: a card with madness that a player discards by any cause (a spell, a cost, the hand size at cleanup) is exiled in the open instead of reaching the graveyard, its owner's trigger asks whether to cast it for its madness cost with every question of the cast asked inside the resolution, a card that is not cast, or cannot be (no mana, no legal target), goes to the graveyard, a madness cost of nothing needs no payment, and the discard still counts as a discard for what watches for it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay, resolve};
use crate::common::{names, pid};

/// How the card is discarded.
#[derive(Debug, Clone, Copy)]
enum Cause {
    /// Faithless Looting makes its controller discard two cards.
    Spell,
    /// Rummaging Goblin discards a card to draw one.
    Cost,
    /// The hand is over seven cards at cleanup.
    HandSize,
}

/// Seat 0 holds `card` and discards it by `cause` with `mountains` Mountains to pay with; the madness trigger is on the
/// stack when this returns.
fn discarded(card: &str, cause: Cause, mountains: usize) -> Result<Driver, ScriptError> {
    let base = Scenario::new(2).library(0, vec![forest(); 6]);
    let scenario = match cause {
        Cause::Spell => base
            .hand_named(0, ["Faithless Looting", card])
            .battlefield(0, lands(mountains, mountain)),
        Cause::Cost => base
            .hand_named(0, [card])
            .battlefield(0, with_goblin(lands(mountains, mountain))),
        Cause::HandSize => base
            .at(1, 0, Step::End)
            .hand(0, hand_of_eight(card))
            .battlefield(0, lands(mountains, mountain)),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    match cause {
        Cause::Spell => {
            driver.cast("Faithless Looting")?;
            resolve(&mut driver)?;
            let id = in_hand(&driver, card);
            let other = driver
                .state()
                .zone(Zone::Hand(pid(0)))
                .iter()
                .copied()
                .find(|other| *other != id)
                .expect("a second card to discard");
            driver.answer(Answer::ChooseCards(CardsChoice(vec![id, other])))?;
        }
        Cause::Cost => {
            driver.begin_activation("Rummaging Goblin")?;
        }
        Cause::HandSize => {
            driver.pass()?;
            driver.pass()?;
            let id = in_hand(&driver, card);
            driver.answer(Answer::Discard(vec![id]))?;
        }
    }
    Ok(driver)
}

/// The Rummaging Goblin, ready to use its ability.
fn goblin() -> std::sync::Arc<mtg_cards::CardDefinition> {
    mtg_testkit::corpus::card("Rummaging Goblin").expect("the corpus has the Goblin")
}

/// The permanents `others` and a Rummaging Goblin ready to use its ability.
fn with_goblin(mut others: Vec<mtg_engine::PresetPermanent>) -> Vec<mtg_engine::PresetPermanent> {
    others.insert(0, ready(goblin()));
    others
}

/// `card` and seven Forests: one card over the hand size.
fn hand_of_eight(card: &str) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    let mut hand = vec![mtg_testkit::corpus::card(card).expect("the corpus has the card")];
    hand.extend(vec![forest(); 7]);
    hand
}

/// The card called `name` in seat 0's hand.
fn in_hand(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| crate::common::name_of(driver.state(), *id) == name)
        .expect("the card is in hand")
}

fn yes(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::YesNo(YesNoChoice(yes)))
}

/// The text of the yes or no question pending.
fn question(driver: &Driver) -> String {
    let DecisionKind::YesNo(question) = &driver.decision().unwrap().kind else {
        panic!("a question is asked: {:?}", driver.decision().unwrap().kind.name());
    };
    question.text.clone()
}

#[rstest]
#[case::a_spell(Cause::Spell)]
#[case::a_cost(Cause::Cost)]
#[case::the_hand_size(Cause::HandSize)]
fn a_discarded_madness_card_waits_in_exile_and_is_cast_for_its_madness_cost(
    #[case] cause: Cause,
) -> Result<(), ScriptError> {
    let mut driver = discarded("Fiery Temper", cause, 2)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Fiery Temper"]);
    assert!(!names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Fiery Temper".to_owned()));
    resolve(&mut driver)?;
    assert_eq!(
        question(&driver),
        "Cast Fiery Temper for its madness cost {R}?"
    );
    yes(&mut driver, true)?;
    let target = driver.target_player(1);
    let events = driver.choose_targets(&[target])?;
    assert!(events.is_empty() || !events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    let events = pay(&mut driver)?;
    let way = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.way),
        _ => None,
    });
    assert_eq!(way, Some(Some(CastWay::Madness)));
    assert!(names(driver.state(), Zone::Stack).contains(&"Fiery Temper".to_owned()));
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))).last().map(String::as_str),
        Some("Fiery Temper")
    );
    Ok(())
}

#[rstest]
#[case::a_spell(Cause::Spell)]
#[case::a_cost(Cause::Cost)]
fn a_madness_card_that_is_not_cast_goes_to_the_graveyard(
    #[case] cause: Cause,
) -> Result<(), ScriptError> {
    let mut driver = discarded("Fiery Temper", cause, 2)?;
    resolve(&mut driver)?;
    yes(&mut driver, false)?;
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Fiery Temper".to_owned()));
    Ok(())
}

#[test]
fn a_madness_cost_that_cannot_be_paid_is_not_asked_and_the_card_goes_to_the_graveyard()
-> Result<(), ScriptError> {
    // Faithless Looting takes the only Mountain.
    let mut driver = discarded("Fiery Temper", Cause::Spell, 1)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Fiery Temper"]);
    resolve(&mut driver)?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)),
        "nothing is asked of a player who cannot pay"
    );
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Fiery Temper".to_owned()));
    Ok(())
}

#[test]
fn a_madness_cost_of_nothing_is_cast_without_a_payment() -> Result<(), ScriptError> {
    let mut driver = discarded("Basking Rootwalla", Cause::Cost, 0)?;
    resolve(&mut driver)?;
    assert_eq!(
        question(&driver),
        "Cast Basking Rootwalla for its madness cost {0}?"
    );
    yes(&mut driver, true)?;
    assert_eq!(names(driver.state(), Zone::Stack).last().map(String::as_str), Some("Basking Rootwalla"));
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Basking Rootwalla".to_owned()));
    Ok(())
}

#[test]
fn a_creature_with_madness_is_cast_for_its_cost_and_enters_the_battlefield() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .library(0, vec![forest(); 6])
        .hand_named(0, ["Arrogant Wurm"])
        .battlefield(
            0,
            with_goblin(vec![ready(forest()), ready(forest()), ready(forest())]),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Rummaging Goblin")?;
    resolve(&mut driver)?;
    assert_eq!(
        question(&driver),
        "Cast Arrogant Wurm for its madness cost {2}{G}?"
    );
    yes(&mut driver, true)?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Arrogant Wurm".to_owned()));
    Ok(())
}

#[test]
fn the_discard_of_a_madness_card_still_triggers_what_watches_for_discards() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .library(0, vec![forest(); 6])
        .hand_named(0, ["Fiery Temper"])
        .battlefield(0, with_goblin(Vec::new()))
        .battlefield_named(1, ["Megrim"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Rummaging Goblin")?;
    for _ in 0..4 {
        if driver.state().player(pid(0)).unwrap().life() < 20 {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        18,
        "Megrim saw the discard"
    );
    Ok(())
}
