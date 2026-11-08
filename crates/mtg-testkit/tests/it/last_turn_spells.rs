//! Goal: "no spells were cast last turn" and "a player cast two or more spells last turn" read the turn that just ended at the table, whoever took it, over a table of turn sequences at two to four seats: no spell is a zero, two spells by one player count, one spell each does not, and a turn's spells do not carry into the next one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A 1/1 with an upkeep trigger for each werewolf condition: one life when no spell was cast last turn, ten when a
/// player cast two or more.
fn wolf() -> Arc<CardDefinition> {
    let upkeep = |condition: &str, life: u32| -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep"}}}},"if":{{"Compare":{{"left":{{"CastLastTurn":"EachPlayer"}},{condition}}}}},"effects":[{{"GainLife":{{"who":"You","amount":{life}}}}}]}}}}"#
        ))
        .unwrap()
    };
    Arc::new(
        CardDefinition::new("Wolf")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(upkeep(r#""op":"Eq","right":0"#, 1))
            .with_rules(upkeep(r#""op":"Ge","right":2"#, 10)),
    )
}

/// Every seat has forests to cast Fog with, and seat 0 controls the wolf.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario
            .hand_named(seat, ["Fog"; 6])
            .library(seat, vec![forest(); 30])
            .battlefield(
                seat,
                (0..6)
                    .map(|_| ready(forest()))
                    .chain((seat == 0).then(|| ready(wolf()))),
            );
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::no_spell_at_all(&[0], 1)]
#[case::one_spell_is_neither(&[1], 0)]
#[case::two_spells_by_one_player(&[2], 10)]
#[case::three_spells_by_one_player(&[3], 10)]
#[case::each_turn_reads_only_the_one_before(&[0, 1, 2, 0], 1 + 10 + 1)]
#[case::spells_do_not_carry_over(&[2, 1, 1], 10)]
fn the_upkeep_reads_the_turn_that_ended(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: &[usize],
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    for (turn, spells) in casts.iter().enumerate() {
        driver.idle_until(u32::try_from(turn).unwrap() + 1, Step::Main1)?;
        for _ in 0..*spells {
            driver.cast_tapping("Fog", &["Forest"])?;
            for _ in 0..seats {
                driver.pass()?;
            }
        }
    }
    let after = u32::try_from(casts.len()).unwrap() + 1;
    driver.idle_until(after, Step::Draw)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 + gained, "{casts:?} at {seats} seats");
    Ok(())
}
