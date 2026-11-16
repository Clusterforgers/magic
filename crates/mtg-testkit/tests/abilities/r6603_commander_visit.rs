//! Goal: Hellkite Courser puts a commander its controller owns from the command zone onto the battlefield with haste and
//! returns it to the command zone at the beginning of the next end step; a "no" moves nothing, and a commander that
//! left the battlefield before then is a new object the return does not find (CR 400.7).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Hellkite Courser as the corpus plays it, free to cast.
fn courser() -> Arc<CardDefinition> {
    let card = mtg_testkit::corpus::card("Hellkite Courser").unwrap();
    Arc::new((*card).clone().with_mana_cost(cost(0, &[])))
}

fn general() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("General")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_mana_cost(cost(6, &[])),
    )
}

/// Resolves everything, answering the trigger's "may" with `yes` and taking the first card offered.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) => {
                if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty()
                {
                    return Ok(());
                }
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from.iter().copied().take(1).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

fn where_is(driver: &Driver, name: &str) -> Option<Zone> {
    let state = driver.state();
    [
        Zone::Battlefield,
        Zone::Command(pid(0)),
        Zone::Graveyard(pid(0)),
    ]
    .into_iter()
    .find(|zone| {
        state
            .zone(*zone)
            .iter()
            .any(|id| common::name_of(state, *id) == name)
    })
}

#[rstest]
#[case::visits_and_returns(true, Zone::Battlefield)]
#[case::stays_home(false, Zone::Command(pid(0)))]
fn r6603_the_commander_visits_until_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[case] yes: bool,
    #[case] after_cast: Zone,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .format(Arc::new(Commander))
            .at(1, 0, Step::Main1)
            .commanders(0, [general()])
            .hand(0, [courser()]),
    )?;
    driver.cast("Hellkite Courser")?;
    settle(&mut driver, yes)?;
    assert_eq!(where_is(&driver, "General"), Some(after_cast));
    if yes {
        let general = driver.id_of("General")?;
        assert!(
            has_keyword(driver.state(), general, &Keyword::Haste),
            "it gains haste"
        );
    }
    // No attack: the General goes home at the end step all the same.
    while driver.state().turn().step() != Step::End {
        driver.answer_passively()?;
    }
    settle(&mut driver, yes)?;
    assert_eq!(where_is(&driver, "General"), Some(Zone::Command(pid(0))));
    assert_eq!(
        where_is(&driver, "Hellkite Courser"),
        Some(Zone::Battlefield)
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
