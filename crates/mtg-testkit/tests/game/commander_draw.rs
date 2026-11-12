//! Goal: a commander that would be drawn is offered the command zone instead of the hand (CR 903.9b) at 2 to 6 seats: a yes moves it out of the library with no draw (no card is drawn for it, "whenever an opponent draws" sees only the other cards, the cards below it are drawn in its place), a no draws it, and it is asked only when it is among the cards drawn; it holds on top, in the middle and last of a draw of several cards, for both cards of a Partner pair, in the draw step, under Thought Reflection in either order of the two effects (each draw it makes is a card of its own), and for a library with nothing below it; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, names, pid};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{ReplacementChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::{Event, ZoneCause};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::commander_cards::{legend, partner_legend};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: usize = 0;
const REFLECTION: &str = "Thought Reflection";

/// A card of the library, from the top down.
#[derive(Clone, Copy, Debug)]
enum Top {
    /// A creature called "Card n" for its place n in the stack.
    Card,
    /// The commander "Cmd".
    Cmd,
    /// The commander "Pal", the partner of "Cmd".
    Pal,
}
use Top::{Card, Cmd, Pal};

/// What one draw of a stacked library does.
#[derive(Clone, Copy, Debug)]
struct Shape {
    /// The top of the library, top card first; the "Base n" cards lie below.
    stack: &'static [Top],
    /// The cards drawn at once.
    count: u32,
    /// Thought Reflection is in play.
    doubled: bool,
    /// The effect picked first when the player orders two: a card name.
    first: &'static str,
    /// The answers to the questions of the commanders, in the order they are asked.
    answers: &'static [bool],
    /// How many times the player orders two effects.
    orders: usize,
    /// The hand afterwards, sorted.
    hand: &'static [&'static str],
    /// The command zone afterwards, sorted.
    command: &'static [&'static str],
    /// The card on top of the library afterwards.
    top: &'static str,
}

const PLAIN: Shape = Shape {
    stack: &[],
    count: 1,
    doubled: false,
    first: "",
    answers: &[],
    orders: 0,
    hand: &[],
    command: &[],
    top: "Base 0",
};

fn doubled(shape: Shape, first: &'static str) -> Shape {
    Shape {
        doubled: true,
        first,
        ..shape
    }
}

/// A Commander table whose seat 0 has the stacked cards in its hand and the commanders in its command zone.
fn table(seats: usize, stack: &[Top], base: usize, doubled: bool) -> Scenario {
    let fillers = stack
        .iter()
        .enumerate()
        .filter(|(_, top)| matches!(top, Card))
        .map(|(at, _)| free_creature(&format!("Card {at}"), 1, 1));
    let mut commanders = vec![legend("Cmd", 1, &[Color::Red], 2, 2)];
    if stack.iter().any(|top| matches!(top, Pal)) {
        commanders.push(partner_legend("Pal", 1, &[Color::Red], 2, 2));
    }
    let mut permanents = Vec::new();
    if doubled {
        permanents.push(ready(card(REFLECTION).unwrap()));
    }
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(SEAT, commanders)
        .hand(SEAT, fillers)
        .library(
            SEAT,
            (0..base).map(|n| free_creature(&format!("Base {n}"), 1, 1)),
        )
        .battlefield(SEAT, permanents)
        .battlefield(1, [ready(card("Underworld Dreams").unwrap())])
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap()
}

fn say(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    Ok(())
}

/// Puts the cards of the stack on top of seat 0's library, each commander kept there by a no.
fn stack_the_library(driver: &mut Driver, stack: &[Top]) -> Result<(), ScriptError> {
    for (at, top) in stack.iter().enumerate().rev() {
        let (zone, name) = match top {
            Card => (Zone::Hand(pid(SEAT)), format!("Card {at}")),
            Cmd => (Zone::Command(pid(SEAT)), "Cmd".to_owned()),
            Pal => (Zone::Command(pid(SEAT)), "Pal".to_owned()),
        };
        let object = find(driver, zone, &name);
        driver.manual(ManualAction::Move {
            object,
            to: ManualZone::LibraryTop,
        })?;
        if !matches!(top, Card) {
            say(driver, false)?;
        }
    }
    Ok(())
}

/// Answers what the draw asks until the stack is empty: the commanders by `answers`, the order of two effects by
/// `first`. Returns how many commander questions and orderings were asked.
fn resolve(
    driver: &mut Driver,
    answers: &[bool],
    first: &str,
) -> Result<(usize, usize), ScriptError> {
    let mut given = answers.iter();
    let (mut asked, mut ordered) = (0, 0);
    for _ in 0..200 {
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => {
                let yes = *given.next().expect("an answer for the question");
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseReplacement(options) => {
                let pick = options
                    .candidates
                    .iter()
                    .position(|c| name_of(driver.state(), c.source) == first)
                    .unwrap_or(0);
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

fn log_since(driver: &Driver, begun: usize) -> Vec<Event> {
    driver.engine().events()[begun..].to_vec()
}

fn drawn_in(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event, Event::CardDrawn { .. }))
        .count()
}

/// The moves from the library to the command zone that the log holds.
fn pulled_in(log: &[Event]) -> Vec<ObjectId> {
    log.iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from: Zone::Library(from),
                to: Zone::Command(to),
                cause: ZoneCause::Moved,
            } if *from == pid(SEAT) && *to == pid(SEAT) => Some(*object),
            _ => None,
        })
        .collect()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let rebuilt = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
}

#[rstest]
// One commander on top.
#[case::on_top_accepted(Shape { stack: &[Cmd], answers: &[true], command: &["Cmd"], ..PLAIN })]
#[case::on_top_refused(Shape { stack: &[Cmd], answers: &[false], hand: &["Cmd"], ..PLAIN })]
// A commander the draw does not reach is not asked about.
#[case::below_the_cards_drawn(Shape { stack: &[Card, Cmd], hand: &["Card 0"], top: "Cmd", ..PLAIN })]
// The second, the third and the last card of a draw of several.
#[case::second_of_two_accepted(Shape { stack: &[Card, Cmd], count: 2, answers: &[true], hand: &["Card 0"], command: &["Cmd"], ..PLAIN })]
#[case::second_of_two_refused(Shape { stack: &[Card, Cmd], count: 2, answers: &[false], hand: &["Card 0", "Cmd"], ..PLAIN })]
#[case::third_of_three_accepted(Shape { stack: &[Card, Card, Cmd], count: 3, answers: &[true], hand: &["Card 0", "Card 1"], command: &["Cmd"], ..PLAIN })]
#[case::middle_of_three_accepted(Shape { stack: &[Card, Cmd, Card], count: 3, answers: &[true], hand: &["Card 0", "Card 2"], command: &["Cmd"], ..PLAIN })]
#[case::middle_of_three_refused(Shape { stack: &[Card, Cmd, Card], count: 3, answers: &[false], hand: &["Card 0", "Card 2", "Cmd"], ..PLAIN })]
#[case::fourth_of_five_accepted(Shape { stack: &[Card, Card, Card, Cmd, Card], count: 5, answers: &[true], hand: &["Card 0", "Card 1", "Card 2", "Card 4"], command: &["Cmd"], ..PLAIN })]
#[case::cards_below_are_drawn_in_its_place(Shape { stack: &[Cmd], count: 2, answers: &[true], hand: &["Base 0"], command: &["Cmd"], top: "Base 1", ..PLAIN })]
// A Partner pair.
#[case::pair_both_accepted(Shape { stack: &[Cmd, Pal], count: 2, answers: &[true, true], command: &["Cmd", "Pal"], ..PLAIN })]
#[case::pair_first_accepted(Shape { stack: &[Cmd, Pal], count: 2, answers: &[true, false], hand: &["Pal"], command: &["Cmd"], ..PLAIN })]
#[case::pair_second_accepted(Shape { stack: &[Cmd, Pal], count: 2, answers: &[false, true], hand: &["Cmd"], command: &["Pal"], ..PLAIN })]
#[case::pair_both_refused(Shape { stack: &[Cmd, Pal], count: 2, answers: &[false, false], hand: &["Cmd", "Pal"], ..PLAIN })]
#[case::pair_apart(Shape { stack: &[Cmd, Card, Pal], count: 3, answers: &[true, true], hand: &["Card 1"], command: &["Cmd", "Pal"], ..PLAIN })]
#[case::pair_one_drawn(Shape { stack: &[Cmd, Pal], answers: &[true], command: &["Cmd"], top: "Pal", ..PLAIN })]
// Thought Reflection: each draw is two, and each of the two is a card of its own.
#[case::doubled_commander_first_accepted(doubled(Shape { stack: &[Cmd], answers: &[true], orders: 1, command: &["Cmd"], ..PLAIN }, "Cmd"))]
#[case::doubled_commander_first_refused(doubled(Shape { stack: &[Cmd], answers: &[false], orders: 1, hand: &["Base 0", "Cmd"], top: "Base 1", ..PLAIN }, "Cmd"))]
#[case::doubled_reflection_first_accepted(doubled(Shape { stack: &[Cmd], answers: &[true], orders: 1, hand: &["Base 0"], command: &["Cmd"], top: "Base 1", ..PLAIN }, REFLECTION))]
#[case::doubled_reflection_first_refused(doubled(Shape { stack: &[Cmd], answers: &[false], orders: 1, hand: &["Base 0", "Cmd"], top: "Base 1", ..PLAIN }, REFLECTION))]
#[case::doubled_second_card_accepted(doubled(Shape { stack: &[Card, Cmd], answers: &[true], hand: &["Card 0"], command: &["Cmd"], ..PLAIN }, REFLECTION))]
#[case::doubled_pair_commander_first(doubled(Shape { stack: &[Cmd, Pal], answers: &[false, true], orders: 1, hand: &["Cmd"], command: &["Pal"], ..PLAIN }, "Cmd"))]
#[case::doubled_pair_reflection_first(doubled(Shape { stack: &[Cmd, Pal], answers: &[true, true], orders: 1, command: &["Cmd", "Pal"], ..PLAIN }, REFLECTION))]
#[case::doubled_batch(doubled(Shape { stack: &[Card, Cmd], count: 2, answers: &[true], hand: &["Base 0", "Base 1", "Card 0"], command: &["Cmd"], top: "Base 2", ..PLAIN }, REFLECTION))]
fn a_commander_that_would_be_drawn_goes_to_the_command_zone_when_its_owner_says_so(
    #[values(2, 3, 4, 5, 6)] seats: usize,
    #[case] shape: Shape,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, shape.stack, 10, shape.doubled);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    stack_the_library(&mut driver, shape.stack)?;
    let library = names(driver.state(), Zone::Library(pid(SEAT))).len();
    let begun = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count: shape.count })?;
    let (asked, ordered) = resolve(&mut driver, shape.answers, shape.first)?;
    let log = log_since(&driver, begun);
    assert_eq!(asked, shape.answers.len(), "{log:?}");
    assert_eq!(ordered, shape.orders, "{log:?}");
    assert_eq!(
        sorted(&driver, Zone::Hand(pid(SEAT))),
        shape.hand,
        "{log:?}"
    );
    assert_eq!(
        sorted(&driver, Zone::Command(pid(SEAT))),
        shape.command,
        "{log:?}"
    );
    assert_eq!(
        names(driver.state(), Zone::Library(pid(SEAT))).last(),
        Some(&shape.top.to_owned())
    );
    // A card in the command zone left the library without being drawn: the hand holds the cards drawn.
    assert_eq!(drawn_in(&log), shape.hand.len(), "{log:?}");
    let pulled = pulled_in(&log);
    assert_eq!(pulled.len(), shape.command.len(), "{log:?}");
    assert_eq!(
        names(driver.state(), Zone::Library(pid(SEAT))).len() + shape.hand.len() + pulled.len(),
        library,
    );
    // Underworld Dreams hurts an opponent for each card drawn: the commanders sent away were no draw.
    let life = driver.state().player(pid(SEAT)).unwrap().life();
    assert_eq!(life, 40 - i32::try_from(shape.hand.len()).unwrap());
    for object in pulled {
        assert!(driver.state().is_commander(object));
        assert_eq!(
            driver.state().object(object).unwrap().zone(),
            Zone::Command(pid(SEAT))
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::one_draw(1, 0, 0)]
#[case::two_draws(2, 0, 1)]
#[case::three_draws(3, 0, 2)]
fn a_library_with_nothing_below_the_commander_fails_the_draws_that_are_left(
    #[values(2, 3, 4, 5, 6)] seats: usize,
    #[case] count: u32,
    #[case] drawn: usize,
    #[case] failed: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &[Cmd], 0, false);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    stack_the_library(&mut driver, &[Cmd])?;
    let begun = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count })?;
    say(&mut driver, true)?;
    let log = log_since(&driver, begun);
    let empty = log
        .iter()
        .filter(|event| matches!(event, Event::DrewFromEmptyLibrary { .. }))
        .count();
    // The first draw takes the commander before the next one looks for a card; the commander left no draw failed.
    assert_eq!((drawn_in(&log), empty), (drawn, failed), "{log:?}");
    assert_eq!(pulled_in(&log).len(), 1);
    // A failed draw loses the game at the next state-based check, which a draw replaced by the commander does not.
    let lost = log.iter().any(|event| {
        matches!(
            event,
            Event::PlayerLost {
                reason: LossReason::EmptyLibrary,
                ..
            }
        )
    });
    assert_eq!(lost, failed > 0, "{log:?}");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::accepted(true, &["Cmd"], &[])]
#[case::refused(false, &[], &["Cmd"])]
fn the_draw_step_asks_about_the_commander_it_draws(
    #[values(2, 3, 4, 5, 6)] seats: usize,
    #[case] yes: bool,
    #[case] command: &[&str],
    #[case] hand: &[&str],
) -> Result<(), ScriptError> {
    let turn = u32::try_from(seats).unwrap() + 1;
    let scenario = || table(seats, &[Cmd], 10, false).at(turn, SEAT, Step::Upkeep);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    stack_the_library(&mut driver, &[Cmd])?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().step(), Step::Draw);
    let asked = driver.decision()?.player;
    assert_eq!(asked, pid(SEAT), "the owner of the commander is asked");
    say(&mut driver, yes)?;
    assert_eq!(sorted(&driver, Zone::Command(pid(SEAT))), command);
    assert_eq!(sorted(&driver, Zone::Hand(pid(SEAT))), hand);
    replays(&driver, &setup);
    Ok(())
}
