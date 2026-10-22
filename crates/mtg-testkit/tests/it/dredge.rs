//! Goal: dredge (CR 702.52) replaces a draw of the card's owner at 2 to 4 seats: a yes mills N cards and returns the card from the graveyard to the hand with no card drawn, a no draws as usual, a library with fewer than N cards offers nothing, one with exactly N offers it; in a draw of several cards each draw is asked on its own, a dredged card is not offered again, the cards milled are not drawn, a milled card that dredges can replace a later draw of the batch, and the library left by one dredge may be too short for the next; two dredging cards and Thought Reflection are put in the order the player picks, with a declined dredge asked again for the draw it doubles; a commander on top is milled or drawn by the order of its replacement; the draw step asks too; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::choose::{ReplacementChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: usize = 0;
const REFLECTION: &str = "Thought Reflection";
const MOSSDOG: &str = "Greater Mossdog";
const DARKBLAST: &str = "Darkblast";
const SHENANIGANS: &str = "Shenanigans";

/// How the player answers the questions of a draw, in the order they are asked.
#[derive(Clone, Copy, Debug)]
enum Say {
    /// Yes to the question.
    Yes,
    /// No to the question.
    No,
    /// The effect called so, picked first when two or more apply.
    First(&'static str),
}
use Say::{First, No, Yes};

/// One draw of seat 0 against a stacked library.
#[derive(Clone, Copy, Debug)]
struct Shape {
    /// The graveyard before.
    graveyard: &'static [&'static str],
    /// The cards in the library, which holds "Lib n" for its place n unless `at` names the card there.
    size: usize,
    /// Named cards at places of the library.
    at: &'static [(usize, &'static str)],
    /// Thought Reflection is in play.
    reflection: bool,
    /// The cards drawn at once.
    count: u32,
    /// The answers, in the order the questions are asked.
    says: &'static [Say],
    /// The hand afterwards, sorted.
    hand: &'static [&'static str],
    /// The graveyard afterwards, sorted.
    after: &'static [&'static str],
    /// The card on top of the library afterwards; empty for an empty library.
    top: &'static str,
    /// The cards drawn, not dredged.
    drawn: usize,
}

const PLAIN: Shape = Shape {
    graveyard: &[MOSSDOG],
    size: 8,
    at: &[],
    reflection: false,
    count: 1,
    says: &[],
    hand: &[],
    after: &[MOSSDOG],
    top: "Lib 0",
    drawn: 0,
};

/// The card at place `n` of a library built from `shape`.
fn library_card(shape: &Shape, n: usize) -> Arc<CardDefinition> {
    match shape.at.iter().find(|(place, _)| *place == n) {
        Some((_, name)) => card(name).unwrap(),
        None => free_creature(&format!("Lib {n}"), 1, 1),
    }
}

/// A table where seat 0 has the graveyard and library of `shape` and seat 1 punishes each card an opponent draws.
fn table(seats: usize, shape: &Shape) -> Scenario {
    let mut permanents = Vec::new();
    if shape.reflection {
        permanents.push(ready(card(REFLECTION).unwrap()));
    }
    Scenario::new(seats)
        .graveyard_named(SEAT, shape.graveyard.iter().copied())
        .library(SEAT, (0..shape.size).map(|n| library_card(shape, n)))
        .battlefield(SEAT, permanents)
        .battlefield(1, [ready(card("Underworld Dreams").unwrap())])
}

/// Answers what the draw asks until the stack is empty; returns how many yes or no questions and orderings were asked.
fn resolve(driver: &mut Driver, says: &[Say]) -> Result<(usize, usize), ScriptError> {
    let mut given = says.iter();
    let (mut asked, mut ordered) = (0, 0);
    for _ in 0..200 {
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(options) => {
                let yes = match given.next().unwrap_or_else(|| panic!("an answer for {options:?}")) {
                    Yes => true,
                    No => false,
                    First(name) => panic!("a yes or no was asked, not the order of {name}"),
                };
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseReplacement(options) => {
                let Some(First(first)) = given.next() else {
                    panic!("the order of effects was asked: {options:?}");
                };
                let pick = options
                    .candidates
                    .iter()
                    .position(|c| name_of(driver.state(), c.source) == *first)
                    .expect("the effect is among the candidates");
                ordered += 1;
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(pick)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => {
                assert!(given.next().is_none(), "every answer was asked for");
                return Ok((asked, ordered));
            }
        }
    }
    Err(ScriptError::Stuck("the draw".into()))
}

fn sorted(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found = names(driver.state(), zone);
    found.sort();
    found
}

fn drawn_in(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event, Event::CardDrawn { .. }))
        .count()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let rebuilt = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
}

/// The number of questions of the answers that are not an order of effects.
fn questions(says: &[Say]) -> usize {
    says.iter()
        .filter(|say| matches!(say, Yes | No))
        .count()
}

#[rstest]
// One dredging card and one draw.
#[case::yes(Shape { says: &[Yes], hand: &[MOSSDOG], after: &["Lib 0", "Lib 1", "Lib 2"], top: "Lib 3", ..PLAIN })]
#[case::no(Shape { says: &[No], hand: &["Lib 0"], top: "Lib 1", drawn: 1, ..PLAIN })]
#[case::one_card(Shape { graveyard: &[SHENANIGANS], says: &[Yes], hand: &[SHENANIGANS], after: &["Lib 0"], top: "Lib 1", ..PLAIN })]
#[case::an_instant(Shape { graveyard: &[DARKBLAST], says: &[Yes], hand: &[DARKBLAST], after: &["Lib 0", "Lib 1", "Lib 2"], top: "Lib 3", ..PLAIN })]
// The library holds fewer cards than the number, or exactly as many.
#[case::too_few_cards(Shape { size: 2, after: &[MOSSDOG], hand: &["Lib 0"], top: "Lib 1", drawn: 1, ..PLAIN })]
#[case::exactly_enough(Shape { size: 3, says: &[Yes], hand: &[MOSSDOG], after: &["Lib 0", "Lib 1", "Lib 2"], top: "", ..PLAIN })]
// Several draws in a batch.
#[case::two_draws_first_dredged(Shape { count: 2, says: &[Yes], hand: &[MOSSDOG, "Lib 3"], after: &["Lib 0", "Lib 1", "Lib 2"], top: "Lib 4", drawn: 1, ..PLAIN })]
#[case::two_draws_second_dredged(Shape { count: 2, says: &[No, Yes], hand: &[MOSSDOG, "Lib 0"], after: &["Lib 1", "Lib 2", "Lib 3"], top: "Lib 4", drawn: 1, ..PLAIN })]
#[case::two_draws_both_declined(Shape { count: 2, says: &[No, No], hand: &["Lib 0", "Lib 1"], top: "Lib 2", drawn: 2, ..PLAIN })]
#[case::three_draws_one_dredge(Shape { count: 3, says: &[No, No, Yes], hand: &[MOSSDOG, "Lib 0", "Lib 1"], after: &["Lib 2", "Lib 3", "Lib 4"], top: "Lib 5", drawn: 2, ..PLAIN })]
// Two dredging cards.
#[case::the_second_picked(Shape { graveyard: &[MOSSDOG, DARKBLAST], says: &[First(DARKBLAST), Yes], hand: &[DARKBLAST], after: &[MOSSDOG, "Lib 0", "Lib 1", "Lib 2"], top: "Lib 3", ..PLAIN })]
#[case::the_picked_declined_and_the_other_taken(Shape { graveyard: &[MOSSDOG, DARKBLAST], says: &[First(DARKBLAST), No, Yes], hand: &[MOSSDOG], after: &[DARKBLAST, "Lib 0", "Lib 1", "Lib 2"], top: "Lib 3", ..PLAIN })]
#[case::both_declined(Shape { graveyard: &[MOSSDOG, DARKBLAST], says: &[First(DARKBLAST), No, No], hand: &["Lib 0"], after: &[DARKBLAST, MOSSDOG], top: "Lib 1", drawn: 1, ..PLAIN })]
#[case::one_for_each_of_two_draws(Shape { graveyard: &[MOSSDOG, DARKBLAST], count: 2, says: &[First(MOSSDOG), Yes, Yes], hand: &[DARKBLAST, MOSSDOG], after: &["Lib 0", "Lib 1", "Lib 2", "Lib 3", "Lib 4", "Lib 5"], top: "Lib 6", ..PLAIN })]
// The library the first dredge leaves is too short for the second.
#[case::the_library_runs_short(Shape { graveyard: &[MOSSDOG, DARKBLAST], size: 5, count: 2, says: &[First(MOSSDOG), Yes], hand: &[MOSSDOG, "Lib 3"], after: &[DARKBLAST, "Lib 0", "Lib 1", "Lib 2"], top: "Lib 4", drawn: 1, ..PLAIN })]
// A card milled by one dredge dredges for the next draw.
#[case::a_milled_card_dredges(Shape { at: &[(1, SHENANIGANS)], count: 2, says: &[Yes, Yes], hand: &[MOSSDOG, SHENANIGANS], after: &["Lib 0", "Lib 2", "Lib 3"], top: "Lib 4", ..PLAIN })]
#[case::a_milled_card_is_declined(Shape { at: &[(1, SHENANIGANS)], count: 2, says: &[Yes, No], hand: &[MOSSDOG, "Lib 3"], after: &["Lib 0", "Lib 2", SHENANIGANS], top: "Lib 4", drawn: 1, ..PLAIN })]
// Thought Reflection makes the draw two.
#[case::reflection_first_then_dredge(Shape { reflection: true, says: &[First(REFLECTION), Yes], hand: &[MOSSDOG, "Lib 3"], after: &["Lib 0", "Lib 1", "Lib 2"], top: "Lib 4", drawn: 1, ..PLAIN })]
#[case::dredge_first_then_no_draw_is_left_to_double(Shape { reflection: true, says: &[First(MOSSDOG), Yes], hand: &[MOSSDOG], after: &["Lib 0", "Lib 1", "Lib 2"], top: "Lib 3", ..PLAIN })]
#[case::reflection_first_dredge_declined_for_both(Shape { reflection: true, says: &[First(REFLECTION), No, No], hand: &["Lib 0", "Lib 1"], top: "Lib 2", drawn: 2, ..PLAIN })]
#[case::reflection_first_dredge_for_the_second(Shape { reflection: true, says: &[First(REFLECTION), No, Yes], hand: &[MOSSDOG, "Lib 0"], after: &["Lib 1", "Lib 2", "Lib 3"], top: "Lib 4", drawn: 1, ..PLAIN })]
#[case::dredge_declined_then_reflection(Shape { reflection: true, says: &[First(MOSSDOG), No, Yes], hand: &[MOSSDOG, "Lib 0"], after: &["Lib 1", "Lib 2", "Lib 3"], top: "Lib 4", drawn: 1, ..PLAIN })]
fn dredge_replaces_a_draw_when_its_owner_says_so(
    #[values(2, 3, 4)] seats: usize,
    #[case] shape: Shape,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &shape);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count: shape.count })?;
    let (asked, ordered) = resolve(&mut driver, shape.says)?;
    let log = driver.engine().events()[begun..].to_vec();
    assert_eq!(asked, questions(shape.says), "{log:?}");
    assert_eq!(ordered, shape.says.len() - asked, "{log:?}");
    assert_eq!(sorted(&driver, Zone::Hand(pid(SEAT))), shape.hand, "{log:?}");
    assert_eq!(
        sorted(&driver, Zone::Graveyard(pid(SEAT))),
        shape.after,
        "{log:?}"
    );
    assert_eq!(
        names(driver.state(), Zone::Library(pid(SEAT)))
            .last()
            .map_or("", String::as_str),
        shape.top
    );
    assert_eq!(drawn_in(&log), shape.drawn, "{log:?}");
    // Underworld Dreams hurts the player for each card drawn: a dredge is no draw.
    let life = driver.state().player(pid(SEAT)).unwrap().life();
    assert_eq!(life, 20 - i32::try_from(shape.drawn).unwrap());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_card_in_another_graveyard_does_not_replace_the_draw(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(seats, &PLAIN)
            .graveyard_named(SEAT, [] as [&str; 0])
            .graveyard_named(1, [MOSSDOG])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.manual(ManualAction::Draw { count: 1 })?;
    let (asked, ordered) = resolve(&mut driver, &[])?;
    assert_eq!((asked, ordered), (0, 0));
    assert_eq!(sorted(&driver, Zone::Hand(pid(SEAT))), ["Lib 0"]);
    assert_eq!(sorted(&driver, Zone::Graveyard(pid(1))), [MOSSDOG]);
    Ok(())
}

#[rstest]
#[case::yes(&[Yes], &[MOSSDOG], 20)]
#[case::no(&[No], &["Lib 0"], 19)]
fn the_draw_step_asks_the_player_whose_turn_it_is(
    #[values(2, 3, 4)] seats: usize,
    #[case] says: &[Say],
    #[case] hand: &[&str],
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &PLAIN).at(3, SEAT, Step::Upkeep);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    for _ in 0..seats {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.pass()?;
    }
    let (asked, _) = resolve(&mut driver, says)?;
    assert_eq!(asked, 1);
    assert_eq!(sorted(&driver, Zone::Hand(pid(SEAT))), hand);
    assert_eq!(driver.state().player(pid(SEAT)).unwrap().life(), life);
    replays(&driver, &setup);
    Ok(())
}

/// A Commander table whose seat 0 has Greater Mossdog in the graveyard and the commander "Cmd" on top of the library.
fn commander_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(SEAT, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .graveyard_named(SEAT, [MOSSDOG])
        .library(SEAT, (0..8).map(|n| free_creature(&format!("Lib {n}"), 1, 1)))
}

/// Moves the commander to the top of the library, keeping it there against the question of the command zone.
fn put_commander_on_top(driver: &mut Driver) -> Result<(), ScriptError> {
    let object = driver
        .state()
        .zone(Zone::Command(pid(SEAT)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Cmd")
        .unwrap();
    driver.manual(ManualAction::Move {
        object,
        to: ManualZone::LibraryTop,
    })?;
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    Ok(())
}

#[rstest]
// The dredge applies first: the commander is the top card, so it is milled into the graveyard, where its owner may
// still put it into the command zone (CR 903.9a).
#[case::dredge_first_commander_stays(&[First(MOSSDOG), Yes, No], &[MOSSDOG], &[], &["Cmd", "Lib 0", "Lib 1"])]
#[case::dredge_first_commander_goes_to_the_command_zone(&[First(MOSSDOG), Yes, Yes], &[MOSSDOG], &["Cmd"], &["Lib 0", "Lib 1"])]
// The commander applies first and is sent away: the draw is gone and nothing is left to dredge.
#[case::commander_first_sent_away(&[First("Cmd"), Yes], &[], &["Cmd"], &[MOSSDOG])]
// The commander applies first and is kept: it is drawn, unless the dredge then replaces the draw.
#[case::commander_kept_then_dredge(&[First("Cmd"), No, Yes, No], &[MOSSDOG], &[], &["Cmd", "Lib 0", "Lib 1"])]
#[case::commander_kept_then_drawn(&[First("Cmd"), No, No], &["Cmd"], &[], &[MOSSDOG])]
fn a_commander_on_top_is_milled_or_drawn_by_the_order_of_the_effects(
    #[values(2, 3, 4)] seats: usize,
    #[case] says: &[Say],
    #[case] hand: &[&str],
    #[case] command: &[&str],
    #[case] graveyard: &[&str],
) -> Result<(), ScriptError> {
    let setup = commander_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(commander_table(seats))?;
    put_commander_on_top(&mut driver)?;
    let begun = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count: 1 })?;
    let (asked, ordered) = resolve(&mut driver, says)?;
    let log = driver.engine().events()[begun..].to_vec();
    assert_eq!((asked, ordered), (questions(says), 1), "{log:?}");
    assert_eq!(sorted(&driver, Zone::Hand(pid(SEAT))), hand, "{log:?}");
    assert_eq!(sorted(&driver, Zone::Command(pid(SEAT))), command, "{log:?}");
    assert_eq!(
        sorted(&driver, Zone::Graveyard(pid(SEAT))),
        graveyard,
        "{log:?}"
    );
    replays(&driver, &setup);
    Ok(())
}
