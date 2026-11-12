//! Goal: a spell that says "This spell can't be copied." gets no copy from an effect that copies every spell cast, while a
//! spell without the rule does, at tables of two to four seats; the copy of the other spell resolves and deals its damage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// "Whenever a player casts a spell, that player copies it."
fn twin() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Twin")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(
                    r#"{"Triggered":{"when":{"SpellCast":{"what":[]}},"effects":[{"CopySpell":{"what":"Triggering","who":"TriggeringPlayer"}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// "Shock deals 1 damage to target player.", with "This spell can't be copied." when `uncopyable`.
fn shock(uncopyable: bool) -> Arc<CardDefinition> {
    let card = spell(
        "Shock",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":1}}]"#,
    );
    if !uncopyable {
        return card;
    }
    let rule: Ability = serde_json::from_str(r#"{"Rule":{"rule":"CantBeCopied"}}"#).unwrap();
    Arc::new((*card).clone().with_rules(rule))
}

#[rstest]
fn a_spell_that_cant_be_copied_gets_no_copy(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] uncopyable: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![shock(uncopyable)])
        .battlefield(seats - 1, vec![ready(twin())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    driver.cast_at("Shock", &[Target::Player(pid(1))])?;
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    let lost = before - driver.state().player(pid(1)).unwrap().life();
    assert_eq!(
        lost,
        if uncopyable { 1 } else { 2 },
        "uncopyable {uncopyable}"
    );
    Ok(())
}
