//! Goal: pack e16a's facts of the turn's history play whole at two, three and four seats: "an artifact you've sacrificed
//! this turn" counts an artifact and not a creature, judged as it was on the battlefield; combat and noncombat damage to
//! an opponent are told apart; only the batch of a player's first discards of the turn triggers; a tap to attack is told
//! from another tap; and the first tap of a permanent in a turn triggers once however often it untaps and taps again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::macro_keywords::{life, play, table};

/// A permanent of the type with the ability `json`.
fn permanent(name: &str, kind: CardType, json: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(json).unwrap();
    let mut card = CardDefinition::new(name).with_type(kind).with_rules(rule);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

const SAC_SELF: &str =
    r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#;
const ASKS: &str = r#"{"Activated":{"condition":{"Compare":{"left":{"Tally":{"event":"Sacrificed","filter":[{"Type":"Artifact"}],"who":"You"}},"op":"Ge","right":1}},"cost":["Tap"],"effects":[{"GainLife":{"amount":3}}]}}"#;

/// Seat 0 with the asking artifact, an artifact and a creature that sacrifice themselves, at `seats` seats.
fn driver(seats: usize) -> Result<Driver, ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![
                    ready(permanent("Satchel", CardType::Artifact, ASKS)),
                    ready(permanent("Bauble", CardType::Artifact, SAC_SELF)),
                    ready(permanent("Bear", CardType::Creature, SAC_SELF)),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 5]),
    );
    Driver::from_scenario(scenario)
}

/// Activates the named permanent's ability and lets it and what it triggers resolve, with the passive answers.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    settle(driver)
}

/// Answers passively and passes until seat 0 has priority with an empty stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?;
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the activation did not settle");
}

#[rstest]
#[case::nothing(None, false)]
#[case::an_artifact(Some("Bauble"), true)]
#[case::a_creature(Some("Bear"), false)]
fn an_artifact_sacrificed_this_turn_opens_the_ability(
    #[values(2, 3, 4)] seats: usize,
    #[case] sacrificed: Option<&str>,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = driver(seats)?;
    if let Some(name) = sacrificed {
        activate(&mut driver, name)?;
    }
    assert_eq!(!driver.offered_abilities("Satchel")?.is_empty(), offered);
    Ok(())
}

/// "{T}: You gain 3 life. Activate only if an opponent was dealt damage of the kind this turn."
fn asks_dealt(name: &str, what: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Activated":{{"condition":{{"Compare":{{"left":{{"Players":{{"PlayersWhere":{{"test":{{"Compare":{{"left":{{"Tallied":{{"what":"{what}","who":"You"}}}},"op":"Ge","right":1}}}},"who":"Opponents"}}}}}},"op":"Ge","right":1}}}},"cost":["Tap"],"effects":[{{"GainLife":{{"amount":3}}}}]}}}}"#
    );
    permanent(name, CardType::Artifact, &rule)
}

#[rstest]
#[case::burn(false)]
#[case::an_attack(true)]
fn combat_and_noncombat_damage_to_an_opponent_are_told_apart(
    #[values(2, 3, 4)] seats: usize,
    #[case] combat: bool,
) -> Result<(), ScriptError> {
    let cards = vec![
        asks_dealt("Combat Ear", "CombatDamageTaken"),
        asks_dealt("Burn Ear", "NoncombatDamageTaken"),
        free_creature("Bear", 2, 2),
        mountain(),
    ];
    let scenario =
        table(seats, cards, &[free_creature("Wall", 0, 4)]).hand(0, vec![burn("Shock", 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    if combat {
        play(&mut driver, &["Bear"], &[], &|_, _| None, Step::Main2)?;
    } else {
        let opponent = driver.target_player(1);
        driver.cast_at("Shock", &[opponent])?;
        settle(&mut driver)?;
    }
    assert_eq!(!driver.offered_abilities("Combat Ear")?.is_empty(), combat);
    assert_eq!(!driver.offered_abilities("Burn Ear")?.is_empty(), !combat);
    Ok(())
}

/// A permanent with a trigger on `when` that gains 5 life, and `more` abilities.
fn watcher(name: &str, when: &str, more: &[&str]) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{when},"effects":[{{"GainLife":{{"amount":5}}}}]}}}}"#
    ))
    .unwrap();
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_rules(rule);
    for json in more {
        card = card.with_rules(serde_json::from_str::<Ability>(json).unwrap());
    }
    Arc::new(card)
}

#[rstest]
fn only_the_first_discards_of_the_turn_trigger(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] discards: usize,
) -> Result<(), ScriptError> {
    let looter =
        r#"{"Activated":{"cost":[{"Discard":{"count":1}}],"effects":[{"GainLife":{"amount":1}}]}}"#;
    let first = r#"{"Nth":{"event":{"Batch":{"Discarded":{"by":"You"}}},"nth":1}}"#;
    let scenario =
        table(seats, vec![watcher("Rielle", first, &[looter])], &[]).hand(0, vec![forest(); 3]);
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..discards {
        activate(&mut driver, "Rielle")?;
    }
    let gained = life(&driver, 0) - 20;
    assert_eq!(gained, 5 + i32::try_from(discards).unwrap());
    Ok(())
}

#[rstest]
#[case::not_attacking(
    r#"{"TappedNotAttacking":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    5,
    0
)]
#[case::any_tap(
    r#"{"Tapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    5,
    5
)]
fn a_tap_to_attack_is_told_from_another_tap(
    #[values(2, 3, 4)] seats: usize,
    #[case] when: &str,
    #[case] by_ability: i32,
    #[case] by_attack: i32,
) -> Result<(), ScriptError> {
    let tapper = r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#;
    let cards = vec![
        watcher("Watcher", when, &[]),
        permanent("Tapper", CardType::Creature, tapper),
        free_creature("Bear", 2, 2),
    ];
    let mut driver = Driver::from_scenario(table(seats, cards, &[]))?;
    activate(&mut driver, "Tapper")?;
    assert_eq!(life(&driver, 0), 20 + 1 + by_ability);
    play(&mut driver, &["Bear"], &[], &|_, _| None, Step::Main2)?;
    assert_eq!(life(&driver, 0), 20 + 1 + by_ability + by_attack);
    Ok(())
}

#[rstest]
fn the_first_tap_of_a_permanent_in_a_turn_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] taps: usize,
) -> Result<(), ScriptError> {
    let untapper = r#"{"Activated":{"cost":["Tap"],"effects":[{"Untap":{"what":"This"}}]}}"#;
    let first = r#"{"Nth":{"event":{"Tapped":{"what":["This"]}},"nth":1}}"#;
    let scenario = table(seats, vec![watcher("Spinner", first, &[untapper])], &[]);
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..taps {
        activate(&mut driver, "Spinner")?;
    }
    assert_eq!(life(&driver, 0), 25);
    Ok(())
}
