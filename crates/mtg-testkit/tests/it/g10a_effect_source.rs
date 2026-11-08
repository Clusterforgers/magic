//! Goal: a delayed trigger an activated ability sets up names that ability's source as `This` (Forge's `EffectSource`): "When that creature leaves the battlefield this turn, sacrifice ~" and "When ~ leaves the battlefield this turn, sacrifice that creature" each fire on the right departure and sacrifice the right permanent, and the log replays to the same state; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, spell};

/// A Mount with Phantasmal Mount's ability as the translator writes it: `{T}`: target creature you control gets
/// +1/+1; when that creature leaves this turn, sacrifice the Mount; when the Mount leaves this turn, sacrifice it.
fn mount() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[
            {"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}},
            {"DelayedTrigger":{"this_turn":true,"when":{"ZoneChange":{"from":"Battlefield","what":[{"Is":{"Target":0}}]}},
                "effects":[{"Sacrifice":{"filter":[{"Is":"This"}],"who":{"ControllerOf":"This"}}}]}},
            {"DelayedTrigger":{"this_turn":true,"when":{"ZoneChange":{"from":"Battlefield","what":["This"]}},
                "effects":[{"Sacrifice":{"filter":[{"Is":{"Target":0}}],"who":{"ControllerOf":{"Target":0}}}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Mount")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

/// Passes priority until the stack is empty and nothing waits to go on it.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..16 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn the_delayed_trigger_sacrifices_the_partner_of_what_left(
    #[values(2, 3, 4)] seats: usize,
    #[values("Mount", "Squire", "Bystander")] smitten: &str,
) -> Result<(), ScriptError> {
    let smite = spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(mount()),
                    ready(free_creature("Squire", 1, 1)),
                    ready(free_creature("Bystander", 1, 1)),
                ],
            )
            .hand(0, [smite]),
    )?;
    driver.begin_ability("Mount", 0)?;
    let squire = driver.target_named("Squire")?;
    driver.choose_targets(&[squire])?;
    settle(&mut driver)?;
    let target = driver.target_named(smitten)?;
    driver.cast_at("Smite", &[target])?;
    settle(&mut driver)?;
    let mut left = names(driver.state(), Zone::Battlefield);
    left.sort();
    let expected: &[&str] = match smitten {
        "Bystander" => &["Mount", "Squire"],
        _ => &["Bystander"],
    };
    assert_eq!(left, expected);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
