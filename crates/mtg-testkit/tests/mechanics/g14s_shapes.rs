//! Goal: three shapes the translator reads in round 14 pack s play at tables of two, three and four seats: a spell's shield
//! that prevents damage and gains life, a permanent that prevents all but one of the damage dealt to its controller and
//! an "attach all Equipment" trigger.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free sorcery with these effects.
fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Spell":{{"effects":{effects}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// Passes until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match driver.decision()?.kind {
            DecisionKind::Priority(_) => driver.pass()?,
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// What the translator writes for "prevent all damage this turn. You gain life equal to the damage prevented this way".
const PREVENT_AND_GAIN: &str = r#"{"Shield":{"by":{"PreventAnd":[{"GainLife":{"amount":"Triggering"}}]},
    "duration":"UntilEndOfTurn","when":{"Damage":{}}}}"#;

#[rstest]
fn a_shield_prevents_the_damage_and_gains_that_much_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let burn = r#"{"DealDamage":{"to":"Opponents","amount":3}}"#;
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(
        0,
        [sorcery(
            "Fog and Feast",
            &format!("[{PREVENT_AND_GAIN},{burn}]"),
        )],
    ))?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.cast("Fog and Feast")?;
    settle(&mut driver)?;
    for (seat, was) in before.iter().enumerate().skip(1) {
        assert_eq!(life(&driver, seat), *was, "seat {seat} is unhurt");
    }
    // Each opponent was meant to take 3: the shield prevented it and the caster gained 3 for each.
    let opponents = i32::try_from(seats - 1).unwrap();
    assert_eq!(life(&driver, 0), before[0] + 3 * opponents);
    Ok(())
}

#[rstest]
fn all_but_one_of_the_damage_to_the_controller_is_prevented(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let guard: CardDefinition = CardDefinition::new("Guard")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_rules(
            serde_json::from_str(
                r#"{"Replacement":{"by":{"PreventAllBut":1},"when":{"Damage":{"to":"You"}}}}"#,
            )
            .unwrap(),
        );
    let burn = r#"{"DealDamage":{"to":"Opponents","amount":5}}"#;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [sorcery("Burn", &format!("[{burn}]"))])
            .battlefield(1, [ready(Arc::new(guard))]),
    )?;
    let others: Vec<i32> = (2..seats).map(|seat| life(&driver, seat)).collect();
    let guarded = life(&driver, 1);
    driver.cast("Burn")?;
    settle(&mut driver)?;
    assert_eq!(
        life(&driver, 1),
        guarded - 1,
        "the guard's controller loses 1"
    );
    for (index, seat) in (2..seats).enumerate() {
        assert_eq!(
            life(&driver, seat),
            others[index] - 5,
            "seat {seat} takes all 5"
        );
    }
    Ok(())
}

#[rstest]
fn the_trigger_attaches_every_equipment_on_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attach = r#"{"Triggered":{"effects":[{"Attach":{"to":"This","what":{"Each":[{"Subtype":"Equipment"}]}}}],
        "when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
    let battlemaster = Arc::new(
        CardDefinition::new("Battlemaster")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(2, 2)
            .with_rules(serde_json::from_str(attach).unwrap()),
    );
    let equipment = |name: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Artifact)
                .with_subtype("Equipment"),
        )
    };
    let mut scenario = Scenario::new(seats).hand(0, [battlemaster]);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(equipment(&format!("Blade {seat}")))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Battlemaster")?;
    settle(&mut driver)?;
    let host = driver.id_of("Battlemaster")?;
    for seat in 0..seats {
        let blade = driver.id_of(&format!("Blade {seat}"))?;
        assert_eq!(driver.state().host_of(blade), Some(host), "Blade {seat}");
        assert_eq!(
            driver.state().object(blade).unwrap().controller(),
            pid(seat),
            "control does not change"
        );
    }
    Ok(())
}
