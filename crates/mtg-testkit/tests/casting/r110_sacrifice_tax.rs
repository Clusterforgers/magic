//! Goal: a rule that adds "Sacrifice a <permanent>" to costs (CR 601.2f, 602.2b) at 2 to 4 seats: under Drought a spell
//! of any player sacrifices one Swamp for each black symbol of its mana cost (hybrid included) with its other costs and
//! a spell without one sacrifices nothing, a spell whose caster lacks the Swamps is not cast, and an activated ability of
//! the objects a rule names sacrifices for each black symbol of its activation cost or once when the rule counts none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaSymbol;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature_costing, forest, gain_life, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// A 2/2 creature card for exactly these symbols.
fn costing(symbols: &[ManaSymbol]) -> Arc<CardDefinition> {
    creature_costing("Taxed One", symbols, 2, 2)
}

/// An enchantment whose one rule taxes the activated abilities of "Pump Rig" with a land, per black symbol or once.
fn rig_taxer(per_symbol: Option<Color>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rig Taxer")
            .with_type(CardType::Enchantment)
            .with_rules(Ability::Rule {
                rule: GameRule::R110SacrificeTax {
                    spells: None,
                    abilities: Some(Selector(vec![Pred::Named("Pump Rig".into())])),
                    sacrifice: Selector(vec![Pred::Type(CardType::Land)]),
                    per_symbol,
                },
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// An artifact with "{B}{B}: You gain 1 life."
fn pump_rig() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Pump Rig")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Mana("{B}{B}".parse().unwrap())]),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                by: Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![gain_life(1)],
            }),
    )
}

/// Seat 0 holds `hand` and controls `swamps` untapped Swamps and `own`; seat 1 controls Drought.
fn game(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    swamps: usize,
    own: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut lands: Vec<_> = (0..swamps).map(|_| ready(swamp())).collect();
    lands.extend(own.into_iter().map(ready));
    let mut scenario = Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, lands)
        .battlefield(1, vec![ready(card("Drought").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// Answers the announcement: the mana paid automatically, the sacrifices taken from the first permanents offered.
/// Returns how many permanents the sacrifices asked for.
fn finish(driver: &mut Driver) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..20 {
        match driver.decision()?.kind.clone() {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) => {
                let count = usize::try_from(options.min).unwrap();
                asked += count;
                let chosen: Vec<ObjectId> = options.from.iter().copied().take(count).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ => return Ok(asked),
        }
    }
    Err(ScriptError::Stuck("the taxed announcement".into()))
}

/// How many Swamps lie in seat 0's graveyard.
fn swamps_sacrificed(driver: &Driver) -> usize {
    names(driver.state(), Zone::Graveyard(pid(0)))
        .iter()
        .filter(|name| name.as_str() == "Swamp")
        .count()
}

#[rstest]
#[case::two_black(&[ManaSymbol::Generic(1), ManaSymbol::Colored(Color::Black), ManaSymbol::Colored(Color::Black)], 2)]
#[case::hybrid_counts(&[ManaSymbol::Hybrid(Color::Black, Color::Red)], 1)]
#[case::no_black(&[ManaSymbol::Generic(2)], 0)]
fn a_spell_sacrifices_a_swamp_for_each_black_symbol(
    #[values(2, 3, 4)] seats: usize,
    #[case] symbols: &[ManaSymbol],
    #[case] sacrificed: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, vec![costing(symbols)], 5, Vec::new())?;
    driver.begin_cast("Taxed One")?;
    assert_eq!(finish(&mut driver)?, sacrificed);
    assert_eq!(swamps_sacrificed(&driver), sacrificed);
    let stack: Vec<String> = driver
        .state()
        .zone(Zone::Stack)
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert_eq!(stack, ["Taxed One"]);
    Ok(())
}

#[test]
fn a_spell_whose_caster_lacks_the_swamps_is_not_cast() -> Result<(), ScriptError> {
    let symbols = [
        ManaSymbol::Colored(Color::Black),
        ManaSymbol::Colored(Color::Black),
        ManaSymbol::Colored(Color::Black),
    ];
    let mut driver = game(2, vec![costing(&symbols)], 2, Vec::new())?;
    let cast = driver
        .begin_cast("Taxed One")
        .and_then(|()| finish(&mut driver));
    let on_stack = !driver.state().zone(Zone::Stack).is_empty();
    assert!(
        cast.is_err() || !on_stack,
        "three Swamps are needed and two are there"
    );
    assert_eq!(swamps_sacrificed(&driver), 0);
    Ok(())
}

#[rstest]
// Drought, in play too, adds one Swamp for each of the two black symbols of Pump Rig's cost.
#[case::per_black_symbol(Some(Color::Black), 4)]
#[case::once(None, 3)]
fn an_activated_ability_of_a_named_object_sacrifices_lands(
    #[values(2, 4)] seats: usize,
    #[case] per_symbol: Option<Color>,
    #[case] sacrificed: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(
        seats,
        Vec::new(),
        6,
        vec![pump_rig(), rig_taxer(per_symbol)],
    )?;
    driver.begin_activation("Pump Rig")?;
    assert_eq!(finish(&mut driver)?, sacrificed);
    assert_eq!(swamps_sacrificed(&driver), sacrificed);
    assert_eq!(
        driver.state().zone(Zone::Stack).len(),
        1,
        "the ability is on the stack"
    );
    Ok(())
}
