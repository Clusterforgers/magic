//! Goal: a decision another seat makes plays on the normal clock with a default answer: an unless-cost any player may pay and a choice among several lists run whole with the other seat idle (the question is put to it and the passive answer is a no), with the seat paying or picking, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A sorcery for {1} whose one ability is `json`.
fn probe(json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Probe")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

/// What a seat does when it is asked: nothing (the clock runs out), or the answer it is given.
#[derive(Clone, Copy)]
enum Seat {
    Idle,
    Pays,
    Picks(usize),
}

/// Plays the stack out; the seat 1 questions are answered as `seat1` says, every other question passively.
fn resolve(driver: &mut Driver, seat1: Seat) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(asked);
        }
        let other = decision.player == pid(1);
        asked += usize::from(other);
        let answer = match (&decision.kind, seat1, other) {
            // A seat that cannot pay is asked the same question and can only say no.
            (DecisionKind::YesNo(options), Seat::Pays, true) => {
                Answer::YesNo(YesNoChoice(!options.only_no))
            }
            (DecisionKind::PayMana(_), Seat::Pays, true) => Answer::PayMana(PayChoice::Auto),
            (DecisionKind::ChooseOption(_), Seat::Picks(n), true) => {
                Answer::ChooseOption(OptionChoice(n))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn untapped_lands(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| {
            object.controller() == pid(seat) && object.permanent().is_some_and(|p| !p.tapped())
        })
        .count()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn two_seats(card: Arc<CardDefinition>, foe_lands: usize) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card])
            .battlefield(0, [ready(mountain())])
            .battlefield(1, (0..foe_lands).map(|_| ready(mountain()))),
    )
}

const ANY_PLAYER_PAYS: &str = r#"{"Spell":{"effects":[{"Pay":{"who":"EachPlayer","cost":"{2}","unpaid":[{"GainLife":{"amount":3}}]}}]}}"#;

/// "Unless any player pays {2}": the caster gains 3 life only when nobody pays; the other seat is asked, and a seat
/// that never answers (or cannot pay) leaves the effect to happen.
#[rstest]
#[case::the_other_seat_is_idle(Seat::Idle, 2, 3, 2)]
#[case::the_other_seat_cannot_pay(Seat::Pays, 1, 3, 1)]
#[case::the_other_seat_pays(Seat::Pays, 2, 0, 0)]
fn any_player_may_pay_and_an_idle_seat_does_not(
    #[case] seat1: Seat,
    #[case] foe_lands: usize,
    #[case] gained: i32,
    #[case] foe_untapped: usize,
) -> Result<(), ScriptError> {
    let mut driver = two_seats(probe(ANY_PLAYER_PAYS), foe_lands)?;
    let before = life(&driver, 0);
    driver.cast("Probe")?;
    let asked = resolve(&mut driver, seat1)?;
    assert!(asked >= 1, "the other seat was never asked");
    assert_eq!(life(&driver, 0), before + gained);
    assert_eq!(untapped_lands(&driver, 1), foe_untapped);
    replays(&driver);
    Ok(())
}

const THREE_WAYS: &str = r#"{"Spell":{"effects":[{"OneOf":{"who":"Opponents","options":[[{"GainLife":{"amount":1}}],[{"GainLife":{"amount":2}}],[{"GainLife":{"amount":3}}]]}}]}}"#;

/// A choice among three lists made by the other seat: an idle seat takes the first, and the seat can pick any.
#[rstest]
#[case::idle(Seat::Idle, 1)]
#[case::first(Seat::Picks(0), 1)]
#[case::second(Seat::Picks(1), 2)]
#[case::third(Seat::Picks(2), 3)]
fn the_other_seat_chooses_among_three_lists(
    #[case] seat1: Seat,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = two_seats(probe(THREE_WAYS), 0)?;
    let before = life(&driver, 0);
    driver.cast("Probe")?;
    let asked = resolve(&mut driver, seat1)?;
    assert!(asked >= 1, "the other seat was never asked");
    assert_eq!(life(&driver, 0), before + gained);
    replays(&driver);
    Ok(())
}
