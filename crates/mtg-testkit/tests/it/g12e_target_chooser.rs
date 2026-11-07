//! Goal: "target creature of their choice" plays whole at tables of two, three and four seats: the player the target
//! names (an opponent of the controller, the player whose upkeep it is) is asked for the target, cannot back out of
//! the controller's ability, may pick only what the ability allows, and the ability then does its work on that target.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::settle;
use crate::common::{name_of, pid};

/// An artifact with one rule, from its JSON.
fn artifact(name: &str, rule: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(rule).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(rules),
    )
}

/// "{T}: Destroy target creature of an opponent's choice."
fn executioner() -> Arc<CardDefinition> {
    artifact(
        "Executioner",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"ChosenBy":{"who":"Opponents","what":{"Object":[{"Type":"Creature"}]}}}}]}}"#,
    )
}

/// "At the beginning of each player's upkeep, destroy target creature that player controls of their choice."
fn abyss() -> Arc<CardDefinition> {
    artifact(
        "Abyss",
        r#"{"Triggered":{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"ChosenBy":{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"ActivePlayer"}]},"who":"Triggering"}}}],"when":{"StepBegins":{"step":"Upkeep"}}}}"#,
    )
}

/// The pending target decision, with the names of the targets it offers.
fn target_decision(driver: &Driver) -> (Decision, Vec<String>) {
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!(
            "a target decision was expected, not {}",
            decision.kind.name()
        );
    };
    let names = options
        .legal
        .iter()
        .map(|target| match target {
            Target::Object(object) => name_of(driver.state(), object.id),
            Target::Player(player) => format!("player {}", player.get()),
        })
        .collect();
    (decision, names)
}

#[rstest]
fn an_opponent_chooses_the_target_of_the_controllers_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [ready(executioner()), ready(free_creature("Bear", 2, 2))],
            )
            .battlefield(1, [ready(free_creature("Ogre", 3, 3))]),
    )?;
    driver.begin_activation("Executioner")?;
    let (decision, mut legal) = target_decision(&driver);
    legal.sort();
    assert_eq!(
        decision.player,
        pid(1),
        "the first opponent in turn order chooses"
    );
    assert_eq!(legal, ["Bear", "Ogre"]);
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        unreachable!()
    };
    assert!(
        !options.can_cancel,
        "the opponent cannot back out of the activation"
    );
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    settle(&mut driver)?;
    assert!(
        driver.id_of("Bear").is_err(),
        "the Bear the opponent chose is destroyed"
    );
    assert!(driver.id_of("Ogre").is_ok());
    Ok(())
}

#[rstest]
fn the_player_whose_upkeep_it_is_chooses_among_their_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main2)
            .battlefield(0, [ready(abyss()), ready(free_creature("Bear", 2, 2))])
            .battlefield(
                1,
                [
                    ready(free_creature("Ogre", 3, 3)),
                    ready(free_creature("Elk", 1, 1)),
                ],
            ),
    )?;
    for _ in 0..200 {
        if driver.state().turn().active() == pid(1)
            && matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_))
        {
            break;
        }
        driver.answer_passively()?;
    }
    let (decision, mut legal) = target_decision(&driver);
    legal.sort();
    assert_eq!(decision.player, pid(1));
    assert_eq!(legal, ["Elk", "Ogre"]);
    let elk = driver.target_named("Elk")?;
    driver.choose_targets(&[elk])?;
    settle(&mut driver)?;
    assert!(driver.id_of("Elk").is_err());
    assert!(driver.id_of("Ogre").is_ok() && driver.id_of("Bear").is_ok());
    Ok(())
}
