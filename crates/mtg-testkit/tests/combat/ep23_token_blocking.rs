//! Goal: a token created "that's blocking that creature" by a block trigger (Brimaz) blocks the attacker its source blocked, beside its source, one token for the one trigger; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const CAT: &str = r#"{"Custom":{"abilities":[{"Keyword":"Vigilance"}],"colors":["White"],"name":"Cat Soldier","power":1,"subtypes":["Cat","Soldier"],"toughness":1,"types":["Creature"]}}"#;

/// A 3/4 with "Whenever ~ blocks a creature, create a 1/1 Cat Soldier token that's blocking that creature."
fn king() -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"effects":[{{"CreateToken":{{"blocking":"Triggering","token":{CAT}}}}}],"when":{{"BecomesBlocked":{{"by":["This"],"what":[{{"Type":"Creature"}}]}}}}}}}}"#
    );
    Arc::new(
        (*free_creature("King", 3, 4))
            .clone()
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// The names of the creatures blocking the attacker named `attacker`.
fn blockers_of(driver: &Driver, attacker: &str) -> Vec<String> {
    let state = driver.state();
    state
        .combat()
        .attackers()
        .iter()
        .filter(|attack| state.object(attack.object()).unwrap().card().name == attacker)
        .flat_map(|attack| attack.blockers().iter())
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

/// Seat 0 attacks seat 1 with an Ogre; the King blocks it; the game goes on to the end of combat damage.
fn fight(seats: usize) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(free_creature("Ogre", 5, 5))])
        .battlefield(1, [ready(king())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1)])?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) if decision.player.index() == 1 => {
                driver.block(&[("King", "Ogre")])?;
            }
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::DeclareBlockers
                    && driver.state().stack().is_empty()
                    && blockers_of(&driver, "Ogre").len() > 1 =>
            {
                return Ok(driver);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the token never blocked");
}

#[rstest]
fn ep23_the_token_blocks_the_creature_its_source_blocked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = fight(seats)?;
    assert_eq!(blockers_of(&driver, "Ogre"), ["King", "Cat Soldier"]);
    let cats = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Cat Soldier")
        .count();
    assert_eq!(cats, 1, "one trigger, one token");
    Ok(())
}
