//! Goal: "Lands you control enter untapped" plays at tables of two to four seats: a land of its controller that enters
//! tapped by its own effect, or that an effect puts onto the battlefield tapped, enters untapped with no question asked,
//! a land of another player still enters tapped, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

const UNTAPPED: &str = r#"{"Replacement":{"by":"Untapped","when":{"Enters":{"what":[{"Type":"Land"},{"ControlledBy":"You"}]}}}}"#;

/// A land that enters tapped.
fn slow_land() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Slow Land")
            .with_type(CardType::Land)
            .with_rules(ability(
                r#"{"Replacement":{"by":"Tapped","when":{"Enters":{"what":["This"]}}}}"#,
            )),
    )
}

/// A plain land put onto the battlefield tapped from its owner's hand by the rig of seat `seat`.
fn putter() -> Arc<CardDefinition> {
    artifact_with(
        "Putter",
        vec![ability(
            r#"{"Activated":{"cost":["Tap"],"effects":[{"Move":{"what":{"Each":[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}]},"to":{"Battlefield":{"tapped":true}}}}]}}"#,
        )],
    )
}

fn guide() -> Arc<CardDefinition> {
    artifact_with("Guide", vec![ability(UNTAPPED)])
}

fn tapped_lands(driver: &Driver, seat: usize) -> Vec<bool> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Slow Land" && object.controller() == pid(seat))
        .map(|object| object.permanent().unwrap().tapped())
        .collect()
}

#[rstest]
fn lands_you_control_enter_untapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] guided: bool,
    #[values(true, false)] by_effect: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(putter())];
    if guided {
        mine.push(ready(guide()));
    }
    let scenario = Scenario::new(seats)
        .hand(0, vec![slow_land()])
        .battlefield(0, mine);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if by_effect {
        driver.begin_activation("Putter")?;
        for _ in 0..30 {
            let decision = driver.decision()?.clone();
            if matches!(decision.kind, DecisionKind::Priority(_))
                && driver.state().stack().is_empty()
            {
                break;
            }
            assert!(!matches!(decision.kind, DecisionKind::ChooseReplacement(_)));
            driver.answer(Driver::passive_answer(&decision))?;
        }
    } else {
        driver.play_land("Slow Land")?;
    }
    assert!(!matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseReplacement(_)
    ));
    assert_eq!(tapped_lands(&driver, 0), vec![!guided]);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// Puts every land card in every hand onto the battlefield tapped, each under its owner's control.
fn spreader() -> Arc<CardDefinition> {
    artifact_with(
        "Putter",
        vec![ability(
            r#"{"Activated":{"cost":["Tap"],"effects":[{"Move":{"what":{"Each":[{"Type":"Land"},{"In":"Hand"}]},"to":{"Battlefield":{"tapped":true}}}}]}}"#,
        )],
    )
}

#[rstest]
fn another_players_land_still_enters_tapped(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let theirs = seats - 1;
    let scenario = Scenario::new(seats)
        .hand(theirs, vec![slow_land()])
        .hand(0, vec![slow_land()])
        .battlefield(0, vec![ready(spreader()), ready(guide())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Putter")?;
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    assert_eq!(tapped_lands(&driver, 0), vec![false]);
    assert_eq!(tapped_lands(&driver, theirs), vec![true]);
    Ok(())
}
