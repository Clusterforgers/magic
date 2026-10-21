//! Goal: the combat forecast and the block preview, which every seat shares, never move a number because of a card nobody may see: a replacement effect printed on a card in a hand or a library leaves the forecast and the preview as they are with a plain card there (the same value, so nothing of it shows), the same effect on the battlefield is applied, and the forecast names only creatures of the battlefield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::combat::to_blockers;
use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, PresetPermanent};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// An enchantment whose effect prevents all damage while it is in the zone it names.
fn ward(name: &str, from: &str) -> Arc<CardDefinition> {
    let rules =
        format!(r#"{{"Replacement":{{"when":{{"Damage":{{}}}},"by":"Prevent","from":"{from}"}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rules).expect("a replacement effect")),
    )
}

/// Where the card sits.
#[derive(Debug, Clone, Copy)]
enum Place {
    Hand,
    Library,
}

/// Player 0 attacks player 1 with a 3/3, who holds `card` in `place` and a 2/2 that does not block; the game waits
/// for the blockers.
fn facing_blocks(card: Arc<CardDefinition>, place: Place) -> Result<Driver, ScriptError> {
    let base = Scenario::new(2)
        .at(2, 0, mtg_engine::turn::Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("A", 3, 3))])
        .battlefield(1, [ready(free_creature("B", 2, 2))]);
    let scenario = match place {
        Place::Hand => base.hand(1, [card]),
        Place::Library => base.library(1, [card, island()]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    Ok(driver)
}

fn forecast_and_preview(driver: &Driver) -> (CombatForecast, CombatForecast) {
    let answer = Answer::DeclareBlockers(Vec::new());
    let previewed = driver
        .engine()
        .preview_forecast(pid(1), &answer)
        .expect("a forecast");
    (
        pending_combat_forecast(driver.state()).expect("damage is ahead"),
        previewed,
    )
}

#[rstest]
fn a_card_nobody_may_see_changes_no_number(
    #[values(Place::Hand, Place::Library)] place: Place,
    #[values("Hand", "Library")] from: &str,
) -> Result<(), ScriptError> {
    let plain = facing_blocks(
        Arc::new(CardDefinition::new("Plain").with_type(CardType::Enchantment)),
        place,
    )?;
    let hidden = facing_blocks(ward("Hidden Ward", from), place)?;
    assert_eq!(
        forecast_and_preview(&hidden),
        forecast_and_preview(&plain),
        "the cards in the {place:?} showed"
    );
    // The unblocked 3/3 is expected to hit for 3: nothing hidden took part.
    let (forecast, _) = forecast_and_preview(&hidden);
    assert_eq!(forecast.players[0].damage, 3);
    Ok(())
}

#[test]
fn the_same_effect_in_play_is_applied() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, mtg_engine::turn::Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", 3, 3))])
            .battlefield(
                1,
                [
                    ready(free_creature("B", 2, 2)),
                    PresetPermanent::new(ward("Open Ward", "Battlefield")),
                ],
            ),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let (forecast, previewed) = forecast_and_preview(&driver);
    assert_eq!(forecast.players[0].damage, 0);
    assert_eq!(previewed, forecast);
    Ok(())
}

#[test]
fn only_creatures_of_the_battlefield_are_named() -> Result<(), ScriptError> {
    let driver = facing_blocks(ward("Hidden Ward", "Hand"), Place::Hand)?;
    let (forecast, _) = forecast_and_preview(&driver);
    assert!(forecast.creatures.iter().all(|creature| {
        driver
            .state()
            .object(creature.object)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
    }));
    Ok(())
}
