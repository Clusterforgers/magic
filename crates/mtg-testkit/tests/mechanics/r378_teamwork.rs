//! Goal: "whenever you cast a spell using teamwork" plays at tables of two to four seats: Virtual Assistant makes a Robot Hero only when the teamwork cost was paid, never for a spell cast without it and never for an opponent's spell; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for {1} with teamwork 2 that gains 2 life.
fn huddle() -> Arc<CardDefinition> {
    let effects = r#"{"Spell":{"effects":[{"GainLife":{"amount":2}}]}}"#;
    Arc::new(
        CardDefinition::new("Huddle")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(Ability::Keyword(Keyword::Macro {
                name: "teamwork".to_owned(),
                amount: 2,
            }))
            .with_rules(serde_json::from_str(effects).unwrap()),
    )
}

fn robots(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.controller() == pid(seat) && object.card().name == "Robot Hero"
        })
        .count()
}

#[rstest]
#[case::using_teamwork(0, true, 1)]
#[case::declined(0, false, 0)]
#[case::an_opponent_casts_it(1, true, 0)]
fn the_assistant_answers_a_spell_cast_using_teamwork(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
    #[case] pays: bool,
    #[case] made: usize,
) -> Result<(), ScriptError> {
    let assistant = owner;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(free_creature("Mate", 2, 2)), ready(forest())])
        .hand(0, [huddle()]);
    scenario = if assistant == 0 {
        scenario.battlefield(
            0,
            [
                ready(free_creature("Mate", 2, 2)),
                ready(forest()),
                ready(card("Virtual Assistant").unwrap()),
            ],
        )
    } else {
        scenario.battlefield(1, [ready(card("Virtual Assistant").unwrap())])
    };
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Huddle")?;
    if let DecisionKind::ChooseCosts(_) = &driver.decision()?.kind {
        let chosen = if pays { vec![0] } else { vec![] };
        driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
    }
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let mate: Vec<_> = options
            .from
            .iter()
            .copied()
            .filter(|id| driver.state().object(*id).unwrap().card().name == "Mate")
            .collect();
        driver.answer(Answer::ChooseCards(CardsChoice(mate)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(robots(&driver, assistant), made);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
