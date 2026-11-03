//! Goal: `Modal.who` has the player the card names choose the modes (the first of them in turn order from the controller who is still in the game, the controller when none is), that player cannot back the cast out, the controller goes on with the payment, a seat that concedes while asked never holds the game up; `Let` fixes its amount when it resolves, so later changes to the game leave the name as it was, and the log replays to the state it was played to, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn spell(rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// `who` picks one of two modes: 3 life (0) or a card (1).
fn chosen_by(who: &str) -> Arc<CardDefinition> {
    spell(&format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"who":{who},"modes":[
            {{"effects":[{{"GainLife":{{"amount":3}}}}]}},{{"effects":[{{"Draw":{{}}}}]}}]}}}}]}}}}"#
    ))
}

/// Seat 0 holds `card` with Mountains to pay for it in the first main phase of turn 1.
fn table(seats: usize, card: Arc<CardDefinition>) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card])
        .battlefield(0, (0..3).map(|_| ready(mountain())))
        .library(0, (0..8).map(|_| mountain()))
        .at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| mountain()));
    }
    Driver::from_scenario(scenario)
}

fn asked_modes(driver: &Driver) -> (usize, bool) {
    let decision = driver.decision().unwrap();
    match &decision.kind {
        DecisionKind::ChooseMode(options) => (decision.player.index(), options.can_cancel),
        other => panic!("a choice of modes was expected, got {}", other.name()),
    }
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let copy = replay(driver.engine().setup(), driver.engine().events())?;
    assert_eq!(copy.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::an_opponent_in_a_pair(2, r#""Opponents""#, &[], 1)]
#[case::the_next_opponent_of_three(3, r#""Opponents""#, &[], 1)]
#[case::the_next_opponent_of_four(4, r#""Opponents""#, &[], 1)]
#[case::the_next_after_one_left(3, r#""Opponents""#, &[1], 2)]
#[case::the_third_after_two_left(4, r#""Opponents""#, &[1, 2], 3)]
#[case::the_controller_by_default(3, r#""You""#, &[], 0)]
#[case::the_controller_when_no_one_is_named(3, r#""Monarch""#, &[], 0)]
fn the_modes_are_chosen_by_the_player_named_and_the_controller_goes_on(
    #[case] seats: usize,
    #[case] who: &str,
    #[case] left: &[usize],
    #[case] chooser: usize,
    #[values(0, 1)] mode: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, chosen_by(who))?;
    for seat in left {
        driver.concede(pid(*seat))?;
    }
    driver.begin_cast("Spell")?;
    let (asked, can_cancel) = asked_modes(&driver);
    assert_eq!(asked, chooser);
    assert_eq!(can_cancel, chooser == 0, "only the caster may back out");
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let before = life(&driver);
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode])))?;
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the payment is the caster's"
    );
    resolve(&mut driver)?;
    let cards = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(
        (life(&driver) - before, cards),
        if mode == 0 { (3, hand - 1) } else { (0, hand) }
    );
    Ok(())
}

#[rstest]
fn a_seat_that_leaves_while_asked_does_not_hold_the_game_up(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, chosen_by(r#""Opponents""#))?;
    driver.begin_cast("Spell")?;
    assert_eq!(asked_modes(&driver).0, 1);
    driver.concede(pid(1))?;
    // The cast is taken back and the table is asked something of a seat still in the game.
    let decision = driver.decision()?;
    assert!(driver.state().is_alive(decision.player));
    assert!(!matches!(decision.kind, DecisionKind::ChooseMode(_)));
    driver.begin_cast("Spell")?;
    assert_eq!(asked_modes(&driver).0, 2, "the next opponent is asked");
    Ok(())
}

/// Lose `lose` life, then gain what `Let` kept (the life before) or, without it, the life now.
fn let_spell(keep: bool, lose: i32) -> Arc<CardDefinition> {
    let (set, read) = if keep {
        (
            r#"{"Let":{"as":"before","amount":{"Life":"You"}}},"#,
            r#"{"Bound":"before"}"#,
        )
    } else {
        ("", r#"{"Life":"You"}"#)
    };
    spell(&format!(
        r#"{{"Spell":{{"effects":[{set}{{"LoseLife":{{"amount":{lose}}}}},{{"GainLife":{{"amount":{read}}}}}]}}}}"#
    ))
}

#[rstest]
#[case::nothing_lost(0)]
#[case::some_lost(5)]
#[case::most_lost(19)]
fn let_keeps_the_amount_it_had_when_it_ran(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] keep: bool,
    #[case] lose: i32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, let_spell(keep, lose))?;
    let start = life(&driver);
    driver.begin_cast("Spell")?;
    resolve(&mut driver)?;
    let gained = if keep { start } else { start - lose };
    assert_eq!(life(&driver), start - lose + gained);
    Ok(())
}
