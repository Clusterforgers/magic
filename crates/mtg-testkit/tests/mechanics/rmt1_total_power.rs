//! Goal: "any number of target creature cards with total power 3 or less" plays at tables of two to four seats: the target options offer only what fits by power (mana value plays no part), refuse a set whose powers add up to more, and the spell moves what was chosen; every game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::c4_control::replays;
use crate::common::{names, pid, spell};

/// "Return any number of target creature cards with total power 3 or less from your graveyard to the battlefield."
fn rmt1_reunion() -> Arc<CardDefinition> {
    spell(
        "Reunion",
        CardType::Sorcery,
        r#"[{"bounds":{"min":0,"max":null},"what":{"Together":{"rule":{"TotalPowerAtMost":3},"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}}]"#,
        r#"[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]"#,
    )
}

/// Seat 0 holds the spell; every seat's graveyard holds creature cards of power 1 to 4, each of mana value 5 - power,
/// so a total over mana values would offer the opposite cards.
fn rmt1_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [rmt1_reunion()]);
    for seat in 0..seats {
        scenario = scenario.graveyard(
            seat,
            (1..=4).map(|power: i32| {
                creature(
                    &format!("Card {seat}-{power}"),
                    u32::try_from(5 - power).unwrap(),
                    &[],
                    power,
                    1,
                )
            }),
        );
    }
    scenario
}

fn rmt1_in_graveyard(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .expect("the card is in that graveyard");
    Target::Object(state.object(id).unwrap().reference())
}

fn rmt1_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::two_that_fit(&[1, 2], true)]
#[case::one_that_fits_alone(&[3], true)]
#[case::none(&[], true)]
#[case::over_the_total(&[1, 3], false)]
fn rmt1_a_total_power_bounds_the_set(
    #[case] powers: &[i32],
    #[case] accepted: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rmt1_table(seats))?;
    driver.begin_cast("Reunion")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the spell asks for its targets");
    };
    let total = options.total.clone().expect("the options say the total");
    assert_eq!(total.most, 3);
    let four = rmt1_in_graveyard(&driver, 0, "Card 0-4");
    assert!(
        !options.legal.contains(&four),
        "the card of power 4 is over the total alone"
    );
    assert_eq!(options.legal.len(), 3);
    let picked: Vec<Target> = powers
        .iter()
        .map(|power| rmt1_in_graveyard(&driver, 0, &format!("Card 0-{power}")))
        .collect();
    let answered = driver.answer(Answer::ChooseTargets(TargetChoice::Targets(picked)));
    assert_eq!(answered.is_ok(), accepted);
    if !accepted {
        return Ok(());
    }
    rmt1_settle(&mut driver)?;
    let field = names(driver.state(), Zone::Battlefield);
    for power in powers {
        assert!(field.contains(&format!("Card 0-{power}")));
    }
    assert_eq!(
        field.iter().filter(|name| name.starts_with("Card")).count(),
        powers.len()
    );
    replays(&driver);
    Ok(())
}
