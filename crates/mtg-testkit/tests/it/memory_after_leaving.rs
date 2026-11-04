//! Goal: what a permanent's effect exiled stays remembered after the permanent leaves, until its leaves-the-battlefield trigger has resolved, so that trigger returns those cards (Worldgorger Dragon); a permanent with no such trigger forgets them when it leaves, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{names, spell};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::replay;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature with "T: exile your other permanents" and, when `returns`, "when this leaves the battlefield, return the
/// cards it exiled to the battlefield".
fn dragon(returns: bool) -> Arc<CardDefinition> {
    let exile: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"effects":[{"Exile":{"what":{"Each":[{"ControlledBy":"You"},"Other"]}}}]}}"#,
    )
    .unwrap();
    let mut card = CardDefinition::new("Dragon")
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4)
        .with_rules(exile);
    if returns {
        let back: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"what":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"to":{"Battlefield":{}}}}]}}"#,
        )
        .unwrap();
        card = card.with_rules(back);
    }
    Arc::new(card)
}

fn on_battlefield(driver: &Driver) -> Vec<String> {
    let mut found = names(driver.state(), Zone::Battlefield);
    found.sort();
    found
}

/// Passes priority until the stack is empty and nothing waits to go on it.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..12 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority && driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::the_trigger_returns_them(true, &["Alpha", "Beta"])]
#[case::no_trigger_means_they_stay(false, &[])]
fn what_a_permanent_exiled_comes_back_when_its_leave_trigger_says_so(
    #[case] returns: bool,
    #[case] back: &[&str],
) -> Result<(), ScriptError> {
    let smite = spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(dragon(returns)),
                    ready(free_creature("Alpha", 1, 1)),
                    ready(free_creature("Beta", 2, 2)),
                ],
            )
            .hand(0, [smite]),
    )?;
    driver.begin_activation("Dragon")?;
    settle(&mut driver)?;
    assert_eq!(on_battlefield(&driver), ["Dragon"], "the others are exiled");

    let target = driver.target_named("Dragon")?;
    driver.cast_at("Smite", &[target])?;
    settle(&mut driver)?;

    assert_eq!(on_battlefield(&driver), back);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
