//! Goal: a source with `GameRule::PayToIgnore` lets any player, at any time they have priority, pay its price as a
//! special action (no stack) to ignore its other effects until end of turn, for that player alone: under Leonin
//! Arbiter nobody may search until they paid {2}; a payer searches this turn, the others still can't, the payment is
//! offered once a turn per player and lapses as the next turn begins, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A free sorcery: "Search your library for a card, put it into your hand, then shuffle."
fn tutor(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"effects":[{"Search":{"filter":[],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":"Hand"}},{"Shuffle":{}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// The sources the deciding player may pay to ignore now.
fn ignorable(driver: &Driver) -> Vec<mtg_core::ids::ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.ignorable.iter().map(|o| o.object).collect(),
        _ => Vec::new(),
    }
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat `active` has two Wastes, a Tutor and a library with a Bear; the Arbiter is on seat `owner`'s battlefield.
fn table(seats: usize, owner: usize, active: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).at(2, active, Step::Main1);
    for seat in 0..seats {
        let mut board = vec![ready(wastes()), ready(wastes())];
        if seat == owner {
            board.push(ready(card("Leonin Arbiter").unwrap()));
        }
        scenario = scenario
            .battlefield(seat, board)
            .hand(seat, [tutor(&format!("Tutor {seat}"))])
            .library(
                seat,
                [free_creature(&format!("Bear {seat}"), 2, 2), forest()],
            );
    }
    Driver::from_scenario(scenario)
}

/// Whether the active player's tutor found a card: their hand grew by one after casting it.
fn searches(driver: &mut Driver, seat: usize) -> Result<bool, ScriptError> {
    let before = names(driver.state(), Zone::Hand(pid(seat))).len();
    driver.cast(&format!("Tutor {seat}"))?;
    settle(driver)?;
    let after = names(driver.state(), Zone::Hand(pid(seat))).len();
    Ok(after == before)
}

#[rstest]
fn only_the_payer_searches_this_turn(
    #[values(0, 1)] owner: usize,
    #[values(true, false)] pays: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, owner, 1)?;
    let arbiter = driver.id_of("Leonin Arbiter")?;
    assert_eq!(
        ignorable(&driver),
        [arbiter],
        "any player may pay, the controller or not"
    );
    if pays {
        let stack_before = driver.state().stack().len();
        driver.answer(Answer::Priority(PriorityAction::IgnoreEffect(arbiter)))?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        assert_eq!(
            driver.state().stack().len(),
            stack_before,
            "a special action uses no stack"
        );
        assert!(
            ignorable(&driver).is_empty(),
            "paid once is enough this turn"
        );
    }
    assert_eq!(searches(&mut driver, 1)?, pays);
    Ok(())
}

#[rstest]
fn another_players_payment_frees_only_them(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, 0, 1)?;
    let arbiter = driver.id_of("Leonin Arbiter")?;
    // Seat 1 is active: seat 0 pays while it holds priority in seat 1's turn, then seat 1 tries to search.
    driver.cast("Tutor 1")?;
    for _ in 0..20 {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(0));
    driver.answer(Answer::Priority(PriorityAction::IgnoreEffect(arbiter)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let before = names(driver.state(), Zone::Hand(pid(1))).len();
    settle(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(1))).len(),
        before,
        "seat 1 found nothing"
    );
    Ok(())
}

#[rstest]
fn the_payment_lapses_at_the_next_turn(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = table(seats, 0, 1)?;
    let arbiter = driver.id_of("Leonin Arbiter")?;
    driver.answer(Answer::Priority(PriorityAction::IgnoreEffect(arbiter)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(ignorable(&driver), Vec::new());
    // Seat 1's next turn: the payment of the last one counts no more.
    let next = u32::try_from(2 + seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert_eq!(ignorable(&driver), [arbiter]);
    Ok(())
}
