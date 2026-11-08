//! Goal: a token made "tapped and attacking that player" by an attack trigger attacks the player that attack went at, without a question, at tables of two, three and four seats: once for each player attacked when you attack ("whenever you attack a player"), and under the attacking player's control when another player attacks one of your opponents; an attack at the trigger's controller makes nothing, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::controller_of;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const GLIMMER: &str = r#"{"Custom":{"name":"Glimmer","colors":["White"],"types":["Creature"],"subtypes":["Glimmer"],"power":1,"toughness":1}}"#;

/// A free enchantment with one attack trigger that makes a Glimmer tapped and attacking the player attacked.
fn e16c_maker(when: &str, who: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{when},"effects":[{{"CreateToken":{{"who":{who},"token":{GLIMMER},"tapped":true,"attacking":true,"against":"Against"}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Maker")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// "Whenever you attack a player" (Soaring Lightbringer).
const YOU_ATTACK: &str =
    r#"{"GroupAttacksAt":{"at":"EachPlayer","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#;
/// "Whenever a player attacks one of your opponents", made by "that attacking player" (Combat Calligrapher).
const ONE_OF_YOURS: &str = r#"{"GroupAttacksAt":{"at":"Opponents","what":[{"Type":"Creature"}]}}"#;

/// The attacking Glimmers: their controller and the player each attacks.
fn glimmers(driver: &Driver) -> Vec<(PlayerId, Option<AttackTarget>)> {
    let state = driver.state();
    let mut found: Vec<(PlayerId, Option<AttackTarget>)> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.card().name == "Glimmer")
        })
        .map(|id| {
            let aim = state
                .combat()
                .attackers()
                .iter()
                .find(|attacker| attacker.object() == *id)
                .map(|attacker| attacker.target());
            (controller_of(state, *id).unwrap(), aim)
        })
        .collect();
    found.sort_by_key(|(controller, aim)| (controller.0, format!("{aim:?}")));
    found
}

/// Seat 0 attacks with one Bear at each seat of `at`; the Maker is on seat `owner`'s battlefield. Each Glimmer is
/// controlled by seat 0 and attacks one of the `expected` seats.
#[rstest]
#[case::you_attack_the_only_opponent(2, YOU_ATTACK, r#""You""#, 0, &[1], &[1])]
#[case::you_attack_two_players(3, YOU_ATTACK, r#""You""#, 0, &[1, 2], &[1, 2])]
#[case::two_bears_at_one_player_make_one(3, YOU_ATTACK, r#""You""#, 0, &[2, 2], &[2])]
#[case::four_seats(4, YOU_ATTACK, r#""You""#, 0, &[3, 1], &[1, 3])]
#[case::the_attacker_makes_it_at_your_opponent(3, ONE_OF_YOURS, r#"{"ControllerOf":"Triggering"}"#, 1, &[2], &[2])]
#[case::an_attack_at_you_makes_nothing(3, ONE_OF_YOURS, r#"{"ControllerOf":"Triggering"}"#, 1, &[1], &[])]
#[case::four_seats_each_opponent(4, ONE_OF_YOURS, r#"{"ControllerOf":"Triggering"}"#, 1, &[1, 2, 3], &[2, 3])]
fn e16c_tokens_attack_the_player_attacked(
    #[case] seats: usize,
    #[case] when: &str,
    #[case] who: &str,
    #[case] owner: usize,
    #[case] at: &[usize],
    #[case] expected: &[usize],
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut attackers: Vec<_> = at.iter().map(|_| ready(bear())).collect();
        let maker = ready(e16c_maker(when, who));
        let scenario = Scenario::new(seats).at(4, 0, Step::Main1);
        if owner == 0 {
            attackers.push(maker);
            scenario.battlefield(0, attackers)
        } else {
            scenario
                .battlefield(0, attackers)
                .battlefield(owner, vec![maker])
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("seat 0 declares attackers");
    };
    let bears: Vec<ObjectId> = options.attackers.iter().map(|a| a.object).collect();
    let declared: Vec<(ObjectId, AttackTarget)> = bears
        .iter()
        .zip(at)
        .map(|(bear, seat)| (*bear, AttackTarget::Player(pid(*seat))))
        .collect();
    assert_eq!(declared.len(), at.len());
    driver.answer(Answer::DeclareAttackers(declared))?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        assert!(
            !matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)),
            "no one is asked whom the token attacks"
        );
        driver.answer_passively()?;
    }
    let want: Vec<(PlayerId, Option<AttackTarget>)> = expected
        .iter()
        .map(|seat| (pid(0), Some(AttackTarget::Player(pid(*seat)))))
        .collect();
    assert_eq!(glimmers(&driver), want);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
