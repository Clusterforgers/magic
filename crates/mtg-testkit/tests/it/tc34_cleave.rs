//! Goal: cleave is offered as an alternative cost beside the mana cost, a spell cast for it announces and resolves the text without the bracketed words (its targets widen), a spell cast for its mana cost keeps the printed words, at tables of two, three and four seats, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, ways};
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "Destroy target [black] creature." for {R}, with cleave {1}{R}.
fn cleaver() -> Arc<CardDefinition> {
    let spell = |selector: &str| {
        format!(
            r#"{{"Spell":{{"targets":[{{"what":{{"Object":{selector}}}}}],"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}}]}}}}"#
        )
    };
    let printed = spell(r#"[{"Type":"Creature"},{"Color":"Black"}]"#);
    let cleaved = spell(r#"[{"Type":"Creature"}]"#);
    Arc::new(
        CardDefinition::new("Cleaver")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(serde_json::from_str(&printed).unwrap())
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Keyword":{{"Cleave":{{"cost":"{{1}}{{R}}","spell":{cleaved}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [cleaver()])
        .battlefield(0, [ready(mountain()), ready(mountain())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(creature(&format!("Black {seat}"), 0, &[Color::Black], 2, 2)),
                ready(creature(&format!("Green {seat}"), 0, &[Color::Green], 2, 2)),
            ],
        );
    }
    scenario
}

#[rstest]
#[case::the_printed_words_take_only_a_black_creature("Cast", "Black", true)]
#[case::the_printed_words_refuse_a_green_creature("Cast", "Green", false)]
#[case::cleaved_it_takes_a_green_creature("Cleave", "Green", true)]
#[case::cleaved_it_still_takes_a_black_creature("Cleave", "Black", true)]
fn cleave_removes_the_bracketed_words(
    #[case] way: &str,
    #[case] victim: &str,
    #[case] legal: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.begin_cast("Cleaver")?;
    let offered = ways(&driver)?;
    assert_eq!(offered.len(), 2, "{offered:?}");
    assert!(offered[1].starts_with("Cleave"));
    choose_way(&mut driver, way)?;
    let name = format!("{victim} {}", seats - 1);
    let target = driver.target_named(&name)?;
    let chosen = driver.choose_targets(&[target]);
    assert_eq!(chosen.is_ok(), legal, "{chosen:?}");
    if !legal {
        return Ok(());
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(names(driver.state(), Zone::Graveyard(pid(seats - 1))).contains(&name));
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver.state().object(**id).is_some_and(|object| {
                object.owner() == pid(0)
                    && object
                        .permanent()
                        .is_some_and(mtg_engine::object::PermanentState::tapped)
            })
        })
        .count();
    assert_eq!(tapped, if way == "Cleave" { 2 } else { 1 });
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
