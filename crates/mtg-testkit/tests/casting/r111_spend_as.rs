//! Goal: mana spent as though it were another color (CR 609.4b) at 2 to 4 seats: under Sunglasses of Urza white mana
//! pays a red symbol, the red half of a hybrid and a red Phyrexian symbol, by hand and automatically, and never another
//! color; under Celestial Dawn white pays every colored symbol, other mana pays no colored symbol but pays {C}; and a
//! sacrifice tax on activated abilities (Drought, Brutal Suppression) is paid by the mana abilities that pay a spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaSymbol;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature_costing, forest, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const R: ManaSymbol = ManaSymbol::Colored(Color::Red);
const U: ManaSymbol = ManaSymbol::Colored(Color::Blue);
const G: ManaSymbol = ManaSymbol::Colored(Color::Green);

/// Seat 0 controls `rule` (when given) and `lands`, all untapped, and holds a 2/2 costing `symbols`; seat 1 controls
/// `other`.
fn game(
    seats: usize,
    rule: Option<&str>,
    lands: Vec<Arc<CardDefinition>>,
    symbols: &[ManaSymbol],
    other: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut own: Vec<_> = lands.into_iter().map(ready).collect();
    own.extend(rule.map(|name| ready(card(name).unwrap())));
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![creature_costing("Paid One", symbols, 2, 2)])
        .battlefield(0, own)
        .battlefield(1, other.into_iter().map(ready));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// Whether the spell got onto the stack.
fn on_stack(driver: &Driver) -> bool {
    names(driver.state(), Zone::Stack) == ["Paid One"]
}

#[rstest]
#[case::red(&[ManaSymbol::Generic(1), R], true)]
#[case::two_red(&[R, R], true)]
#[case::red_hybrid(&[ManaSymbol::Hybrid(Color::Red, Color::Green)], true)]
#[case::red_mono_hybrid(&[ManaSymbol::MonoHybrid(Color::Red), R], true)]
#[case::red_phyrexian(&[ManaSymbol::Phyrexian(Color::Red), ManaSymbol::Generic(1)], true)]
#[case::blue(&[U], false)]
fn sunglasses_let_white_pay_red(
    #[values(2, 3, 4)] seats: usize,
    #[case] symbols: &[ManaSymbol],
    #[case] paid: bool,
) -> Result<(), ScriptError> {
    let lands = vec![plains(), plains()];
    let mut with = game(
        seats,
        Some("Sunglasses of Urza"),
        lands.clone(),
        symbols,
        Vec::new(),
    )?;
    assert_eq!(with.cast("Paid One").is_ok() && on_stack(&with), paid);
    // Without the rule white mana pays no red symbol (a Phyrexian one still takes life).
    let mut without = game(seats, None, lands, symbols, Vec::new())?;
    let phyrexian = symbols.contains(&ManaSymbol::Phyrexian(Color::Red));
    assert_eq!(
        without.cast("Paid One").is_ok() && on_stack(&without),
        phyrexian
    );
    Ok(())
}

#[rstest]
fn sunglasses_white_pays_red_by_hand(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = game(
        seats,
        Some("Sunglasses of Urza"),
        vec![plains(), mountain()],
        &[R, R],
        Vec::new(),
    )?;
    driver.cast_tapping("Paid One", &["Plains", "Mountain"])?;
    assert!(on_stack(&driver));
    Ok(())
}

#[rstest]
#[case::colored_by_white(vec![plains(), plains()], &[ManaSymbol::Generic(1), U], true)]
#[case::blue_pays_no_blue(vec![dork("Blue", "Blue"), dork("Blue", "Blue")], &[U], false)]
#[case::blue_pays_generic(vec![dork("Blue", "Blue"), dork("Blue", "Blue")], &[ManaSymbol::Generic(2)], true)]
#[case::green_pays_colorless(vec![dork("Green", "Green")], &[ManaSymbol::Colorless], true)]
#[case::white_pays_no_colorless(vec![plains()], &[ManaSymbol::Colorless], false)]
#[case::colorless_pays_colorless(vec![dork("Colorless", "Colorless")], &[ManaSymbol::Colorless], true)]
#[case::green_pays_no_green(vec![dork("Green", "Green")], &[G], false)]
fn celestial_dawn_spends_white_as_any_color_and_the_rest_as_colorless(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: Vec<Arc<CardDefinition>>,
    #[case] symbols: &[ManaSymbol],
    #[case] paid: bool,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, Some("Celestial Dawn"), lands, symbols, Vec::new())?;
    assert_eq!(driver.cast("Paid One").is_ok() && on_stack(&driver), paid);
    Ok(())
}

/// How many cards named `name` lie in seat 0's graveyard.
fn in_graveyard(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Graveyard(pid(0)))
        .iter()
        .filter(|held| held.as_str() == name)
        .count()
}

#[rstest]
#[case::a_swamp_to_spare(2, true)]
#[case::no_swamp_to_spare(1, false)]
fn drought_taxes_a_mana_ability_with_a_black_symbol(
    #[values(2, 3, 4)] seats: usize,
    #[case] swamps: usize,
    #[case] paid: bool,
) -> Result<(), ScriptError> {
    // Graven Cairns' {B/R} ability pays {R}{R} of the spell; under Drought it sacrifices a Swamp, which no other
    // source of the payment may be.
    let mut lands = vec![card("Graven Cairns").unwrap(), mountain()];
    lands.extend((0..swamps).map(|_| swamp()));
    let mut driver = game(
        seats,
        None,
        lands,
        &[R, R, R],
        vec![card("Drought").unwrap()],
    )?;
    assert_eq!(driver.cast("Paid One").is_ok() && on_stack(&driver), paid);
    if paid {
        assert_eq!(in_graveyard(&driver, "Swamp"), 1);
    }
    Ok(())
}

/// A creature named "<name> Dork" with "{T}: Add <mana>" (a mana type as the card files name it).
fn dork(name: &str, mana: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["{mana}"]}}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(format!("{name} Dork"))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str::<Ability>(&ability).unwrap()),
    )
}

/// A nontoken Rebel creature with "{T}: Add {G}".
fn rebel_elf() -> Arc<CardDefinition> {
    let dork = dork("Green", "Green");
    let mut rebel = (*dork).clone();
    rebel.name = "Rebel Elf".into();
    Arc::new(rebel.with_subtype("Rebel"))
}

#[rstest]
#[case::a_land_to_spare(true)]
#[case::no_land_to_spare(false)]
fn brutal_suppression_taxes_a_rebels_mana_ability(
    #[values(2, 3, 4)] seats: usize,
    #[case] spare: bool,
) -> Result<(), ScriptError> {
    let mut lands = vec![rebel_elf(), forest()];
    if spare {
        lands.push(plains());
    }
    let mut driver = game(
        seats,
        None,
        lands,
        &[G, G],
        vec![card("Brutal Suppression").unwrap()],
    )?;
    assert_eq!(driver.cast("Paid One").is_ok() && on_stack(&driver), spare);
    if spare {
        assert_eq!(in_graveyard(&driver, "Plains"), 1);
    }
    Ok(())
}
