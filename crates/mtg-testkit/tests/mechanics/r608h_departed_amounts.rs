//! Goal: an amount over the objects a resolution sacrificed (`Is(Departed(..))`) reads each as it last existed (CR
//! 608.2h): Shadowgrange Archfiend gains the greatest sacrificed power, also when that creature is a commander that
//! went to the command zone or a token that ceased to exist; the total and the count read the same objects, and a
//! count limited to the battlefield finds none of them; at two
//! to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;
use crate::w7d_control_scenarios::w7d_oracle;

/// The live draft's enter trigger, gaining `amount` over the creatures it sacrificed.
fn r608h_trigger(amount: &str) -> Ability {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[
        {{"RepeatEach":{{"over":"Opponents","as":"opp","effects":[
          {{"Sacrifice":{{"who":{{"Bound":"opp"}},"filter":[{{"Type":"Creature"}},
            {{"GreatestAmong":{{"stat":"Power","group":[{{"Type":"Creature"}},{{"ControlledBy":{{"Bound":"opp"}}}}]}}}}],
            "as":"sac"}}}},
          {{"Bind":{{"what":{{"Bound":"sac"}},"as":"sacrificed","adds":true}}}}]}}}},
        {{"GainLife":{{"amount":{amount}}}}}]}}}}"#
    );
    serde_json::from_str(&json).unwrap()
}

/// The Archfiend as a free 6/4 with the draft's trigger gaining `amount`.
fn r608h_archfiend(amount: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shadowgrange Archfiend")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(6, 4)
            .with_rules(r608h_trigger(amount)),
    )
}

/// Answers every seat (the command zone chosen, mana paid) until the stack and the waiting triggers are empty.
fn r608h_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// What seat 1's greatest creature is when the Archfiend enters.
#[derive(Clone, Copy, Debug)]
enum Greatest {
    /// A 2/2 creature card with two +1/+1 counters, read with them.
    Card,
    /// Its 5/5 commander, which goes home to the command zone.
    Commander,
    /// A 7/7 token, which ceases to exist.
    Token,
}

/// Seat 1 sets up `greatest` on its turn, every later seat holds a 2/2 and a 1/1, then seat 0 casts `archfiend`.
/// Returns the driver and the life seat 0 had before the cast.
fn r608h_play(
    seats: usize,
    greatest: Greatest,
    archfiend: Arc<CardDefinition>,
) -> Result<(Driver, i32), ScriptError> {
    let maker = w7d_oracle(
        "Demon Summons",
        "{0}",
        "Sorcery",
        None,
        "Create a 7/7 black Demon creature token.",
    );
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(1, 1, Step::Main1)
        .commanders(1, [legend("Cmd", 0, &[], 5, 5)])
        .hand(1, [maker])
        .hand(0, [archfiend])
        .battlefield(1, [ready(free_creature("Small", 1, 1))]);
    if matches!(greatest, Greatest::Card) {
        scenario = scenario.battlefield(
            1,
            [
                ready(free_creature("Small", 1, 1)),
                with_counters(
                    ready(free_creature("Big", 2, 2)),
                    CounterKind::PlusOnePlusOne,
                    2,
                ),
            ],
        );
    }
    for seat in 2..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Mite", 1, 1)),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    match greatest {
        Greatest::Card => {}
        Greatest::Commander => {
            driver.cast("Cmd")?;
            r608h_settle(&mut driver)?;
        }
        Greatest::Token => {
            driver.begin_cast("Demon Summons")?;
            r608h_settle(&mut driver)?;
        }
    }
    driver.idle_until(u32::try_from(seats).unwrap(), Step::Main1)?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Shadowgrange Archfiend")?;
    r608h_settle(&mut driver)?;
    Ok((driver, before))
}

#[rstest]
#[case::a_card(Greatest::Card, 4)]
#[case::a_commander(Greatest::Commander, 5)]
#[case::a_token(Greatest::Token, 7)]
fn r608h_archfiend_gains_the_greatest_sacrificed_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] greatest: Greatest,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let archfiend = r608h_archfiend(
        r#"{"Greatest":{"of":[{"Is":{"Departed":{"Bound":"sacrificed"}}}],"stat":"Power"}}"#,
    );
    let (driver, before) = r608h_play(seats, greatest, archfiend)?;
    let state = driver.state();
    assert_eq!(state.player(pid(0)).unwrap().life(), before + gained);
    if matches!(greatest, Greatest::Commander) {
        let commander = state.commanders().next().unwrap();
        assert_eq!(
            state.object(commander).unwrap().zone(),
            Zone::Command(pid(1))
        );
    }
    Ok(())
}

#[rstest]
#[case::total_power(
    r#"{"Total":{"of":[{"Is":{"Departed":{"Bound":"sacrificed"}}}],"stat":"Power"}}"#,
    2
)]
#[case::count(r#"{"Count":[{"Is":{"Departed":{"Bound":"sacrificed"}}}]}"#, 1)]
#[case::total_power_of_the_bound(
    r#"{"Total":{"of":[{"Is":{"Bound":"sacrificed"}}],"stat":"Power"}}"#,
    2
)]
#[case::count_of_the_bound(r#"{"Count":[{"Is":{"Bound":"sacrificed"}}]}"#, 1)]
#[case::none_still_on_the_battlefield(
    r#"{"Count":[{"Is":{"Departed":{"Bound":"sacrificed"}}},{"In":"Battlefield"}]}"#,
    0
)]
fn r608h_sibling_amounts_read_what_was_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(Greatest::Card, Greatest::Commander, Greatest::Token)] greatest: Greatest,
    #[case] amount: &str,
    #[case] per_later_seat: i32,
) -> Result<(), ScriptError> {
    let (driver, before) = r608h_play(seats, greatest, r608h_archfiend(amount))?;
    let first = match (greatest, per_later_seat) {
        (_, 0) => 0,
        (_, 1) => 1,
        (Greatest::Card, _) => 4,
        (Greatest::Token, _) => 7,
        _ => 5,
    };
    let later = i32::try_from(seats - 2).unwrap() * per_later_seat;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        before + first + later
    );
    Ok(())
}
