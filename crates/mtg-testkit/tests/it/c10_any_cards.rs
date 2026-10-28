//! Goal: "if you've cast a noncreature spell this turn" holds for the caster of a noncreature spell only, never for a creature spell, so H.E.R.B.I.E. surveils at the beginning of combat exactly then, at tables of two, three and four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, gain_life};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

/// A spell for `{0}` of the type: an instant gains 1 life, a creature is a 1/1.
fn c10_any_spell(name: &str, kind: CardType) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(cost(0, &[]));
    Arc::new(match kind {
        CardType::Creature => card.with_power_toughness(1, 1),
        _ => card.with_rules(Ability::Spell {
            targets: Vec::new(),
            effects: vec![gain_life(1)],
        }),
    })
}

#[rstest]
fn c10_any_herbie_surveils_after_a_noncreature_spell(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(CardType::Instant), Some(CardType::Creature))] cast: Option<CardType>,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("H.E.R.B.I.E., Lovable Robot").unwrap())])
        .hand(
            0,
            [
                c10_any_spell("Brew", CardType::Instant),
                c10_any_spell("Cub", CardType::Creature),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.hand(seat, [c10_any_spell("Brew", CardType::Instant)]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(kind) = cast {
        driver.cast(if kind == CardType::Instant {
            "Brew"
        } else {
            "Cub"
        })?;
    }
    driver.idle_until(1, Step::DeclareAttackers)?;
    let surveilled = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::Surveilled { .. }));
    assert_eq!(surveilled, cast == Some(CardType::Instant), "{cast:?}");
    replays(&driver, &setup);
    Ok(())
}
