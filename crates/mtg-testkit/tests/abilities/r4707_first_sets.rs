//! Goal: names first printed in an old expansion play at two to four seats: Golgothian Sylex makes every controller
//! sacrifice their nontoken permanents with a name first printed in Antiquities (itself included) and spares the rest;
//! City in a Bottle makes its other Arabian Nights permanents go as soon as they are there, and keeps players from
//! casting spells and playing lands with such names while other cards stay open.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A vanilla permanent or spell card of one type with the name `name`.
fn r4707_named(name: &str, kind: CardType) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        definition = definition
            .with_power_toughness(2, 2)
            .with_mana_cost(ManaCost::parse("{G}").unwrap());
    }
    Arc::new(definition)
}

fn r4707_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn r4707_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == name)
    })
}

#[rstest]
fn r4707_golgothian_sylex_sacrifices_the_antiquities_names(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let theirs = seats - 1;
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Golgothian Sylex").unwrap()),
                ready(forest()),
                ready(r4707_named("Urza's Mine", CardType::Land)),
                ready(r4707_named("Grizzly Cub", CardType::Creature)),
            ],
        )
        .battlefield(
            theirs,
            [
                ready(r4707_named("Mishra's Factory", CardType::Land)),
                ready(r4707_named("Su-Chi", CardType::Artifact)),
                ready(r4707_named("Juzám Djinn", CardType::Creature)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Golgothian Sylex")?;
    driver.pay_tapping(&["Forest"], 0)?;
    r4707_settle(&mut driver)?;
    for gone in [
        "Golgothian Sylex",
        "Urza's Mine",
        "Mishra's Factory",
        "Su-Chi",
    ] {
        assert!(!r4707_on_battlefield(&driver, gone), "{gone} is sacrificed");
    }
    for kept in ["Grizzly Cub", "Juzám Djinn", "Forest"] {
        assert!(r4707_on_battlefield(&driver, kept), "{kept} stays");
    }
    let graveyard = driver.state().zone(Zone::Graveyard(pid(theirs)));
    assert_eq!(graveyard.len(), 2, "each controller sacrificed their own");
    Ok(())
}

#[rstest]
fn r4707_city_in_a_bottle_clears_and_bars_the_arabian_nights_names(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("City in a Bottle").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(r4707_named("Juzám Djinn", CardType::Creature))])
        .hand(
            0,
            [
                r4707_named("Erhnam Djinn", CardType::Creature),
                r4707_named("Grizzly Cub", CardType::Creature),
                r4707_named("Island of Wak-Wak", CardType::Land),
                r4707_named("Mishra's Factory", CardType::Land),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    r4707_settle(&mut driver)?;
    assert!(
        !r4707_on_battlefield(&driver, "Juzám Djinn"),
        "the state trigger took it"
    );
    assert!(
        r4707_on_battlefield(&driver, "City in a Bottle"),
        "the City spares itself"
    );
    assert!(
        driver.begin_cast("Erhnam Djinn").is_err(),
        "an Arabian Nights name can't be cast"
    );
    assert!(
        driver.play_land("Island of Wak-Wak").is_err(),
        "nor played as a land"
    );
    driver.play_land("Mishra's Factory")?;
    driver.cast("Grizzly Cub")?;
    r4707_settle(&mut driver)?;
    assert!(r4707_on_battlefield(&driver, "Grizzly Cub"));
    Ok(())
}
