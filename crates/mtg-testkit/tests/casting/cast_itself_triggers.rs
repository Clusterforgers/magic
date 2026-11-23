//! Goal: a trigger on the casting of its own card ("When you cast this spell, exile two target permanents.") works from the stack whatever zone its rule names (the compiler's `Stack`, a translation that names none or the battlefield), with or without "you", for a card cast from a hand, from the command zone and for an alternative cost, at 2 and 4 seats; with fewer legal targets than it needs it is removed and the spell still resolves; beside another cast trigger both go on the stack and resolve.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, pay};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);
const TITAN: &str = "Ceaseless Titan";

/// The zone and caster of the cast trigger as a rule may write them.
const COMPILED: &str = r#""from":"Stack","when":{"SpellCast":{"by":"You","what":["This"]}}"#;
const UNNAMED: &str = r#""when":{"SpellCast":{"what":["This"]}}"#;
const UNNAMED_YOU: &str = r#""when":{"SpellCast":{"by":"You","what":["This"]}}"#;
const BATTLEFIELD: &str = r#""from":"Battlefield","when":{"SpellCast":{"what":["This"]}}"#;

/// Where the titan is cast from, and for what.
#[derive(Clone, Copy, Debug)]
enum Way {
    Hand,
    Command,
    Evoke,
}

/// A legendary 10/10 for {2} with "When you cast this spell, exile two target permanents.", its trigger written as
/// `shape`, and evoke {1} for `Way::Evoke`.
fn titan(shape: &str, way: Way) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Triggered":{{{shape},"effects":[{{"Exile":{{"what":{{"Target":0}}}}}}],"targets":[{{"what":{{"Object":[]}},"bounds":{{"min":2,"max":2}}}}]}}}}"#
    );
    let trigger: Ability = serde_json::from_str(&json).unwrap();
    let mut card = CardDefinition::new(TITAN)
        .with_type(CardType::Creature)
        .with_supertype(Supertype::Legendary)
        .with_power_toughness(10, 10)
        .with_mana_cost(cost(2, &[]))
        .with_rules(trigger);
    if matches!(way, Way::Evoke) {
        let evoke: Ability = serde_json::from_str(r#"{"Keyword":{"Evoke":"{1}"}}"#).unwrap();
        card = card.with_rules(evoke);
    }
    Arc::new(card)
}

/// An enchantment whose controller gains 1 life whenever they cast a spell.
fn watcher() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(rules),
    )
}

/// The permanents of seat 1, in battlefield order.
fn theirs(driver: &Driver) -> Vec<Target> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.owner() == PlayerId(1))
        .map(|object| Target::Object(object.reference()))
        .collect()
}

fn exiled(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .objects()
        .filter(|object| object.zone() == Zone::Exile && object.owner() == PlayerId(1))
        .count()
}

/// Passes until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack never emptied");
}

/// A table of `seats` where seat 0 has `lands` Forests and `extra` on the battlefield and the titan in hand or in the
/// command zone, and seat 1 has `targets` Forests.
fn table(
    seats: usize,
    titan: Arc<CardDefinition>,
    way: Way,
    lands: usize,
    targets: usize,
    extra: Option<Arc<CardDefinition>>,
) -> Scenario {
    let mut mine: Vec<_> = (0..lands).map(|_| ready(forest())).collect();
    mine.extend(extra.map(ready));
    let scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, (0..targets).map(|_| ready(forest())).collect::<Vec<_>>());
    match way {
        Way::Command => scenario.format(Arc::new(Commander)).commanders(0, [titan]),
        Way::Hand | Way::Evoke => scenario.hand(0, [titan]),
    }
}

/// Casts the titan the way given.
fn cast(driver: &mut Driver, way: Way) -> Result<(), ScriptError> {
    match way {
        Way::Hand | Way::Command => {
            driver.cast(TITAN)?;
        }
        Way::Evoke => {
            driver.begin_cast(TITAN)?;
            choose_way(driver, "Evoke")?;
            pay(driver)?;
        }
    }
    Ok(())
}

#[rstest]
fn the_cast_trigger_of_a_spell_exiles_its_two_targets(
    #[values(2, 4)] seats: usize,
    #[values(COMPILED, UNNAMED, UNNAMED_YOU, BATTLEFIELD)] shape: &str,
    #[values(Way::Hand, Way::Command, Way::Evoke)] way: Way,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, titan(shape, way), way, 2, 3, None))?;
    cast(&mut driver, way)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!(
            "the trigger asks its targets: {}",
            driver.decision()?.kind.name()
        );
    };
    assert!(options.triggered);
    assert_eq!((options.min, options.max), (2, 2));
    let picked = theirs(&driver)[..2].to_vec();
    driver.choose_targets(&picked)?;
    settle(&mut driver)?;
    assert_eq!(exiled(&driver), 2);
    let on_battlefield = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == TITAN);
    assert_eq!(on_battlefield, !matches!(way, Way::Evoke));
    Ok(())
}

#[rstest]
#[case::none(0)]
#[case::one(1)]
fn with_fewer_legal_targets_than_it_needs_the_trigger_is_removed(
    #[values(COMPILED, UNNAMED)] shape: &str,
    #[case] targets: usize,
) -> Result<(), ScriptError> {
    // Seat 0's lands are permanents too: with none of them, `targets` Forests of seat 1 are all there is.
    let free = Arc::new(Arc::unwrap_or_clone(titan(shape, Way::Hand)).with_mana_cost(cost(0, &[])));
    let mut driver = Driver::from_scenario(table(2, free, Way::Hand, 0, targets, None))?;
    driver.cast(TITAN)?;
    assert_eq!(
        driver.state().stack().len(),
        1,
        "only the spell is on the stack"
    );
    settle(&mut driver)?;
    assert_eq!(exiled(&driver), 0);
    assert_eq!(theirs(&driver).len(), targets);
    Ok(())
}

#[rstest]
fn beside_another_cast_trigger_both_resolve(
    #[values(COMPILED, UNNAMED)] shape: &str,
    #[values(0, 1)] first: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        2,
        titan(shape, Way::Hand),
        Way::Hand,
        2,
        3,
        Some(watcher()),
    ))?;
    let life = driver.state().player(SEAT).unwrap().life();
    driver.cast(TITAN)?;
    assert_eq!(driver.trigger_options()?.triggers.len(), 2);
    driver.order_triggers(&[first, 1 - first])?;
    let picked = theirs(&driver)[1..].to_vec();
    driver.choose_targets(&picked)?;
    assert_eq!(
        driver.state().stack().len(),
        3,
        "the spell and both triggers"
    );
    settle(&mut driver)?;
    assert_eq!(exiled(&driver), 2);
    assert_eq!(driver.state().player(SEAT).unwrap().life(), life + 1);
    Ok(())
}
