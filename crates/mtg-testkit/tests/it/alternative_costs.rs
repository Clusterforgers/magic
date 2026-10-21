//! Goal: alternative costs play whole: flashback casts a card from the graveyard for its flashback cost when the lands pay it and the timing of its type allows, asks nothing while it is the only way, and exiles the spell however it leaves the stack (resolved, countered); evoke is asked beside the mana cost, pays the evoke cost and sacrifices the permanent as it enters while its enters trigger still resolves; a way that cannot be paid is not offered; backing out leaves no trace; bot games with the cards keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::choose::{CardsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.pass()?;
    driver.pass()?;
    Ok(())
}

fn pay(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

/// Answers the question of the discard with the first `count` cards offered.
fn discard(driver: &mut Driver, count: usize) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the discard is asked: {:?}", driver.decision()?.kind.name());
    };
    let cards = options.from[..count].to_vec();
    driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
    Ok(())
}

/// Seat 0 has Faithless Looting in the graveyard, `lands` Mountains and a library to draw from.
fn looting(count: usize) -> Scenario {
    Scenario::new(2)
        .graveyard_named(0, ["Faithless Looting"])
        .hand(0, [island()])
        .battlefield(0, lands(count, mountain))
        .library(0, (0..6).map(|_| forest()))
}

#[rstest]
#[case::three_lands_pay_the_cost(3, true)]
#[case::two_lands_do_not(2, false)]
fn flashback_is_offered_when_the_lands_pay_its_cost(
    #[case] count: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(count))?;
    assert_eq!(driver.begin_cast("Faithless Looting").is_ok(), offered);
    Ok(())
}

#[test]
fn a_flashed_back_spell_asks_nothing_but_the_payment_and_is_exiled_after_it_resolves()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(3))?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{R}");
    pay(&mut driver)?;
    assert_eq!(driver.state().stack().len(), 1);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).is_empty());
    resolve(&mut driver)?;
    discard(&mut driver, 2)?;
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Faithless Looting"],
        "exiled instead of the graveyard"
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 2);
    assert!(driver.state().stack().is_empty());
    Ok(())
}

#[test]
fn a_flashed_back_spell_that_is_countered_is_exiled_too() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        looting(3)
            .hand_named(1, ["Counterspell"])
            .battlefield(1, lands(2, island)),
    )?;
    driver.cast("Faithless Looting")?;
    driver.pass()?;
    let spell = driver.state().stack()[0];
    let target = mtg_engine::choose::Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Counterspell", &[target])?;
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Faithless Looting"]);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Counterspell"]);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).is_empty());
    Ok(())
}

#[rstest]
#[case::main_phase_of_its_controller(0, Step::Main1, true)]
#[case::upkeep_is_no_time_for_a_sorcery(0, Step::Upkeep, false)]
#[case::an_opponents_turn(1, Step::Main1, false)]
fn flashback_keeps_the_timing_of_the_card_type(
    #[case] active: usize,
    #[case] step: Step,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(3).at(2, active, step))?;
    driver.idle_until(2, step)?;
    if active == 1 {
        driver.pass()?;
    }
    assert_eq!(driver.begin_cast("Faithless Looting").is_ok(), offered);
    Ok(())
}

#[test]
fn the_printed_cost_is_not_paid_from_the_graveyard_and_flashback_is_not_offered_from_the_hand()
-> Result<(), ScriptError> {
    // One Mountain pays the printed {R} but not the flashback {2}{R}, and the card is in the graveyard.
    let mut driver = Driver::from_scenario(looting(1))?;
    assert!(driver.begin_cast("Faithless Looting").is_err());
    // In the hand the card has one way only: the printed cost.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Faithless Looting"])
            .battlefield(0, lands(3, mountain))
            .library(0, (0..6).map(|_| forest())),
    )?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{R}");
    Ok(())
}

#[test]
fn backing_out_of_a_flashback_leaves_the_card_in_the_graveyard_and_no_trace() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(3))?;
    let before = driver.engine().events().len();
    driver.begin_cast("Faithless Looting")?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), before);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Faithless Looting"]);
    assert!(driver.begin_cast("Faithless Looting").is_ok(), "it can be cast again");
    Ok(())
}

/// Seat 0 holds Mulldrifter with `count` Islands and a library to draw from.
fn mulldrifter(count: usize) -> Scenario {
    Scenario::new(2)
        .hand_named(0, ["Mulldrifter"])
        .battlefield(0, lands(count, island))
        .library(0, (0..6).map(|_| forest()))
}

/// Answers the way to cast with the option labelled `label`.
fn choose_way(driver: &mut Driver, label: &str) -> Result<(), ScriptError> {
    let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
        panic!("the way is asked: {}", driver.decision()?.kind.name());
    };
    let at = options
        .options
        .iter()
        .position(|option| matches!(option, mtg_engine::choose::ChoiceOption::Label(found) if found.starts_with(label)))
        .unwrap_or_else(|| panic!("no way {label}: {:?}", options.options));
    driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    Ok(())
}

#[rstest]
#[case::evoked(5, Some("Evoke"), "{2}{U}", 2, 1, 0)]
#[case::cast_for_its_mana_cost(5, Some("Cast"), "{4}{U}", 2, 0, 1)]
#[case::only_the_evoke_cost_is_affordable(3, None, "{2}{U}", 2, 1, 0)]
fn mulldrifter_draws_two_and_is_sacrificed_only_when_evoked(
    #[case] count: usize,
    #[case] way: Option<&str>,
    #[case] cost: &str,
    #[case] drawn: usize,
    #[case] in_graveyard: usize,
    #[case] on_battlefield: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mulldrifter(count))?;
    driver.begin_cast("Mulldrifter")?;
    match way {
        Some(label) => choose_way(&mut driver, label)?,
        None => driver.expect_decision(0, "pay-mana")?,
    }
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    pay(&mut driver)?;
    resolve(&mut driver)?;
    // The enters trigger and the sacrifice trigger are put on the stack; both resolve.
    while !driver.state().stack().is_empty() || driver.state().zone(Zone::Hand(pid(0))).len() < drawn {
        if let DecisionKind::OrderTriggers(_) = driver.decision()?.kind {
            driver.order_triggers(&[0, 1])?;
        } else {
            driver.pass()?;
        }
    }
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), drawn);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), in_graveyard);
    assert_eq!(names(driver.state(), Zone::Battlefield).iter().filter(|n| *n == "Mulldrifter").count(), on_battlefield);
    Ok(())
}

#[test]
fn the_cost_of_a_way_that_cannot_be_paid_is_not_offered_and_nothing_is_asked() -> Result<(), ScriptError> {
    // Two Islands pay neither {4}{U} nor {2}{U}.
    let mut driver = Driver::from_scenario(mulldrifter(2))?;
    assert!(driver.begin_cast("Mulldrifter").is_err());
    // Three Islands pay only the evoke cost: the way is taken without a question.
    let mut driver = Driver::from_scenario(mulldrifter(3))?;
    driver.begin_cast("Mulldrifter")?;
    assert_eq!(driver.decision()?.kind.name(), "pay-mana");
    Ok(())
}

#[test]
fn the_card_cast_for_evoke_remembers_it_on_the_stack() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mulldrifter(5))?;
    driver.begin_cast("Mulldrifter")?;
    choose_way(&mut driver, "Evoke")?;
    let events = pay(&mut driver)?;
    let cast = events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            _ => None,
        })
        .expect("the cast is logged");
    assert_eq!(cast.way, Some(mtg_cards::value::CastWay::Evoke));
    Ok(())
}
