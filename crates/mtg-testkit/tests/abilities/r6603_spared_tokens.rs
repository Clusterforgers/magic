//! Goal: with The Master, Multiplied, triggered abilities its controller controls can't cause them to sacrifice or exile
//! creature tokens they control (`Prohibited::R6603ByOwnTriggers`), while a spell still can, and without The Master the
//! same trigger takes the token; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::cost;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const MAKE: &str = r#"[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}}]"#;
const SACRIFICE: &str = r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},"Token"]}}]"#;
const EXILE: &str =
    r#"[{"Exile":{"what":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]}}}]"#;

fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":{effects}}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// A free creature whose enters trigger runs `effects`.
fn herald(effects: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Herald")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Remover {
    /// A creature's enters trigger.
    Trigger(&'static str),
    /// A sorcery.
    Spell(&'static str),
}

#[rstest]
#[case::a_trigger_cannot_sacrifice_it(true, Remover::Trigger(SACRIFICE), true)]
#[case::a_trigger_cannot_exile_it(true, Remover::Trigger(EXILE), true)]
#[case::a_spell_still_sacrifices_it(true, Remover::Spell(SACRIFICE), false)]
#[case::a_spell_still_exiles_it(true, Remover::Spell(EXILE), false)]
#[case::without_the_master_a_trigger_sacrifices_it(false, Remover::Trigger(SACRIFICE), false)]
#[case::without_the_master_a_trigger_exiles_it(false, Remover::Trigger(EXILE), false)]
fn r6603_own_triggers_spare_creature_tokens(
    #[values(2, 3, 4)] seats: usize,
    #[case] master: bool,
    #[case] remover: Remover,
    #[case] kept: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![sorcery("Muster", MAKE)];
    hand.push(match remover {
        Remover::Trigger(effects) => herald(effects),
        Remover::Spell(effects) => sorcery("Edict", effects),
    });
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, hand);
    if master {
        scenario = scenario.battlefield(0, [ready(card("The Master, Multiplied").unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Muster")?;
    settle(&mut driver)?;
    assert!(common::names(driver.state(), Zone::Battlefield).contains(&"Soldier".to_owned()));
    driver.cast(match remover {
        Remover::Trigger(_) => "Herald",
        Remover::Spell(_) => "Edict",
    })?;
    settle(&mut driver)?;
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield).contains(&"Soldier".to_owned()),
        kept
    );
    Ok(())
}
