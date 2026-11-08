//! Goal: the Plot keyword translates to the engine's plot, and a translated card is plotted for its plot cost and cast free from exile on a later turn; a shape the keyword does not have stays a gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

/// A creature for {4}{G}{G} with plot {1}{G}, as Forge prints it.
const PLOTTER: &str = "Name:Plotter\nManaCost:4 G G\nTypes:Creature Beast\nPT:3/3\nK:Plot:1 G\n";

#[rstest]
#[case::two_mana("1 G", "{1}{G}")]
#[case::generic("3", "{3}")]
fn a_plot_cost_is_the_keyword_cost(#[case] printed: &str, #[case] cost: &str) {
    let text = format!("Name:T\nTypes:Creature Elf\nK:Plot:{printed}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text), vec![json!({"Keyword": {"Plot": cost}})]);
}

fn plotter() -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Plotter")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(
            4,
            &[mtg_core::color::Color::Green, mtg_core::color::Color::Green],
        ))
        .with_power_toughness(3, 3);
    for value in abilities(PLOTTER) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn game(step: Step) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, step)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand(0, [plotter()])
            .battlefield(0, vec![ready(forest()), ready(forest())]),
    )
    .unwrap()
}

fn offered_to_plot(driver: &Driver) -> usize {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return 0;
    };
    options.plottable.len()
}

fn card_in(driver: &Driver, zone: Zone) -> mtg_core::ids::ObjectId {
    *driver.state().zone(zone).first().unwrap()
}

#[test]
fn a_translated_card_is_plotted_and_cast_free_on_a_later_turn() {
    let mut driver = game(Step::Main1);
    assert_eq!(offered_to_plot(&driver), 1);
    let id = card_in(&driver, Zone::Hand(PlayerId::from_index(0)));
    driver
        .answer(Answer::Priority(PriorityAction::Plot(id)))
        .unwrap();
    driver.pay_tapping(&["Forest", "Forest"], 0).unwrap();
    assert_eq!(driver.state().object(id).unwrap().plotted(), Some(1));
    driver.idle_until(3, Step::Main1).unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(id)))
        .unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    let way = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .unwrap();
    assert_eq!(way, Some(CastWay::Plotted));
}

#[test]
fn a_translated_card_is_not_plotted_outside_a_main_phase() {
    assert_eq!(offered_to_plot(&game(Step::Upkeep)), 0);
}
