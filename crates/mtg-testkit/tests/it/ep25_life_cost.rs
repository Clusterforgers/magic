//! Goal: a cost increase in life ("Spells your opponents cast that target ~ cost an additional 3 life to cast") is paid with the cast by a caster it applies to, once for each such permanent targeted, and never by its controller or for a spell that targets something else; a caster with less life than it asks cannot cast the spell, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A creature whose rule makes the spells of its controller's opponents that target it cost 3 life more.
fn dragon(name: &str) -> Arc<CardDefinition> {
    let rule = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":["This"]}],"change":"Increase","mana":"{0}","life":3}}"#;
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(5, 4)
            .with_rules(serde_json::from_str(rule).unwrap()),
    )
}

/// "{R}: Ping deals 1 damage to each of up to two target creatures."
fn ping() -> Arc<CardDefinition> {
    let rules = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":1,"max":2}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Ping")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// Seat `caster` casts Ping at the named creatures; the dragons belong to seat 1 and the bear to seat 0.
fn cast(seats: usize, caster: usize, at: &[&str], life: i32) -> Result<Driver, ScriptError> {
    let mut mine = vec![ready(bear())];
    let mut theirs = vec![ready(dragon("Dragon A")), ready(dragon("Dragon B"))];
    if caster == 0 { &mut mine } else { &mut theirs }.push(ready(mountain()));
    let scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .life(caster, life)
        .hand(caster, [ping()])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    let targets = at
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<Vec<_>, _>>()?;
    driver.cast_at("Ping", &targets)?;
    Ok(driver)
}

#[rstest]
#[case::one_dragon(0, &["Dragon A"], 17)]
#[case::two_dragons(0, &["Dragon A", "Dragon B"], 14)]
#[case::the_bear(0, &["Grizzly Bears"], 20)]
#[case::its_controller(1, &["Dragon A"], 20)]
fn the_life_is_paid_with_the_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] at: &[&str],
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let driver = cast(seats, caster, at, 20)?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(caster))),
        Vec::<String>::new()
    );
    driver.expect_life(caster, life)
}

#[rstest]
fn a_caster_short_of_the_life_cannot_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3)] life: i32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .life(0, life)
        .hand(0, [ping()])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(dragon("Dragon A"))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Dragon A")?;
    driver.begin_cast("Ping")?;
    driver.choose_targets(&[target])?;
    let paying = matches!(driver.decision()?.kind, DecisionKind::PayMana(_));
    assert_eq!(paying, life >= 3);
    if !paying {
        assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Ping"]);
    }
    driver.expect_life(0, life)
}
