//! Goal: a prevention shield divided among targets (Remedy, "prevent the next 5 damage that would be dealt this turn to any number of targets, divided as you choose") prevents each target's own share and no more, a player it did not target is dealt all the damage, and a target given the whole amount keeps it whole; Serra's Hymn divides as many as its verse counters and Pollen Remedy six when kicked and three when not, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::life;
use common::spell;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::Answer;
use mtg_engine::choose::DivideChoice;
use mtg_engine::choose::{CostsChoice, TargetChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Seat 0 casts Remedy on the players of `shares` with those shares, then seat 1 casts a spell that deals 4 damage to
/// each player.
fn remedy_then_blast(seats: usize, shares: &[(usize, u32)]) -> Result<Driver, ScriptError> {
    let blast = spell(
        "Blast",
        CardType::Instant,
        "[]",
        r#"[{"DealDamage":{"to":"EachPlayer","amount":4}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Remedy"])
            .hand(1, [blast])
            .battlefield(0, [ready(plains()), ready(plains()), ready(plains())]),
    )?;
    driver.begin_cast("Remedy")?;
    let targets: Vec<_> = shares
        .iter()
        .map(|(seat, _)| driver.target_player(*seat))
        .collect();
    driver.choose_targets(&targets)?;
    if shares.len() > 1 {
        driver.answer(Answer::DivideAmount(DivideChoice::Division(
            targets
                .iter()
                .zip(shares)
                .map(|(target, (_, share))| (*target, *share))
                .collect(),
        )))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for _ in 0..16 {
        if driver.decision()?.player == common::pid(1) {
            break;
        }
        driver.pass()?;
    }
    driver.cast("Blast")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
#[case::split_three_and_two(&[(1, 3), (0, 2)], &[(0, 18), (1, 19)])]
#[case::split_four_and_one(&[(1, 4), (0, 1)], &[(0, 17), (1, 20)])]
#[case::one_target_keeps_all_five(&[(1, 5)], &[(0, 16), (1, 20)])]
fn each_target_is_spared_its_own_share(
    #[case] shares: &[(usize, u32)],
    #[case] lives: &[(usize, i32)],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = remedy_then_blast(seats, shares)?;
    for (seat, expected) in lives {
        assert_eq!(life(&driver, *seat), *expected, "seat {seat}");
    }
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), 16, "seat {seat} was not a target");
    }
    Ok(())
}

/// Seat 0 sacrifices Serra's Hymn with `verses` verse counters, dividing them among seats 1 and 0 as `shares`, then seat 1
/// deals 4 damage to each player.
#[rstest]
#[case::three_verses_two_and_one(3, &[(1, 2), (0, 1)], &[(0, 17), (1, 18)])]
#[case::one_verse_on_one_target(1, &[(1, 1)], &[(0, 16), (1, 17)])]
fn serras_hymn_divides_its_verse_counters(
    #[case] verses: u32,
    #[case] shares: &[(usize, u32)],
    #[case] lives: &[(usize, i32)],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blast = spell(
        "Blast",
        CardType::Instant,
        "[]",
        r#"[{"DealDamage":{"to":"EachPlayer","amount":4}}]"#,
    );
    let hymn = with_counters(
        ready(card("Serra's Hymn").unwrap()),
        CounterKind::Other("verse".to_owned()),
        verses,
    );
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield(0, [hymn]).hand(1, [blast]))?;
    driver.begin_activation("Serra's Hymn")?;
    let targets: Vec<_> = shares
        .iter()
        .map(|(seat, _)| driver.target_player(*seat))
        .collect();
    driver.choose_targets(&targets)?;
    if shares.len() > 1 {
        driver.answer(Answer::DivideAmount(DivideChoice::Division(
            targets
                .iter()
                .zip(shares)
                .map(|(target, (_, share))| (*target, *share))
                .collect(),
        )))?;
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for _ in 0..16 {
        if driver.decision()?.player == common::pid(1) {
            break;
        }
        driver.pass()?;
    }
    driver.cast("Blast")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for (seat, expected) in lives {
        assert_eq!(life(&driver, *seat), *expected, "seat {seat}");
    }
    Ok(())
}

/// Seat 0 casts Pollen Remedy, kicked or not, on seats 1 and 0 (all but 1 of the total to seat 1), then seat 1 deals 4
/// damage to each player.
#[rstest]
fn pollen_remedy_divides_six_when_kicked_and_three_when_not(
    #[values(true, false)] kicked: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blast = spell(
        "Blast",
        CardType::Instant,
        "[]",
        r#"[{"DealDamage":{"to":"EachPlayer","amount":4}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Pollen Remedy"])
            .hand(1, [blast])
            .battlefield(0, [ready(plains()), ready(plains()), ready(plains())]),
    )?;
    let (foe, me) = (driver.target_player(1), driver.target_player(0));
    driver.begin_cast("Pollen Remedy")?;
    let mut total = 0;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseCosts(_) => {
                Answer::ChooseCosts(CostsChoice::Costs(if kicked { vec![0] } else { vec![] }))
            }
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![foe, me]))
            }
            DecisionKind::DivideAmount(options) => {
                total = options.total;
                Answer::DivideAmount(DivideChoice::Division(vec![(foe, total - 1), (me, 1)]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(total, if kicked { 6 } else { 3 });
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for _ in 0..16 {
        if driver.decision()?.player == common::pid(1) {
            break;
        }
        driver.pass()?;
    }
    driver.cast("Blast")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver, 0), 17);
    let spared = i32::try_from(total).unwrap() - 1;
    assert_eq!(life(&driver, 1), 16 + spared.min(4));
    Ok(())
}
