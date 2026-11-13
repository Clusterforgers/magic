//! Goal: "Reveal the first card you draw each turn" reveals exactly its controller's first draw of the turn as it is
//! drawn, on any player's turn (Primitive Etchings, Rowen) or only on its controller's own turns (Keranos); "Whenever you
//! reveal a <kind> card this way" triggers only for a revealed card of that kind (a creature, a basic land, a land, a
//! nonland card) and never for a later draw; "You may reveal ... as you draw it" (Inquisitor Eisenhorn) asks its
//! controller, and a card kept hidden reveals and triggers nothing; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::Event;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Words of Wisdom ("You draw two cards, then each other player draws a card.") for no mana.
fn words_of_wisdom() -> Arc<CardDefinition> {
    let mut definition = (*card("Words of Wisdom").unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// A Forest that is not basic.
fn wild_grove() -> Arc<CardDefinition> {
    let mut definition = (*forest()).clone();
    "Wild Grove".clone_into(&mut definition.name);
    definition.supertypes.clear();
    Arc::new(definition)
}

/// The top card of a library by kind.
fn top_card(kind: &str) -> Arc<CardDefinition> {
    match kind {
        "creature" => bear(),
        "basic land" => forest(),
        "nonbasic land" => wild_grove(),
        "instant" => card("Lightning Bolt").unwrap(),
        other => panic!("no card of kind {other}"),
    }
}

fn seat(n: usize) -> PlayerId {
    PlayerId(u64::try_from(n).unwrap())
}

/// Seat 0 holds Words of Wisdom; `owner` controls `permanent`; every library is `top` and then Grizzly Bears.
fn table(seats: usize, owner: usize, permanent: &str, top: &str) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, [words_of_wisdom()]);
    scenario = if owner == 0 {
        scenario.battlefield(0, [ready(island()), ready(card(permanent).unwrap())])
    } else {
        scenario
            .battlefield(owner, [ready(card(permanent).unwrap())])
            .battlefield(0, [ready(island())])
    };
    for at in 0..seats {
        let library: Vec<_> = std::iter::once(top_card(top))
            .chain((0..9).map(|_| bear()))
            .collect();
        scenario = scenario.library(at, library);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn hand(driver: &Driver, at: usize) -> usize {
    driver.state().zone(Zone::Hand(seat(at))).len()
}

/// The reveals of first draws so far, as the player and the card.
fn reveals(driver: &Driver) -> Vec<(PlayerId, ObjectId)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::FirstDrawRevealed { player, card, .. } => Some((*player, *card)),
            _ => None,
        })
        .collect()
}

/// Passes and answers passively until the stack is empty, keeping any card-choice question for the caller.
fn settle(driver: &mut Driver) {
    for _ in 0..40 {
        match &driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::ChooseCards(_) => return,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not empty");
}

/// What seat `at` draws from Words of Wisdom.
const fn drawn(at: usize) -> usize {
    if at == 0 { 2 } else { 1 }
}

#[rstest]
fn primitive_etchings_reveals_the_first_draw_and_draws_for_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[values("creature", "basic land")] top: &str,
) {
    let mut driver = table(seats, owner, "Primitive Etchings", top);
    let first = *driver
        .state()
        .zone(Zone::Library(seat(owner)))
        .last()
        .unwrap();
    driver.cast("Words of Wisdom").unwrap();
    settle(&mut driver);
    // Only the owner's first card is revealed, whoever's turn it is; the second card (a creature) never triggers.
    assert_eq!(reveals(&driver), [(seat(owner), first)]);
    let more = usize::from(top == "creature");
    assert_eq!(hand(&driver, owner), drawn(owner) + more);
}

#[rstest]
fn rowen_draws_only_for_a_basic_land(
    #[values(2, 3, 4)] seats: usize,
    #[values("creature", "basic land", "nonbasic land")] top: &str,
) {
    let mut driver = table(seats, 0, "Rowen", top);
    driver.cast("Words of Wisdom").unwrap();
    settle(&mut driver);
    assert_eq!(reveals(&driver).len(), 1);
    assert_eq!(hand(&driver, 0), 2 + usize::from(top == "basic land"));
}

#[rstest]
fn keranos_reveals_only_on_its_controllers_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[values("creature", "basic land")] top: &str,
) {
    let mut driver = table(seats, owner, "Keranos, God of Storms", top);
    driver.cast("Words of Wisdom").unwrap();
    settle(&mut driver);
    let damage: u32 = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt { amount, .. } => Some(*amount),
            _ => None,
        })
        .sum();
    if owner == 0 {
        assert_eq!(reveals(&driver).len(), 1);
        let land = top == "basic land";
        assert_eq!(hand(&driver, 0), 2 + usize::from(land));
        assert_eq!(damage, if land { 0 } else { 3 });
    } else {
        // Seat 0's turn is no turn of Keranos's controller.
        assert_eq!(reveals(&driver), Vec::new());
        assert_eq!(hand(&driver, owner), 1);
        assert_eq!(damage, 0);
    }
}

#[rstest]
fn eisenhorn_asks_and_makes_cherubael_for_a_revealed_instant(
    #[values(2, 3, 4)] seats: usize,
    #[values("instant", "creature")] top: &str,
    #[values(false, true)] reveal: bool,
) {
    let mut driver = table(seats, 0, "Inquisitor Eisenhorn", top);
    let first = *driver.state().zone(Zone::Library(seat(0))).last().unwrap();
    driver.cast("Words of Wisdom").unwrap();
    settle(&mut driver);
    let asked = driver.decision().unwrap().clone();
    let DecisionKind::ChooseCards(options) = &asked.kind else {
        panic!("the reveal is asked: {asked:?}");
    };
    assert_eq!(asked.player, seat(0));
    assert_eq!(options.from, [first]);
    let chosen = if reveal { vec![first] } else { Vec::new() };
    driver
        .answer(Answer::ChooseCards(CardsChoice(chosen)))
        .unwrap();
    settle(&mut driver);
    assert_eq!(reveals(&driver).len(), usize::from(reveal));
    let cherubael = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::TokenCreated(token) if token.name == "Cherubael"));
    assert_eq!(cherubael, reveal && top == "instant");
}
