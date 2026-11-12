//! Goal: "Whenever the final chapter ability of a Saga you control triggers" and "... resolves" fire once for the chapter with the greatest number and never for an earlier chapter or for another player's Saga, at tables of two, three and four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::saga_cards::{lore_spell, saga};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "Whenever the final chapter ability of a Saga you control triggers, you gain 100 life." and "... resolves, you gain
/// 10 life."
fn bard() -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Bard").with_type(CardType::Artifact);
    for (resolves, life) in [(false, 100), (true, 10)] {
        let ability: Ability = serde_json::from_str(&format!(
            r#"{{"Triggered":{{"when":{{"FinalChapter":{{"what":[{{"Subtype":"Saga"}},{{"ControlledBy":"You"}}],"resolves":{resolves}}}}},"effects":[{{"GainLife":{{"amount":{life}}}}}]}}}}"#
        ))
        .unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Passes, pays and orders triggers as listed until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::OrderTriggers(options) => {
                let all: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&all)?;
            }
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// Seat `owner` holds the Bard; seat 0 casts a Saga whose chapters gain 1, 2, ... life and puts a lore counter on it with
/// a spell; the Saga ends after its last chapter.
#[rstest]
#[case::one_chapter_final_as_it_enters(1, 0, 20 + 1 + 100 + 10, 20)]
#[case::second_chapter_final_after_the_spell(2, 0, 20 + 1 + 100 + 2 + 10, 20)]
#[case::third_chapter_never_reached(3, 0, 20 + 1 + 2, 20)]
#[case::another_players_bard(1, 1, 20 + 1, 20)]
fn the_final_chapter_triggers_and_resolves_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] chapters: u32,
    #[case] owner: usize,
    #[case] seat_zero: i32,
    #[case] seat_one: i32,
) -> Result<(), ScriptError> {
    let table = || {
        let gains: Vec<(Vec<u32>, i32)> = (1..=chapters)
            .map(|n| (vec![n], i32::try_from(n).unwrap()))
            .collect();
        let chapters: Vec<(&[u32], i32)> = gains
            .iter()
            .map(|(numbers, life)| (numbers.as_slice(), *life))
            .collect();
        let mut mine = vec![ready(plains()), ready(plains())];
        if owner == 0 {
            mine.push(ready(bard()));
        }
        let scenario = Scenario::new(seats)
            .at(1, 0, Step::Main2)
            .battlefield(0, mine)
            .hand(0, [saga("Tale", &chapters), lore_spell("Lore", 1)]);
        if owner == 0 {
            scenario
        } else {
            scenario.battlefield(owner, [ready(bard())])
        }
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Tale")?;
    settle(&mut driver)?;
    if chapters >= 2 {
        let tale = driver.id_of("Tale")?;
        let target = Target::Object(driver.state().object(tale).unwrap().reference());
        driver.cast_at("Lore", &[target])?;
        settle(&mut driver)?;
    }
    assert_eq!(life(&driver, 0), seat_zero);
    assert_eq!(life(&driver, 1), seat_one);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}
