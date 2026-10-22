//! Goal: myriad, squad and ravenous play whole at tables of two, three and four seats: myriad asks its controller once (never at a table of two, where no other opponent exists) and makes a tapped token copy attacking each opponent other than the defending player, which deals combat damage, triggers nothing and is exiled at the end of combat, with nothing made when the answer is no; squad asks how many times to pay its cost up to what the lands can pay and makes a token copy for each payment, none for the copies themselves; ravenous puts X +1/+1 counters on the creature as it enters (a creature of 0/0 with none dies) and draws a card only when X is 5 or more; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{name_of, pid};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log replays to the live state.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority until the stack is empty or a decision other than priority is pending.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 holds `hand` and controls `permanents`; every seat has a library to draw from.
fn table(seats: usize, hand: &[&str], permanents: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, permanents),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

fn lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The tokens made so far: who controls each, whether it entered tapped, and whom it entered attacking.
fn tokens(driver: &Driver) -> Vec<(usize, bool, Option<AttackTarget>)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) => {
                Some((token.controller.index(), token.tapped, token.attacking))
            }
            _ => None,
        })
        .collect()
}

/// Seat 0 attacks seat `defender` with the Patrol and answers the question of its trigger with `yes` when it is asked;
/// whether it was asked is returned, and the game stops with the trigger resolved.
fn attack(driver: &mut Driver, defender: usize, yes: bool) -> Result<bool, ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Wyrm's Crossing Patrol", defender)])?;
    let mut asked = false;
    for _ in 0..20 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                asked = true;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => break,
        }
    }
    Ok(asked)
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats_the_next_one(3, 1)]
#[case::three_seats_the_other(3, 2)]
#[case::four_seats_the_next_one(4, 1)]
#[case::four_seats_the_last_one(4, 3)]
fn myriad_makes_a_tapped_token_copy_attack_each_other_opponent_and_exiles_them_at_the_end_of_combat(
    #[case] seats: usize,
    #[case] defender: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &[],
        vec![ready(card("Wyrm's Crossing Patrol").unwrap())],
    ))?;
    let asked = attack(&mut driver, defender, true)?;
    assert_eq!(
        asked,
        seats > 2,
        "nothing is asked when no one else is left"
    );
    let others: Vec<usize> = (1..seats).filter(|seat| *seat != defender).collect();
    let made = tokens(&driver);
    assert_eq!(made.len(), others.len());
    for seat in &others {
        assert!(
            made.contains(&(0, true, Some(AttackTarget::Player(pid(*seat))))),
            "a tapped token of seat 0 attacks seat {seat}: {made:?}"
        );
    }
    assert_eq!(driver.state().combat().attackers().len(), 1 + others.len());
    let triggers = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered(_)))
        .count();
    assert_eq!(triggers, 1, "the copies were never declared as attackers");

    driver.idle_until(1, Step::Main2)?;
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 19, "seat {seat} took 1 damage");
    }
    assert_eq!(
        count(&driver, Zone::Battlefield, "Wyrm's Crossing Patrol"),
        1
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn myriad_makes_nothing_when_the_answer_is_no(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &[],
        vec![ready(card("Wyrm's Crossing Patrol").unwrap())],
    ))?;
    assert!(attack(&mut driver, 1, false)?);
    assert_eq!(tokens(&driver), []);
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(life(&driver, 1), 19);
    assert!((2..seats).all(|seat| life(&driver, seat) == 20));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::cannot_pay_it(4, None, 0)]
#[case::pays_none(5, Some(1), 0)]
#[case::pays_once(5, Some(1), 1)]
#[case::could_pay_twice_pays_none(7, Some(2), 0)]
#[case::pays_twice(7, Some(2), 2)]
#[case::pays_three_times(9, Some(3), 3)]
fn squad_makes_a_token_copy_for_each_time_its_cost_was_paid_and_the_copies_make_none(
    #[case] forests: usize,
    #[case] most: Option<u32>,
    #[case] paid: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, &["Galadhrim Brigade"], lands(forests, forest)))?;
    driver.begin_cast("Galadhrim Brigade")?;
    if let Some(most) = most {
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            panic!("the times to pay squad come first");
        };
        assert_eq!((options.min, options.max), (0, Some(most)));
        assert_eq!(options.text, "Choose how many times to pay Squad {1}{G}.");
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(paid)))?;
    }
    assert_eq!(driver.pay_options()?.cost.mana_value(), 3 + 2 * paid);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    // The spell resolves, then the trigger that makes the copies, then whatever they trigger.
    for _ in 0..6 {
        driver.pass()?;
        driver.pass()?;
    }
    let copies = paid as usize;
    assert_eq!(
        count(&driver, Zone::Battlefield, "Galadhrim Brigade"),
        1 + copies
    );
    assert_eq!(tokens(&driver).len(), copies);
    assert!(
        tokens(&driver)
            .iter()
            .all(|(controller, tapped, attacking)| *controller == 0
                && !tapped
                && attacking.is_none())
    );
    // Each Elf is made bigger by every other one.
    let state = driver.state();
    for id in state.zone(Zone::Battlefield) {
        if name_of(state, *id) == "Galadhrim Brigade" {
            let shown = mtg_engine::characteristics(state, *id).unwrap();
            assert_eq!(shown.power, Some(2 + i32::try_from(copies).unwrap()));
        }
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::nothing(0, false)]
#[case::one(1, false)]
#[case::four(4, false)]
#[case::five_draws(5, true)]
#[case::six_draws(6, true)]
fn ravenous_enters_with_x_counters_and_draws_only_for_five_or_more(
    #[case] x: u32,
    #[case] draws: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, &["Aberrant"], lands(x as usize + 2, forest)))?;
    driver.begin_cast("Aberrant")?;
    // With lands for nothing but the {1}{G}, X can only be 0 and is not asked.
    if x > 0 {
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            panic!("X comes first");
        };
        assert_eq!(options.max, Some(x));
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    driver.pass()?;
    settle(&mut driver)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(
        hand,
        usize::from(draws),
        "a card is drawn for X of 5 or more"
    );
    if x == 0 {
        assert_eq!(count(&driver, Zone::Battlefield, "Aberrant"), 0, "0/0 dies");
        assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Aberrant"), 1);
    } else {
        let id = driver.id_of("Aberrant")?;
        let permanent = driver.state().object(id).unwrap().permanent().unwrap();
        assert_eq!(permanent.counters_of(&CounterKind::PlusOnePlusOne), x);
    }
    assert_replays(&driver);
    Ok(())
}
