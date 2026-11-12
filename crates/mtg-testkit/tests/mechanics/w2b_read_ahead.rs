//! Goal: a Saga with read ahead (CR 702.155) asks its controller for a chapter as it enters, at tables of two, three and four seats: it enters with that many lore counters, only that chapter triggers (the lower ones never do, also when counters are added later), the choice is remembered as `Chosen::Chapter`, the Saga is sacrificed after its last chapter as usual, and a Saga without the keyword asks nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::Event;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::plains;
use mtg_testkit::saga_cards::saga;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Chapter I gains 1 life, II gains 2 and III gains 4.
const THREE: &[(&[u32], i32)] = &[(&[1], 1), (&[2], 2), (&[3], 4)];

/// The Saga "Tale", with read ahead when `reads_ahead`.
fn tale(reads_ahead: bool) -> Arc<CardDefinition> {
    let card = saga("Tale", THREE);
    if !reads_ahead {
        return card;
    }
    Arc::new(
        (*card)
            .clone()
            .with_rules(Ability::Keyword(Keyword::ReadAhead)),
    )
}

/// Seat 0 casts Tale with a Plains in its first main phase; every seat passes until a choice is asked or the stack is
/// empty. Returns the chapter options offered, if any.
fn cast(seats: usize, reads_ahead: bool) -> (Driver, Option<Vec<ChoiceOption>>) {
    let scenario = Scenario::new(seats)
        .hand(0, [tale(reads_ahead)])
        .battlefield(0, [ready(plains())])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Tale").unwrap();
    for _ in 0..=seats {
        if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
            assert_eq!(driver.decision().unwrap().player, pid(0));
            let offered = options.options.clone();
            return (driver, Some(offered));
        }
        driver.pass().unwrap();
    }
    (driver, None)
}

/// Passes until the stack is empty and nothing resolves; the life gained, in order.
fn settle(driver: &mut Driver) -> Vec<u32> {
    let mut gained = Vec::new();
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return gained;
        }
        let events = driver.answer_passively().unwrap();
        gained.extend(events.iter().filter_map(|event| match event {
            Event::LifeGained { amount, .. } => Some(*amount),
            _ => None,
        }));
    }
    panic!("the stack never emptied");
}

/// The Saga on the battlefield, if it is there.
fn on_battlefield(driver: &Driver) -> Option<&mtg_engine::object::GameObject> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == "Tale")
}

#[rstest]
fn read_ahead_starts_at_the_chosen_chapter(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] chapter: u32,
) {
    let (mut driver, offered) = cast(seats, true);
    let labels: Vec<ChoiceOption> = (1..=3)
        .map(|n| ChoiceOption::Label(format!("Chapter {n}")))
        .collect();
    assert_eq!(offered.as_deref(), Some(labels.as_slice()));
    let pick = usize::try_from(chapter - 1).unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    let gained = settle(&mut driver);
    let life = [1, 2, 4][pick];
    assert_eq!(gained, [life], "only chapter {chapter} triggers");
    let saga = on_battlefield(&driver);
    if chapter == 3 {
        assert!(
            saga.is_none(),
            "the last chapter resolved, so the Saga is sacrificed"
        );
        return;
    }
    let permanent = saga.and_then(|object| object.permanent()).unwrap();
    assert_eq!(permanent.counters_of(&CounterKind::lore()), chapter);
    assert_eq!(permanent.chosen(), Some(&Chosen::Chapter(chapter)));
}

#[rstest]
fn the_chapters_skipped_never_trigger_and_the_next_one_does(#[values(2, 3, 4)] seats: usize) {
    let (mut driver, _) = cast(seats, true);
    driver
        .answer(Answer::ChooseOption(OptionChoice(1)))
        .unwrap();
    assert_eq!(settle(&mut driver), [2]);
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1).unwrap();
    assert_eq!(settle(&mut driver), [4]);
    assert!(on_battlefield(&driver).is_none());
}

#[rstest]
fn a_saga_without_read_ahead_asks_nothing_and_starts_at_chapter_one(
    #[values(2, 3, 4)] seats: usize,
) {
    let (mut driver, offered) = cast(seats, false);
    assert!(offered.is_none());
    assert_eq!(settle(&mut driver), [1]);
    let permanent = on_battlefield(&driver).and_then(|o| o.permanent()).unwrap();
    assert_eq!(permanent.counters_of(&CounterKind::lore()), 1);
    assert_eq!(permanent.chosen(), None);
}
