//! Goal: the storm copies of Aeve, Progenitor Ooze become tokens that are not legendary, at 2 to 4 seats: they live beside the original with no legend-rule question and every decision keeps the invariants, while a second Aeve card meets the legend rule, which asks once for the two cards and leaves one of them with the tokens.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_engine::characteristics::has_supertype;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest, island};
use mtg_testkit::corpus;
use mtg_testkit::invariants::check_decision;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const AEVE: &str = "Aeve, Progenitor Ooze";

/// Aeve as printed: a legendary 2/2 Ooze for {2}{G}{U} with the rules of the corpus.
fn aeve() -> Arc<CardDefinition> {
    let mut card = (*corpus::card(AEVE).unwrap()).clone();
    card.mana_cost = Some(cost(2, &[Color::Green, Color::Blue]));
    card.supertypes = [Supertype::Legendary].into();
    card.types = [CardType::Creature].into();
    card.subtypes = vec![Subtype("Ooze".into())];
    card.power = Some(2);
    card.toughness = Some(2);
    Arc::new(card)
}

/// Seat 0 holds Aeve and `before` one-mana Bears, with Forests and Islands for all of them, and `on_board` Aeves in play.
fn table(seats: usize, before: usize, on_board: usize) -> Scenario {
    let lands = (0..2 + before)
        .map(|_| ready(forest()))
        .chain((0..2).map(|_| ready(island())));
    Scenario::new(seats)
        .battlefield(
            0,
            lands.chain(std::iter::repeat_with(|| ready(aeve())).take(on_board)),
        )
        .hand(
            0,
            (0..before)
                .map(|_| creature("Bear", 0, &[Color::Green], 2, 2))
                .chain([aeve()]),
        )
}

/// Answers passively until the stack is empty, counting the legend-rule questions and checking every decision.
fn settle(driver: &mut Driver) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..400 {
        check_decision(driver.engine()).unwrap();
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            assert!(options.text.starts_with("Legend rule"), "{}", options.text);
            asked += 1;
        }
        let empty = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if empty && matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(asked);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::storm_copies_beside_the_original(2, 0, (3, 2))]
#[case::no_storm_no_copies(0, 0, (1, 0))]
#[case::a_second_card_meets_the_legend_rule(1, 1, (2, 1))]
fn storm_copies_of_aeve_are_not_legendary(
    #[values(2, 3, 4)] seats: usize,
    #[case] before: usize,
    #[case] on_board: usize,
    #[case] expected: (usize, usize),
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, before, on_board))?;
    let mut asked = 0;
    for _ in 0..before {
        driver.cast("Bear")?;
        asked += settle(&mut driver)?;
    }
    driver.cast(AEVE)?;
    asked += settle(&mut driver)?;
    assert_eq!(
        asked, on_board,
        "one legend-rule question for two Aeve cards"
    );
    let state = driver.state();
    let aeves: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).unwrap().card().name == AEVE)
        .collect();
    let tokens = aeves
        .iter()
        .filter(|id| state.object(**id).unwrap().kind() == ObjectKind::Token)
        .count();
    let legendary_tokens = aeves
        .iter()
        .filter(|id| state.object(**id).unwrap().kind() == ObjectKind::Token)
        .filter(|id| has_supertype(state, **id, Supertype::Legendary))
        .count();
    assert_eq!(
        (aeves.len(), tokens, legendary_tokens),
        (expected.0, expected.1, 0)
    );
    Ok(())
}
