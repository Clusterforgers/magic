//! Goal: pack e14f's mechanics play whole at tables of two, three and four seats: a suspected creature that loses all abilities can block again (its "can't block" is an ability, CR 701.60c).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::{play, table};

/// A creature that suspects a target creature an opponent controls when it attacks.
fn accuser() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Suspect":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Accuser")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

/// An enchantment whose static ability takes every ability from the creatures its controller's opponents control.
fn silence() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":["LoseAbilities"]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Silence")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

/// The names of the objects.
fn names_of(driver: &Driver, ids: impl Iterator<Item = mtg_core::ids::ObjectId>) -> Vec<String> {
    ids.map(|id| driver.state().object(id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
fn a_suspected_creature_without_abilities_can_block(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] silenced: bool,
) -> Result<(), ScriptError> {
    let mut scenario = table(
        seats,
        vec![accuser()],
        &[free_creature("Wall", 0, 4), free_creature("Guard", 0, 4)],
    );
    if silenced {
        scenario = scenario.battlefield(0, vec![ready(silence())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let offered: RefCell<Option<Vec<String>>> = RefCell::new(None);
    let hook = |driver: &Driver, decision: &Decision| -> Option<Answer> {
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let wall = options.legal.iter().find(|target| {
                    matches!(target, Target::Object(object)
                        if driver.state().object(object.id).is_some_and(|o| o.card().name == "Wall"))
                })?;
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![*wall])))
            }
            DecisionKind::DeclareBlockers(options) if decision.player == pid(1) => {
                let ids = options.blockers.iter().map(|option| option.object);
                let mut names = names_of(driver, ids);
                names.sort();
                offered.replace(Some(names));
                None
            }
            _ => None,
        }
    };
    play(&mut driver, &["Accuser"], &[], &hook, Step::EndCombat)?;
    let expected: &[&str] = if silenced {
        &["Guard", "Wall"]
    } else {
        &["Guard"]
    };
    assert_eq!(
        offered.take(),
        Some(expected.iter().map(|s| (*s).to_owned()).collect())
    );
    let wall = driver.id_of("Wall")?;
    let held = driver.state().object(wall).unwrap().permanent().unwrap();
    assert!(held.suspected());
    assert_eq!(
        keywords_of(driver.state(), wall).contains(&&Keyword::Menace),
        !silenced
    );
    Ok(())
}
