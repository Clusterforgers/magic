//! Goal: a creature that must enter as a copy (`ReplaceBy::AsCopy` with `must`, Essence of the Wild) becomes a copy of
//! the permanent without a question, only when its controller is the one the replacement names; lands of a chosen
//! nonbasic type (`ValueKind::NonbasicLandType`, March from Velis Vel) become copies of a creature with haste until end of
//! turn and lands of another type do not, at any table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_testkit::cards::{cost, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The corpus card with no mana cost, so it is cast without lands.
fn free(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        card(name)
            .unwrap()
            .as_ref()
            .clone()
            .with_mana_cost(cost(0, &[])),
    )
}

/// Passes priority until the stack is empty: a choice of option takes the one labelled `pick`, anything else is
/// answered passively; how many choices of cards were asked.
fn settle(driver: &mut Driver, pick: &str) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(
                        |option| matches!(option, ChoiceOption::Label(label) if label == pick),
                    )
                    .expect("the type is offered");
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(_) => {
                asked += 1;
                driver.answer_passively()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn r370_a_creature_enters_as_a_copy_unasked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let cub = free_creature("Cub", 2, 2);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(card("Essence of the Wild").unwrap())])
            .battlefield(seats - 1, [ready(free_creature("Ogre", 3, 3))])
            .hand(0, [cub]),
    )?;
    driver.cast("Cub")?;
    assert_eq!(settle(&mut driver, "")?, 0, "nobody is asked");
    assert!(driver.id_of("Cub").is_err(), "the Cub is a copy now");
    let copies: Vec<_> = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Essence of the Wild")
        .collect();
    assert_eq!(copies.len(), 2);
    for id in copies {
        let seen = characteristics(driver.state(), *id).unwrap();
        assert_eq!((seen.power, seen.toughness), (Some(6), Some(6)));
    }
    let ogre = driver.id_of("Ogre")?;
    assert_eq!(
        characteristics(driver.state(), ogre).unwrap().power,
        Some(3)
    );
    Ok(())
}

#[rstest]
fn r370_lands_of_the_chosen_type_become_copies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let gate = || {
        Arc::new(
            CardDefinition::new("Gate")
                .with_type(CardType::Land)
                .with_subtype("Gate"),
        )
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(free_creature("Ogre", 3, 3)),
                    ready(gate()),
                    ready(gate()),
                    ready(plains()),
                ],
            )
            .battlefield(seats - 1, [ready(gate())])
            .hand(0, [free("March from Velis Vel")]),
    )?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("March from Velis Vel", &[ogre])?;
    settle(&mut driver, "Gate")?;
    let state = driver.state();
    let ogres: Vec<_> = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).unwrap().card().name == "Ogre")
        .collect();
    assert_eq!(ogres.len(), 3, "the Ogre and the two Gates");
    for id in &ogres {
        let seen = characteristics(state, *id).unwrap();
        assert!(!seen.types.contains(CardType::Land));
        assert_eq!(seen.power, Some(3));
    }
    let hasty = ogres
        .iter()
        .filter(|id| has_keyword(state, **id, &Keyword::Haste))
        .count();
    assert_eq!(hasty, 2, "only the lands gain haste");
    let gates = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == "Gate")
        .count();
    assert_eq!(gates, 1, "the opponent's Gate stays");
    Ok(())
}

/// Passes priority until the stack is empty, answering every "may" with `may`.
fn settle_may(driver: &mut Driver, may: bool) -> Result<(), ScriptError> {
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(may)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::copies(true, 2)]
#[case::declines(false, 0)]
fn r370_creatures_become_copies_of_the_revealed_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] may: bool,
    #[case] giants: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Deceiver of Form").unwrap()),
                    ready(free_creature("Ogre", 3, 3)),
                    ready(free_creature("Bear", 2, 2)),
                ],
            )
            .library(0, [free_creature("Giant", 5, 5), free_creature("Elf", 1, 1)]),
    )?;
    driver.pass_until(mtg_engine::turn::Step::BeginCombat)?;
    settle_may(&mut driver, may)?;
    let state = driver.state();
    let named = |name: &str| {
        state
            .zone(mtg_engine::zones::Zone::Battlefield)
            .iter()
            .filter(|id| state.object(**id).unwrap().card().name == name)
            .count()
    };
    assert_eq!(named("Giant"), giants);
    assert_eq!(named("Deceiver of Form"), 1, "the Deceiver stays itself");
    Ok(())
}
