//! Goal: "You may cast this spell as though it had flash if you pay {N} more to cast it" lets a sorcery be cast at instant speed for its mana cost plus {N}, and at sorcery speed for its mana cost alone (CR 601.3b, 118.8).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const FLASH: &str = r#"{"AltCast":{"way":"Alternative","cost":"{2}{R}","flash":true}}"#;
const BURN: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":3}}]}}"#;

/// A sorcery for {R}: "You may cast Ember Rite as though it had flash if you pay {2} more to cast it. You gain 3 life."
fn ember_rite() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ember Rite")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(serde_json::from_str::<Ability>(FLASH).unwrap())
            .with_rules(serde_json::from_str::<Ability>(BURN).unwrap()),
    )
}

/// Seat 0 holds Ember Rite with `lands` Mountains in `step` of turn 1: how many Mountains are tapped after the cast, if it was cast.
fn cast_in(step: Step, lands: usize) -> Result<Option<usize>, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, (0..lands).map(|_| ready(mountain())))
            .hand(0, vec![ember_rite()])
            .at(1, 0, step),
    )?;
    if driver.begin_cast("Ember Rite").is_err() {
        return Ok(None);
    }
    // The ways are the mana cost alone, then the mana cost with the two more; the sorcery-speed one is the first, the
    // instant-speed one the last.
    if let mtg_engine::decision::DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
        let way = if step == Step::Main1 {
            0
        } else {
            options.options.len() - 1
        };
        driver.answer(mtg_engine::decision::Answer::ChooseOption(
            mtg_engine::choose::OptionChoice(way),
        ))?;
    }
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    let tapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        })
        .count();
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(Some(tapped))
}

#[rstest]
#[case::sorcery_speed_pays_the_mana_cost_alone(Step::Main1, 3, Some(1))]
#[case::instant_speed_pays_two_more(Step::Upkeep, 3, Some(3))]
#[case::instant_speed_without_the_two_more(Step::Upkeep, 2, None)]
fn the_extra_two_is_paid_only_for_the_cast_at_instant_speed(
    #[case] step: Step,
    #[case] lands: usize,
    #[case] tapped: Option<usize>,
) -> Result<(), ScriptError> {
    assert_eq!(cast_in(step, lands)?, tapped);
    Ok(())
}
