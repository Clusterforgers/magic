//! Goal: what a spell gains as it is cast ("it gains flying") stays with the permanent the spell becomes as it resolves (CR 400.7, 608.3), at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent that gives every creature spell its controller casts flying for ever.
fn herald() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"GrantAbility":{"Keyword":"Flying"}}],"duration":"Forever"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Herald")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

fn flies(driver: &Driver, name: &str) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap())
        .into_iter()
        .any(|found| *found == Keyword::Flying)
}

#[rstest]
fn a_creature_spell_keeps_what_it_gained_as_it_becomes_a_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![free_creature("Bear", 2, 2)])
        .battlefield(0, vec![ready(herald())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 30]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Bear")?;
    for _ in 0..40 {
        if driver.state().resolving().is_none()
            && driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert!(flies(&driver, "Bear"));
    Ok(())
}
