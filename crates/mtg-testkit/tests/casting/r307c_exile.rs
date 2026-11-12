//! Goal: Fire Lord Zuko puts a +1/+1 counter on each creature its controller controls when they cast a spell from exile
//! and when a permanent they control enters from exile (a flickered creature included), at tables of two, three and four
//! seats; the same spell cast from a hand and the creature it becomes put none; and the log replays to the state it was
//! played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, cost, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free creature card, 1/1.
fn r307c_free_creature() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Free Sprite")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(1, 1),
    )
}

/// Seat 0's own 2/2, named apart from the other seats' Bears.
fn r307c_ally() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ally Bear")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(2, &[]))
            .with_power_toughness(2, 2),
    )
}

/// A free instant: exile the top card of your library; you may play it this turn.
fn r307c_impulse() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"ExileTop":{"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfTurn"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Free Impulse")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

fn r307c_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

/// Passes priority until the stack is empty.
fn r307c_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r307c_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 has Fire Lord Zuko, its own 2/2 and a Plains, a hand of the impulse spell, a free creature and Cloudshift, and a
/// library of free creatures; seat 1 has a Bear and Cloudshift.
fn r307c_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Fire Lord Zuko").unwrap()),
                ready(r307c_ally()),
                ready(plains()),
            ],
        )
        .hand(
            0,
            [
                r307c_impulse(),
                r307c_free_creature(),
                card("Cloudshift").unwrap(),
            ],
        )
        .library(0, (0..4).map(|_| r307c_free_creature()))
        .battlefield(1, [ready(bear()), ready(plains())])
}

#[rstest]
fn r307c_zuko_counts_a_spell_cast_from_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r307c_table(seats))?;
    driver.cast("Free Sprite")?;
    r307c_settle(&mut driver)?;
    assert_eq!(
        r307c_counters(&driver, "Fire Lord Zuko"),
        0,
        "cast from a hand"
    );
    driver.cast("Free Impulse")?;
    r307c_settle(&mut driver)?;
    driver.cast("Free Sprite")?;
    r307c_settle(&mut driver)?;
    assert_eq!(r307c_counters(&driver, "Fire Lord Zuko"), 1);
    assert_eq!(r307c_counters(&driver, "Ally Bear"), 1);
    r307c_replays(&driver);
    Ok(())
}

#[rstest]
fn r307c_zuko_counts_a_permanent_entering_from_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r307c_table(seats))?;
    let bear = driver.target_named("Ally Bear")?;
    driver.cast_at("Cloudshift", &[bear])?;
    r307c_settle(&mut driver)?;
    assert_eq!(r307c_counters(&driver, "Fire Lord Zuko"), 1);
    assert_eq!(
        r307c_counters(&driver, "Ally Bear"),
        1,
        "the returned creature too"
    );
    r307c_replays(&driver);
    Ok(())
}
