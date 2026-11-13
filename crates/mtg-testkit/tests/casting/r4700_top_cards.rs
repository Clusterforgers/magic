//! Goal: an alternative cost that exiles the top cards of a kind from the caster's graveyard (Spinning Darkness, "the top
//! three black cards of your graveyard") is offered only while the graveyard holds three black cards, takes exactly the
//! three topmost of them however other cards lie between, leaves the rest in place, and the spell then resolves; at two,
//! three and four seats, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const DARKNESS: &str = "Spinning Darkness";

/// A creature card of one color, named for its place in the graveyard.
fn r4700_card(name: &str, color: Color) -> Arc<CardDefinition> {
    creature(name, 1, &[color], 1, 1)
}

/// The graveyard from bottom to top: `B` for a black card, `R` for a red one.
fn r4700_graveyard(cards: &str) -> Vec<Arc<CardDefinition>> {
    cards
        .chars()
        .enumerate()
        .map(|(at, kind)| match kind {
            'B' => r4700_card(&format!("Black {at}"), Color::Black),
            _ => r4700_card(&format!("Red {at}"), Color::Red),
        })
        .collect()
}

/// The names of the cards in seat 0's graveyard, bottom to top.
fn r4700_names(driver: &Driver) -> Vec<String> {
    driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

#[rstest]
#[case::exactly_three(2, "BBB", &["Spinning Darkness"])]
#[case::the_top_three_of_four(3, "BBBB", &["Black 0", "Spinning Darkness"])]
#[case::others_between(4, "BRBRBB", &["Black 0", "Red 1", "Red 3", "Spinning Darkness"])]
#[case::red_on_top(2, "BBBBRR", &["Black 0", "Red 4", "Red 5", "Spinning Darkness"])]
fn r4700_the_top_three_black_cards_pay_for_the_spell(
    #[case] seats: usize,
    #[case] graveyard: &str,
    #[case] left: &[&str],
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(1, 0, mtg_engine::turn::Step::Main1)
            .hand(0, [card(DARKNESS).unwrap()])
            .graveyard(0, r4700_graveyard(graveyard))
            .battlefield(1, [ready(bear())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast(DARKNESS)?;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::ChooseOption(_) => {
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            DecisionKind::PayMana(options) => {
                assert_eq!(options.cost.to_string(), "{0}", "the mana cost is not paid");
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            other => panic!("nothing else is asked: {other:?}"),
        }
    }
    assert_eq!(r4700_names(&driver), left);
    assert_eq!(driver.state().zone(Zone::Exile).len(), 3);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 23);
    assert!(
        driver.id_of("Grizzly Bears").is_err(),
        "the creature is dealt 3 damage"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::two_black(2, "BB")]
#[case::two_black_and_red(3, "BRBR")]
#[case::empty(4, "")]
fn r4700_fewer_than_three_black_cards_leave_the_spell_unoffered(
    #[case] seats: usize,
    #[case] graveyard: &str,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, mtg_engine::turn::Step::Main1)
        .hand(0, [card(DARKNESS).unwrap()])
        .graveyard(0, r4700_graveyard(graveyard))
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.begin_cast(DARKNESS).is_err());
    Ok(())
}
