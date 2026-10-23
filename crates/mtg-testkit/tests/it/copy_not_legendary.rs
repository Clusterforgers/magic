//! Goal: a token copy "except it isn't legendary" (Quantum Misalignment) is not a legend, at 2 to 4 seats: the copy of a legendary creature lives beside the original with no legend-rule choice, while a copy that keeps the supertype (Cackling Counterpart) leaves one of the two, a copy of a creature that is not legendary is the same either way, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Supertype;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};

const SEAT: PlayerId = PlayerId(0);

/// Seat 0 controls `hero` and five Islands and holds the corpus card `spell`.
fn table(seats: usize, hero: Arc<CardDefinition>, spell: &str) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            std::iter::once(ready(hero)).chain((0..5).map(|_| ready(island()))),
        )
        .hand_named(0, [spell])
}

fn legendary_hero() -> Arc<CardDefinition> {
    legend("Hero", 1, &[Color::Blue], 2, 2)
}

fn plain_hero() -> Arc<CardDefinition> {
    creature("Hero", 1, &[Color::Blue], 2, 2)
}

/// The permanents named Hero seat 0 controls, and how many of the tokens among them are legendary.
fn heroes(driver: &Driver) -> (usize, usize, usize) {
    let state = driver.state();
    let found: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap())
        .filter(|object| object.card().name == "Hero" && object.controller() == SEAT)
        .collect();
    let tokens: Vec<_> = found
        .iter()
        .filter(|object| object.kind() == ObjectKind::Token)
        .collect();
    let legendary = tokens
        .iter()
        .filter(|object| object.card().supertypes.contains(&Supertype::Legendary))
        .count();
    (found.len(), tokens.len(), legendary)
}

#[rstest]
#[case::the_exception_keeps_both("Quantum Misalignment", true, (2, 1, 0))]
#[case::a_plain_copy_meets_the_legend_rule("Cackling Counterpart", true, (1, 0, 0))]
#[case::nothing_to_lose_for_a_plain_creature("Quantum Misalignment", false, (2, 1, 0))]
#[case::a_plain_copy_of_a_plain_creature("Cackling Counterpart", false, (2, 1, 0))]
fn a_copy_that_is_not_legendary_lives_beside_the_original(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] legendary: bool,
    #[case] expected: (usize, usize, usize),
) -> Result<(), ScriptError> {
    let hero = || {
        if legendary {
            legendary_hero()
        } else {
            plain_hero()
        }
    };
    let setup = table(seats, hero(), spell).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, hero(), spell))?;
    let target = driver.target_named("Hero")?;
    driver.begin_cast(spell)?;
    driver.choose_targets(&[target])?;
    let taps: &[&str] = if spell == "Quantum Misalignment" {
        &["Island", "Island", "Island", "Island", "Island"]
    } else {
        &["Island", "Island", "Island"]
    };
    driver.pay_tapping(taps, 0)?;
    settle(&mut driver)?;
    // The legend rule asks which of two legends to keep (CR 704.5j) before anyone has priority again.
    let mut asked = 0;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        asked += 1;
        assert!(asked <= 1, "one question for the legend rule");
        driver.answer_passively()?;
    }
    assert_eq!(
        asked,
        usize::from(spell == "Cackling Counterpart" && legendary)
    );
    let (all, tokens, legends) = heroes(&driver);
    // Which legend stays is the controller's pick, so only the number of permanents and the legends among the tokens
    // are fixed.
    assert_eq!((all, legends), (expected.0, expected.2));
    assert!(tokens <= expected.1, "{tokens} tokens");
    replays(&driver, &setup);
    Ok(())
}
