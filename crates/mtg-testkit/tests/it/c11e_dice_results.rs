//! Goal: the results of a roll as the compiler reads them play whole at two to four seats: two dice with a choice ask the roller between two different numbers and the effects after it read the chosen result and the other one, "whenever you roll one or more dice" triggers once for the roll and "a die's highest natural result" once for each die that rolled its faces, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::rng::GameRng;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A creature for `{0}` with the abilities the compiler reads from this text; the card must read whole.
fn c11e_compiled(name: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789c11".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: "Creature \u{2014} Elemental".to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let compiled = compile(&printed);
    assert!(
        compiled.clauses.iter().all(|clause| clause.read),
        "{compiled:?}"
    );
    let card = compiled.card.expect("the card compiles");
    let mut definition = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_mana_cost(cost(0, &[]));
    for ability in &card.faces[0].abilities {
        definition = definition.with_rules(ability.clone());
    }
    Arc::new(definition)
}

/// Passes and answers passively, and a choice of option by `pick`, until the stack is empty; the choices asked.
fn c11e_play_out(driver: &mut Driver, pick: usize) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(_) => {
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The first seed whose two d6 from the substream `index` roll `first` and then `second`.
fn c11e_seed(index: u64, first: u64, second: u64) -> u64 {
    (0u64..1_000_000)
        .find(|seed| {
            GameRng::substream(*seed, index).below(6) + 1 == first
                && GameRng::substream(*seed, index + 1).below(6) + 1 == second
        })
        .unwrap()
}

fn c11e_start(seats: usize) -> u64 {
    Driver::from_scenario(Scenario::new(seats))
        .unwrap()
        .state()
        .rng_index()
}

fn c11e_replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const C11E_ENDEAVOR: &str = "When this creature enters, roll two d6 and choose one result. You gain life equal to that result. Then each opponent loses X life, where X is the other result.";

/// The roller chooses one of two different numbers; the gain reads it and the loss reads the other.
#[rstest]
#[case::the_first_die(2, 5, 0, 2, 5)]
#[case::the_second_die(2, 5, 1, 5, 2)]
#[case::the_same_number_asks_nothing(4, 4, 0, 4, 4)]
fn c11e_two_dice_with_a_choice_read_the_chosen_result_and_the_other(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: u64,
    #[case] second: u64,
    #[case] pick: usize,
    #[case] gained: i32,
    #[case] lost: i32,
) -> Result<(), ScriptError> {
    let start = c11e_start(seats);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(c11e_seed(start, first, second))
            .hand(0, [c11e_compiled("Endeavor", C11E_ENDEAVOR)]),
    )?;
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let base = life(&driver, 0);
    driver.cast("Endeavor")?;
    let asked = c11e_play_out(&mut driver, pick)?;
    assert_eq!(asked, usize::from(first != second));
    assert_eq!(life(&driver, 0) - base, gained);
    for seat in 1..seats {
        assert_eq!(base - life(&driver, seat), lost, "seat {seat}");
    }
    assert_eq!(
        driver.state().rng_index(),
        start + 2,
        "two dice used two draws"
    );
    c11e_replays(&driver);
    Ok(())
}

const C11E_WATCHER: &str = "Whenever you roll one or more dice, you gain 1 life.\nWhenever you roll a die's highest natural result, draw a card.";

/// A roll of two dice triggers the roll once and the highest result once for each six.
#[rstest]
#[case::no_six(2, 5, 0)]
#[case::one_six(6, 3, 1)]
#[case::two_sixes(6, 6, 2)]
fn c11e_a_roll_triggers_once_and_each_highest_die_once(
    #[values(2, 4)] seats: usize,
    #[case] first: u64,
    #[case] second: u64,
    #[case] sixes: usize,
) -> Result<(), ScriptError> {
    let start = c11e_start(seats);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(c11e_seed(start, first, second))
            .hand(0, [c11e_compiled("Endeavor", C11E_ENDEAVOR)])
            .battlefield(
                0,
                [
                    ready(c11e_compiled("Watcher", C11E_WATCHER)),
                    ready(island()),
                ],
            ),
    )?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let before = hand(&driver);
    let base = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Endeavor")?;
    c11e_play_out(&mut driver, 0)?;
    // The Endeavor left the hand; each six drew a card.
    assert_eq!(hand(&driver) + 1, before + sixes);
    let rolls = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DieRolled { .. }))
        .count();
    assert_eq!(rolls, 2);
    // The first option is the first die: its result, and 1 for the roll once.
    let gained = driver.state().player(pid(0)).unwrap().life() - base;
    assert_eq!(gained, 1 + i32::try_from(first).unwrap());
    c11e_replays(&driver);
    Ok(())
}
