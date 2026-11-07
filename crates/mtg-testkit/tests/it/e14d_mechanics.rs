//! Goal: pack e14d's mechanics play whole at tables of two, three and four seats: recruit draws, discards and makes a Human Soldier only for a nonland discard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::{permanents, play, table};

/// A creature that recruits when it attacks.
fn recruiter() -> Arc<CardDefinition> {
    let effects =
        serde_json::to_string(&macros::action_effects("recruit", 0, &Ref::You).unwrap()).unwrap();
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Recruiter")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

#[rstest]
fn recruit_makes_a_soldier_only_for_a_nonland_discard(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] nonland: bool,
) -> Result<(), ScriptError> {
    let top = if nonland {
        burn("Shock", 2)
    } else {
        mountain()
    };
    let scenario =
        table(seats, vec![recruiter()], &[free_creature("Wall", 0, 4)]).library(0, vec![top]);
    let mut driver = Driver::from_scenario(scenario)?;
    play(
        &mut driver,
        &["Recruiter"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    let state = driver.state();
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 0);
    assert_eq!(state.zone(Zone::Graveyard(pid(0))).len(), 1);
    assert_eq!(permanents(&driver, 0), 1 + usize::from(nonland));
    if nonland {
        assert!(driver.id_of("Human Soldier").is_ok());
    }
    Ok(())
}
