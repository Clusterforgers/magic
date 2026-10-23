//! Goal: the tax of Norn's Annex ({W/P} for each attacker) is paid as the attack is declared at two to four seats, with white mana, with 2 life for each symbol the mana does not pay, and never with the last of the player's life: an attack whose only payment would take it is refused whole like any attack the player cannot pay for, and a smaller one is made; backing out of the payment returns to the declaration with the log and the state as they were before it, as often as the player backs out; and a bot that backs out does not declare the same attack again in that step, so a bot with many creatures and little life finishes the declaration.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, PresetPermanent};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Names of creatures with no abilities, each a 1/1.
fn creatures(names: &[&str]) -> Vec<PresetPermanent> {
    names
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect()
}

/// Seat 0 is about to declare with `mine` in play and `life`; every other seat holds a Norn's Annex.
fn scenario(seats: usize, mine: Vec<PresetPermanent>, life: i32) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .life(0, life);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(card("Norn's Annex").unwrap())]);
    }
    scenario
}

/// Seat 0 holds `plains_count` Plains and `mountain_count` Mountains beside the creatures `names`.
fn table(
    seats: usize,
    names: &[&str],
    plains_count: usize,
    mountain_count: usize,
    life: i32,
) -> Driver {
    let mut mine = creatures(names);
    mine.extend(lands(plains_count, plains));
    mine.extend(lands(mountain_count, mountain));
    Driver::from_scenario(scenario(seats, mine, life)).unwrap()
}

fn declared(driver: &Driver) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::AttackersDeclared { .. }))
}

fn tapped(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .filter(|object| object.permanent().is_some_and(PermanentState::tapped))
        .count()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn attack(driver: &mut Driver, names: &[&str]) -> Result<Vec<Event>, ScriptError> {
    let pairs: Vec<(&str, usize)> = names.iter().map(|name| (*name, 1)).collect();
    driver.attack(&pairs)
}

fn unaffordable(result: &Result<Vec<Event>, ScriptError>) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::AttackUnaffordable(_)
        )))
    )
}

fn pay_auto(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

// The payment

#[rstest]
#[case::white_mana_pays_each_symbol(2, 0, 20, &["A", "B"], Some((2, 20)))]
#[case::mana_pays_one_and_life_the_other(1, 0, 20, &["A", "B"], Some((1, 18)))]
#[case::life_pays_when_there_is_no_mana(0, 0, 20, &["A", "B"], Some((0, 16)))]
#[case::mana_of_another_colour_does_not_pay(0, 3, 20, &["A", "B"], Some((0, 16)))]
#[case::an_odd_life_total_keeps_its_last_point(0, 0, 5, &["A", "B"], Some((0, 1)))]
#[case::life_pays_one_symbol_of_three(2, 0, 3, &["A", "B", "C"], Some((2, 1)))]
#[case::one_life_cannot_pay_a_symbol(0, 0, 1, &["A"], None)]
#[case::the_last_life_does_not_pay_one_symbol(0, 0, 2, &["A"], None)]
#[case::the_last_life_does_not_pay_two_symbols(0, 0, 4, &["A", "B"], None)]
#[case::the_last_life_does_not_pay_what_the_mana_leaves(1, 0, 2, &["A", "B"], None)]
#[case::more_symbols_than_life_can_pay(0, 3, 4, &["A", "B", "C"], None)]
fn the_phyrexian_tax_is_paid_with_mana_or_with_life_that_leaves_some(
    #[values(2, 3, 4)] seats: usize,
    #[case] white: usize,
    #[case] red: usize,
    #[case] life_total: i32,
    #[case] attackers: &[&str],
    #[case] paid: Option<(usize, i32)>,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, attackers, white, red, life_total);
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let result = attack(&mut driver, attackers);
    let Some((plains_tapped, life_after)) = paid else {
        assert!(unaffordable(&result), "{result:?}");
        assert_eq!(
            driver.engine().state_hash(),
            hash,
            "the engine is as it was"
        );
        assert_eq!(driver.engine().events().len(), log);
        driver.expect_decision(0, "declare-attackers")?;
        return Ok(());
    };
    result?;
    driver.expect_decision(0, "pay-mana")?;
    assert!(
        driver.pay_options()?.auto_available,
        "an attack that is offered a payment can be paid automatically"
    );
    pay_auto(&mut driver)?;
    assert!(declared(&driver));
    assert_eq!(tapped(&driver, "Plains"), plains_tapped);
    assert_eq!(life(&driver), life_after);
    Ok(())
}

#[rstest]
fn a_smaller_attack_is_made_when_the_larger_one_would_take_the_last_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["A", "B"], 0, 0, 4);
    assert!(unaffordable(&attack(&mut driver, &["A", "B"])));
    attack(&mut driver, &["A"])?;
    pay_auto(&mut driver)?;
    assert!(declared(&driver));
    assert_eq!(life(&driver), 2);
    Ok(())
}

// Backing out

#[rstest]
fn backing_out_returns_to_the_declaration_as_often_as_the_player_does(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] times: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["A", "B"], 1, 0, 20);
    let before = driver.decision()?.clone();
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    for _ in 0..times {
        attack(&mut driver, &["A", "B"])?;
        driver.expect_decision(0, "pay-mana")?;
        assert!(driver.pay_options()?.can_cancel);
        driver.answer(Answer::PayMana(PayChoice::Cancel))?;
        assert_eq!(driver.decision()?, &before, "the same declaration is asked");
        assert_eq!(driver.engine().state_hash(), hash, "nothing was paid");
        assert_eq!(driver.engine().events().len(), log, "nothing was logged");
    }
    attack(&mut driver, &["A", "B"])?;
    pay_auto(&mut driver)?;
    assert!(declared(&driver));
    assert_eq!(life(&driver), 18);
    assert_eq!(tapped(&driver, "Plains"), 1);
    Ok(())
}

#[rstest]
fn a_player_who_backs_out_every_time_ends_the_step_with_the_attack_the_rules_require(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["A", "B", "C"], 0, 0, 20);
    for _ in 0..5 {
        attack(&mut driver, &["A", "B", "C"])?;
        driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    }
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("the declaration was expected");
    };
    let required = options.required_attack();
    assert!(required.is_empty(), "attacking taxed costs nothing to skip");
    driver.answer(Answer::DeclareAttackers(required))?;
    assert!(
        !matches!(
            driver.decision()?.kind,
            DecisionKind::DeclareAttackers(_) | DecisionKind::PayMana(_)
        ),
        "the declaration is over"
    );
    assert_eq!(life(&driver), 20);
    Ok(())
}

// The bots

/// What the bot answers, as a declaration.
fn declared_by(answer: &Answer) -> &[(mtg_core::ids::ObjectId, mtg_engine::combat::AttackTarget)] {
    match answer {
        Answer::DeclareAttackers(attackers) => attackers,
        other => panic!("a declaration was expected, not {other:?}"),
    }
}

#[rstest]
fn a_bot_that_backs_out_of_the_payment_does_not_declare_the_attack_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["A", "B", "C", "D"], 0, 4, 20);
    let declaration: Decision = driver.decision()?.clone();
    let DecisionKind::DeclareAttackers(options) = &declaration.kind else {
        panic!("the declaration was expected");
    };
    attack(&mut driver, &["A", "B"])?;
    let mut payment: Decision = driver.decision()?.clone();
    // A payment with nothing to answer but backing out: the automatic one is not there.
    let DecisionKind::PayMana(pay) = &mut payment.kind else {
        panic!("the payment was expected");
    };
    pay.auto_available = false;
    let mut taxed = 0;
    for seed in 0..40 {
        let mut bot = RandomBot::new(seed, 0);
        let first = bot.choose(&declaration);
        if options.cost_of(declared_by(&first)).symbols().is_empty() {
            continue;
        }
        taxed += 1;
        assert_eq!(
            bot.choose(&payment),
            Answer::PayMana(PayChoice::Cancel),
            "seed {seed}: backing out is all there is"
        );
        let again = bot.choose(&declaration);
        assert_eq!(
            declared_by(&again),
            options.required_attack(),
            "seed {seed}: the attack declared before is not declared again"
        );
    }
    assert!(
        taxed > 10,
        "the bot declared a taxed attack in {taxed} runs"
    );
    Ok(())
}

/// Seat 0 has twelve creatures, twelve Mountains and `life`; the bot declares until the step is over, answering at
/// most `limit` times. Returns whether it finished.
fn bot_declares(seats: usize, seed: u64, life: i32, limit: usize) -> bool {
    let names: Vec<String> = (0..12).map(|slot| format!("Fodder {slot}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut mine = creatures(&names);
    mine.extend(lands(12, mountain));
    let mut engine = scenario(seats, mine, life).build().unwrap();
    let mut bot = RandomBot::new(seed, 0);
    for _ in 0..limit {
        let decision = engine.decision().unwrap().clone();
        let ours = decision.player == PlayerId::from_index(0);
        if !ours
            || !matches!(
                decision.kind,
                DecisionKind::DeclareAttackers(_) | DecisionKind::PayMana(_)
            )
        {
            return true;
        }
        apply_answer(&mut engine, &decision, bot.choose(&decision)).unwrap();
    }
    false
}

#[rstest]
fn a_bot_with_many_creatures_and_little_life_finishes_the_declaration(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 4, 6, 20)] life: i32,
) {
    for seed in 0..25 {
        assert!(
            bot_declares(seats, seed, life, 60),
            "seed {seed} was still declaring after 60 answers"
        );
    }
}
