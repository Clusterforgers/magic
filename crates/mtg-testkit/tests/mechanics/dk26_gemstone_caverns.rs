//! Goal: Gemstone Caverns in an opening hand is offered only to a player who is not the starting player; chosen, it
//! begins the game on the battlefield with a luck counter and its owner then exiles exactly one card of their choice from
//! their hand before the game starts; declined, it costs nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::format::Normal;
use mtg_engine::setup::GameSetup;
use mtg_engine::state::Stage;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

fn filler() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Filler")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// Both seats' decks are all Gemstone Caverns.
fn opening(seed: u64) -> Result<Driver, ScriptError> {
    let deck = || -> Vec<Arc<CardDefinition>> {
        (0..40)
            .map(|at| {
                if at % 2 == 0 {
                    card("Gemstone Caverns").unwrap()
                } else {
                    filler()
                }
            })
            .collect()
    };
    let mut setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
    setup.validate_decks = false;
    Driver::new(Engine::new(setup)?)
}

fn keep_everyone(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    Ok(())
}

fn caverns_in_hand(driver: &Driver, seat: PlayerId) -> usize {
    driver
        .state()
        .zone(Zone::Hand(seat))
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Gemstone Caverns")
        .count()
}

#[rstest]
fn only_the_player_who_does_not_start_is_offered_the_caverns(
    #[values(1, 2, 3, 4)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = opening(seed)?;
    keep_everyone(&mut driver)?;
    let first = driver.state().first_player();
    let asked: Vec<PlayerId> = driver
        .engine()
        .decisions()
        .iter()
        .filter(|decision| matches!(decision.kind, DecisionKind::ChooseCards(_)))
        .map(|decision| decision.player)
        .collect();
    assert!(
        !asked.contains(&first),
        "the starting player is never asked"
    );
    Ok(())
}

#[rstest]
fn the_caverns_begin_with_a_luck_counter_and_cost_a_card_of_the_hand(
    #[values(1, 2, 3, 4, 5)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = opening(seed)?;
    keep_everyone(&mut driver)?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        // The other seat holds no Caverns this seed.
        assert_eq!(driver.state().stage(), Stage::Turns);
        return Ok(());
    };
    let seat = decision.player;
    assert_ne!(seat, driver.state().first_player());
    let held = driver.state().zone(Zone::Hand(seat)).len();
    let caverns = options.from[0];
    driver.answer(Answer::ChooseCards(CardsChoice(vec![caverns])))?;
    // The exile is asked next: exactly one card of the hand.
    let DecisionKind::ChooseCards(exile) = driver.decision()?.kind.clone() else {
        panic!("the exile from the hand is asked");
    };
    assert_eq!(driver.decision()?.player, seat);
    assert_eq!((exile.min, exile.max), (1, 1));
    assert_eq!(exile.from.len(), held - 1);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(Vec::new())))
            .is_err()
    );
    driver.answer(Answer::ChooseCards(CardsChoice(vec![exile.from[0]])))?;
    assert_eq!(driver.state().stage(), Stage::Turns);
    assert_eq!(driver.state().zone(Zone::Hand(seat)).len(), held - 2);
    assert_eq!(driver.state().zone(Zone::Exile), [exile.from[0]]);
    let on_battlefield = driver.state().object(caverns).unwrap();
    assert_eq!(on_battlefield.zone(), Zone::Battlefield);
    let luck = on_battlefield
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("luck".to_owned()));
    assert_eq!(luck, 1);
    assert!(caverns_in_hand(&driver, seat) < held);
    Ok(())
}

#[rstest]
fn declining_the_caverns_costs_nothing(#[values(1, 2, 3)] seed: u64) -> Result<(), ScriptError> {
    let mut driver = opening(seed)?;
    keep_everyone(&mut driver)?;
    let decision = driver.decision()?.clone();
    if !matches!(decision.kind, DecisionKind::ChooseCards(_)) {
        return Ok(());
    }
    let held = driver.state().zone(Zone::Hand(decision.player)).len();
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    assert_eq!(driver.state().stage(), Stage::Turns);
    assert_eq!(driver.state().zone(Zone::Hand(decision.player)).len(), held);
    assert_eq!(driver.state().zone(Zone::Exile), []);
    Ok(())
}
