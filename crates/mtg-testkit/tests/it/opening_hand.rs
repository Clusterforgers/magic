//! Goal: a card with the opening-hand ability is offered to its owner once the hand is final, and the cards chosen begin the game on the battlefield while the rest stay in hand.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::format::Normal;
use mtg_engine::setup::GameSetup;
use mtg_engine::state::Stage;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event};
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

fn leyline() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Leyline")
            .with_type(CardType::Enchantment)
            .with_rules(Ability::OpeningHand),
    )
}

fn filler() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Filler")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// A two-seat game where seat 0's deck holds `leylines` of the cards and seat 1's none.
fn opening(leylines: usize, seed: u64) -> Result<Driver, ScriptError> {
    let deck = |count: usize| -> Vec<Arc<CardDefinition>> {
        (0..40)
            .map(|at| if at < count { leyline() } else { filler() })
            .collect()
    };
    let mut setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(leylines), deck(0)]);
    setup.validate_decks = false;
    Driver::new(Engine::new(setup)?)
}

fn keep_everyone(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    Ok(())
}

#[rstest]
fn the_chosen_cards_begin_on_the_battlefield_and_the_rest_stay_in_hand(
    #[values(0, 40)] leylines: usize,
    #[values(0, 1, 3, 7)] take: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = opening(leylines, seed)?;
    keep_everyone(&mut driver)?;
    let seat = PlayerId(0);
    let held = driver.state().zone(Zone::Hand(seat)).to_vec();
    let offered = match &driver.decision()?.kind {
        DecisionKind::ChooseCards(options) => {
            assert_eq!(driver.decision()?.player, seat);
            assert_eq!(options.min, 0);
            options.from.clone()
        }
        _ => Vec::new(),
    };
    if leylines == 0 {
        assert!(offered.is_empty(), "no card is offered without the ability");
        assert_eq!(driver.state().stage(), Stage::Turns);
        return Ok(());
    }
    assert_eq!(offered.len(), held.len(), "every card of the hand has it");
    assert!(offered.iter().all(|id| held.contains(id)));
    let chosen: Vec<_> = offered.iter().copied().take(take).collect();
    let events = driver.answer(Answer::ChooseCards(CardsChoice(chosen.clone())))?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::OpeningCardsPlayed { cards, .. } if *cards == chosen
    )));
    keep_everyone(&mut driver)?;
    assert_eq!(driver.state().stage(), Stage::Turns);
    assert_eq!(driver.state().zone(Zone::Battlefield).len(), chosen.len());
    assert_eq!(
        driver.state().zone(Zone::Hand(seat)).len(),
        held.len() - chosen.len()
    );
    Ok(())
}

#[rstest]
fn nothing_begins_on_the_battlefield_while_a_seat_still_decides(
    #[values(1, 2, 3, 4)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = opening(40, seed)?;
    // One seat keeps; the other has not answered yet.
    let first = driver.decision()?.player;
    driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    let asked = driver.engine().decisions();
    assert!(
        asked
            .iter()
            .all(|decision| matches!(decision.kind, DecisionKind::Mulligan(_))),
        "no opening-hand question while a seat decides"
    );
    assert_eq!(driver.state().zone(Zone::Battlefield), []);
    assert!(asked.iter().all(|decision| decision.player != first));
    // Once the last seat keeps, the one seat with such cards is asked.
    driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    let asked = driver.engine().decisions();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].player, PlayerId(0));
    assert!(matches!(asked[0].kind, DecisionKind::ChooseCards(_)));
    Ok(())
}
