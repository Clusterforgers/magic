//! Goal: "You may activate abilities of creatures you control as though those creatures had haste" lets a summoning-sick creature its controller controls use its `{T}` ability, never an opponent's creature, and gives no haste to attack; the activation replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::healer;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// An artifact with the rule for the creatures its controller controls.
fn elixir() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Elixir")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Rule {
                rule: GameRule::AbilitiesAsThoughHaste(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(Ref::You),
                ])),
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// Whether the deciding player is offered the healer's ability.
fn offered(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.abilities.iter().any(|a| {
            driver
                .state()
                .object(a.object)
                .is_some_and(|o| o.card().name == "Healer")
        }),
        _ => false,
    }
}

#[rstest]
#[case::no_rule(None, false)]
#[case::my_elixir(Some(0), true)]
#[case::an_opponent_s_elixir(Some(1), false)]
fn a_sick_creature_taps_for_its_ability_only_under_its_controller_s_rule(
    #[case] owner: Option<usize>,
    #[case] can: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![sick(healer("Healer", 0))];
        let mut theirs = Vec::new();
        match owner {
            Some(0) => mine.push(ready(elixir())),
            Some(_) => theirs.push(ready(elixir())),
            None => {}
        }
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(offered(&driver), can);
    if can {
        driver.begin_activation("Healer")?;
        run(&mut driver, 2)?;
        assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
        let healer = driver.id_of("Healer")?;
        assert!(
            mtg_engine::keywords::summoning_sick(driver.state(), healer),
            "the creature has no haste to attack"
        );
    }
    replays(&driver, &setup);
    Ok(())
}
