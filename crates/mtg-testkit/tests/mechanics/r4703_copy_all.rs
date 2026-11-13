//! Goal: "Whenever you cast an Eldrazi spell, you may pay {C}{C}. If you do, copy all spells you control, then copy all
//! other activated and triggered abilities you control" (Ulalek, Fused Atrocity) copies every spell of its controller on
//! the stack, the Eldrazi spell included, and nothing when the cost is not paid, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free instant: 1 damage to each opponent.
fn spark() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"DealDamage":{"to":"Opponents","amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Spark")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// A free 1/1 Eldrazi creature with flash.
fn spawn() -> Arc<CardDefinition> {
    Arc::new(
        (*free_creature("Spawn", 1, 1))
            .clone()
            .with_subtype("Eldrazi")
            .with_rules(Ability::Keyword(Keyword::Flash)),
    )
}

#[rstest]
fn the_spells_on_the_stack_are_copied_when_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![
                ready(card("Ulalek, Fused Atrocity").unwrap()),
                ready(wastes()),
                ready(wastes()),
            ],
        )
        .hand(0, vec![spark(), spawn()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spark")?;
    driver.cast("Spawn")?;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        match driver.decision()?.kind.name() {
            "yes-no" => {
                driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
            }
            "pay-mana" if driver.decision()?.player == pid(0) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let copies = usize::from(pays);
    let spawns = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Spawn")
        .count();
    assert_eq!(spawns, 1 + copies);
    let damage = i32::try_from(1 + copies).unwrap();
    for seat in 1..seats {
        assert_eq!(
            driver.state().player(pid(seat)).unwrap().life(),
            20 - damage
        );
    }
    Ok(())
}
