//! Goal: an effect that moves a targeted spell takes the card off the stack into the hand, graveyard or library of its
//! owner, so the spell never resolves; a permanent target moves as before.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, instant, one_target};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// An instant with no cost: "Put target spell into its owner's {destination}."
fn recall(destination: Destination) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Recall")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![Pred::Spell])))],
                effects: vec![Effect::Move {
                    what: Ref::Target(0),
                    to: destination,
                    bind: None,
                }],
            }),
    )
}

#[rstest]
#[case::hand(Destination::Hand, "Hand")]
#[case::graveyard(Destination::Graveyard, "Graveyard")]
fn a_targeted_spell_leaves_the_stack_unresolved(
    #[case] destination: Destination,
    #[case] place: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [instant("Bolt", 0, &[])])
            .hand(1, [recall(destination)]),
    )?;
    driver.cast("Bolt")?;
    driver.pass()?;
    driver.begin_cast("Recall")?;
    let id = driver
        .state()
        .stack()
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Bolt")
        .unwrap();
    let bolt = Target::Object(driver.state().object(id).unwrap().reference());
    driver.choose_targets(&[bolt])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    while !driver.state().stack().is_empty()
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    let zone = match place {
        "Hand" => Zone::Hand(pid(0)),
        _ => Zone::Graveyard(pid(0)),
    };
    assert!(driver.state().zone(zone).contains(&id));
    Ok(())
}
