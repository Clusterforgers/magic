//! Goal: a permission that works once each turn on any turn plays as printed at tables of two, three and four seats:
//! under As Foretold with N time counters one spell of mana value N or less is cast for {0} each turn, on its
//! controller's turn and on an opponent's (an instant), never a spell of a greater mana value and never a second one in
//! the same turn; the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

/// A card named `name` of type `kind` costing `cost`, with no rules.
fn r5500_card(name: &str, kind: CardType, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// As Foretold with `counters` time counters.
fn r5500_as_foretold(counters: u32) -> mtg_engine::setup::PresetPermanent {
    with_counters(
        ready(card("As Foretold").unwrap()),
        CounterKind::Other("time".to_owned()),
        counters,
    )
}

/// Whether seat 0, holding priority, may cast the card `name` now.
fn r5500_offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let decision = driver.decision()?;
    if decision.player != pid(0) {
        return Ok(false);
    }
    let DecisionKind::Priority(options) = &decision.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

/// Casts `name` for {0} rather than its mana cost `printed`, until it has left the stack.
fn r5500_cast_free(driver: &mut Driver, name: &str, printed: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..20 {
        let in_hand = driver
            .state()
            .zone(Zone::Hand(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == name);
        let on_stack = driver
            .state()
            .stack()
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == name);
        if !in_hand && !on_stack {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| {
                        matches!(option, ChoiceOption::Label(label) if label != &format!("Cast for {printed}"))
                    })
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck(format!("{name} did not resolve")))
}

/// Passes until seat 0 holds priority with an empty stack.
fn r5500_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..10 {
        let decision = driver.decision()?;
        if decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("seat 0 never held priority".to_owned()))
}

fn r5500_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// On its controller's turn, with no lands: a {3}{U} enchantment is cast for {0} under 4 or more time counters and not
/// under 3; after one such cast a second enchantment of the same cost is not offered that turn.
#[rstest]
#[case::four_counters(4, true)]
#[case::five_counters(5, true)]
#[case::three_counters(3, false)]
fn r5500_as_foretold_casts_one_spell_a_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] open: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [r5500_as_foretold(counters)])
        .hand(
            0,
            [
                r5500_card("First Omen", CardType::Enchantment, "{3}{U}"),
                r5500_card("Second Omen", CardType::Enchantment, "{3}{U}"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r5500_offered(&driver, "First Omen")?, open);
    if open {
        r5500_cast_free(&mut driver, "First Omen", "{3}{U}")?;
        assert!(
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .any(|id| driver.state().object(*id).unwrap().card().name == "First Omen")
        );
        r5500_settle(&mut driver)?;
        assert!(!r5500_offered(&driver, "Second Omen")?, "once each turn");
        r5500_replays(&driver);
    }
    Ok(())
}

/// On an opponent's turn seat 0 casts a {1}{U} instant for {0} under 2 time counters: "once each turn" is not only on
/// its controller's turns.
#[rstest]
fn r5500_as_foretold_works_on_an_opponents_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [r5500_as_foretold(2)])
        .hand(0, [r5500_card("Quick Omen", CardType::Instant, "{1}{U}")]);
    let mut driver = Driver::from_scenario(scenario)?;
    r5500_settle(&mut driver)?;
    assert!(r5500_offered(&driver, "Quick Omen")?);
    r5500_cast_free(&mut driver, "Quick Omen", "{1}{U}")?;
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Quick Omen")
    );
    r5500_replays(&driver);
    Ok(())
}
