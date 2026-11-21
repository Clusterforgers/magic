//! Goal: Shared Fate at 2 to 4 seats: a draw becomes an exile of the top card of the library of an opponent the drawing
//! player chooses, face down; each player looks at and plays only the cards they exiled with it (a per-player `exiled`
//! memory read by `Pred::R114ExiledByThem` for the player a permission or a look is worked out for, CR 607.2a), and the
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact sacrificed at no mana: "Draw a card."
fn study(name: &str) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Draw":{}}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

/// Activates the named Study for the player with priority; whoever chooses an opponent picks seat `victim`; resolves it.
fn draw_with(driver: &mut Driver, study: &str, victim: usize) -> Result<(), ScriptError> {
    driver.begin_ability(study, 0)?;
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty() || !state.pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Player(pid(victim)))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the draw".into()))
}

/// The card of this name in exile.
fn exiled(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    let state = driver.state();
    state
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .unwrap()
}

#[rstest]
fn r114_each_player_plays_the_cards_they_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] pick: usize,
) -> Result<(), ScriptError> {
    let victim = 1 + pick % (seats - 1);
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Shared Fate").unwrap()),
                ready(study("Study A")),
            ],
        )
        .battlefield(1, [ready(study("Study B"))])
        .library(0, [free_creature("Mine", 1, 1), plains()]);
    for seat in 1..seats {
        scenario = scenario.library(
            seat,
            [free_creature(&format!("Fate {seat}"), 2, 2), plains()],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    draw_with(&mut driver, "Study A", victim)?;
    let fate = format!("Fate {victim}");
    assert!(names(driver.state(), Zone::Hand(pid(0))).is_empty());
    let taken = exiled(&driver, &fate);
    assert!(driver.state().object(taken).unwrap().face_down().is_some());
    // Seat 1 draws on seat 0's turn and exiles seat 0's top card (its only opponent at two seats, chosen otherwise).
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    draw_with(&mut driver, "Study B", 0)?;
    let mine = exiled(&driver, "Mine");
    let state = driver.state();
    for seat in 0..seats {
        assert_eq!(state.may_look_at(pid(seat), taken), seat == 0, "seat {seat}");
        assert_eq!(state.may_look_at(pid(seat), mine), seat == 1, "seat {seat}");
    }
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority");
    };
    assert!(options.castable.contains(&taken));
    assert!(!options.castable.contains(&mine));
    driver.cast(&fate)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert!(names(driver.state(), Zone::Battlefield).contains(&fate));
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
