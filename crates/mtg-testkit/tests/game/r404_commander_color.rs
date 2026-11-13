//! Goal: a commander whose card says "If ~ is your commander, choose a color before the game begins. ~ is the chosen
//! color." asks its owner for a color before any hand is dealt, in turn order, and is that color in the command zone and
//! on the battlefield; a card with no such ability asks nothing, at tables of two to four seats; the log replays to the
//! same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::replay;
use mtg_engine::setup::GameSetup;
use mtg_engine::{Engine, Event};
use mtg_testkit::Driver;
use rstest::rstest;

/// A legendary creature with the commander color ability (or none).
fn piper(name: &str, asks: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_supertype(Supertype::Legendary)
        .with_power_toughness(2, 2);
    if asks {
        card = card.with_rules(
            serde_json::from_str(
                r#"{"Static":{"affected":"This","mods":["SetCommanderColor"],"defining":true}}"#,
            )
            .unwrap(),
        );
    }
    Arc::new(card)
}

fn filler() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Filler")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

fn colors_of(driver: &Driver, name: &str) -> ColorSet {
    let id = driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap()
        .id();
    mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .colors
}

#[rstest]
fn each_owner_chooses_a_color_for_their_commander_before_the_hands(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 4)] pick: usize,
) {
    let mut setup = GameSetup::new(
        Arc::new(Commander),
        7,
        (0..seats)
            .map(|_| (0..30).map(|_| filler()).collect())
            .collect(),
    );
    setup.validate_decks = false;
    // Seat 0's and seat 1's commanders ask; the last seat's does not.
    for seat in 0..seats {
        let asks = seat + 1 < seats;
        setup.players[seat].commanders = vec![piper(&format!("Piper {seat}"), asks)];
    }
    let mut driver = Driver::new(Engine::new(setup.clone()).unwrap()).unwrap();
    let mut asked = Vec::new();
    let mut chosen = Vec::new();
    while let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
        let player = driver.decision().unwrap().player;
        assert!(
            driver
                .state()
                .zone(mtg_engine::zones::Zone::Hand(player))
                .is_empty(),
            "no hand before the color"
        );
        assert_eq!(options.options.len(), 5);
        let ChoiceOption::Color(color) = options.options[pick] else {
            panic!("a color is offered");
        };
        asked.push(player);
        chosen.push(color);
        let events = driver
            .answer(Answer::ChooseOption(OptionChoice(pick)))
            .unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            Event::CommanderColorChosen { player: by, color: c, .. } if *by == player && *c == color
        )));
    }
    assert_eq!(
        asked.len(),
        seats - 1,
        "one question per commander that asks"
    );
    let first = driver.state().first_player();
    let order: Vec<PlayerId> = driver
        .state()
        .turn_order_from(first)
        .into_iter()
        .filter(|seat| seat.index() + 1 < seats)
        .collect();
    assert_eq!(asked, order, "asked in turn order from the first player");
    let DecisionKind::Mulligan(mulligan) = &driver.decision().unwrap().kind else {
        panic!("the mulligan follows the colors");
    };
    assert_eq!(
        (mulligan.hand.len(), mulligan.free_mulligans),
        (7, 1),
        "the first mulligan stays free"
    );
    for seat in 0..seats {
        let colors = colors_of(&driver, &format!("Piper {seat}"));
        if seat + 1 < seats {
            assert_eq!(
                colors,
                std::iter::once(Color::ALL[pick]).collect(),
                "seat {seat}"
            );
        } else {
            assert!(
                colors.is_empty(),
                "a commander without the ability stays colorless"
            );
        }
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
