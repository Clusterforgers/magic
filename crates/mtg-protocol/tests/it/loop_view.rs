//! Goal: at two to four seats a loop reaches every seat compactly: the shortcut's thousand iterations arrive as one `LoopRepeated` line with the events it folds counted in `folded`, the batch still folds the view to the same state as a snapshot and stays small, a loop that draws the game names its cards in the table, and the loop question and its answers keep their wire form.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::loops::LoopAnswer;
use mtg_protocol::{DecisionName, EventView, StateView, project_events};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 casts Polyraptor beside `damager` and answers as a loop wants until the engine asks about the loop or ends
/// the game.
fn looping(seats: usize, damager: &str) -> Driver {
    let mut permanents: Vec<_> = (0..8).map(|_| ready(land(Color::Green))).collect();
    permanents.push(ready(card(damager).unwrap()));
    let scenario = Scenario::new(seats)
        .hand(0, [card("Polyraptor").unwrap()])
        .battlefield(0, permanents);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Polyraptor").unwrap();
    for _ in 0..400 {
        if driver.state().result().is_some() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Loop(_) => break,
            DecisionKind::ChooseTargets(options) => {
                let newest = options
                    .legal
                    .iter()
                    .filter(|target| matches!(target, Target::Object(_)))
                    .max_by_key(|target| match target {
                        Target::Object(object) => object.id,
                        Target::Player(_) => mtg_core::ids::ObjectId(0),
                    })
                    .copied()
                    .unwrap();
                Answer::ChooseTargets(TargetChoice::Targets(vec![newest]))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    driver
}

#[rstest]
fn a_shortcut_reaches_each_seat_as_one_line_and_folds_to_the_snapshot(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = looping(seats, "Where Ancients Tread");
    let decision = driver.decision().unwrap().clone();
    assert_eq!(DecisionName::of(&decision.kind), DecisionName::Loop);
    let before = driver.state().clone();
    let from_seq = u64::try_from(driver.engine().events().len()).unwrap();
    let events = driver
        .answer(Answer::Loop(LoopAnswer::Repeat(1000)))
        .unwrap();
    assert!(
        events.len() > 10_000,
        "a thousand iterations make many events"
    );
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, from_seq).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let lines: Vec<_> = batch
            .events
            .iter()
            .filter(|view| matches!(view, EventView::Public(Event::LoopRepeated(_))))
            .collect();
        assert_eq!(lines.len(), 1);
        let EventView::Public(Event::LoopRepeated(summary)) = lines[0] else {
            unreachable!()
        };
        assert_eq!(summary.iterations, 1000);
        assert_eq!(batch.folded, u64::from(summary.events));
        assert_eq!(
            batch.events.len() + usize::try_from(batch.folded).unwrap(),
            events.len()
        );
        let mut view = StateView::for_player(&before, *viewer);
        view.apply(batch);
        assert_eq!(view, StateView::for_player(driver.state(), *viewer));
        let size = serde_json::to_string(batch).unwrap().len();
        assert!(size < 100_000, "{size} bytes");
    }
}

#[rstest]
fn a_loop_that_draws_names_its_cards_in_the_table(#[values(2, 3, 4)] seats: usize) {
    let driver = looping(seats, "Aether Flash");
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    let cards = view.table.loop_draw.expect("drawn by a loop");
    assert!(cards.iter().any(|card| card == "Aether Flash"));
    assert!(cards.iter().any(|card| card == "Polyraptor"));
}

#[test]
fn the_loop_answers_keep_their_wire_form() {
    for (answer, json) in [
        (LoopAnswer::Repeat(100), r#"{"Loop":{"Repeat":100}}"#),
        (LoopAnswer::Stop, r#"{"Loop":"Stop"}"#),
        (LoopAnswer::Draw, r#"{"Loop":"Draw"}"#),
    ] {
        let answer = Answer::Loop(answer);
        assert_eq!(serde_json::to_string(&answer).unwrap(), json);
        assert_eq!(serde_json::from_str::<Answer>(json).unwrap(), answer);
    }
}
