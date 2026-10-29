//! Goal: a sorcery whose only lines are cascade and another keyword plays whole: Into the Time Vortex cascades as it is cast from the hand, is exiled by rebound as it resolves and cascades again when it is cast free at its owner's next upkeep; Throes of Chaos cascades whether it is cast from the hand or retraced from the graveyard and returns to the graveyard each time; at 2 to 4 seats, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;
use std::sync::Arc;

/// The cards called `name` in `zone`.
fn count(state: &GameState, zone: Zone, name: &str) -> usize {
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

fn first(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap()
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Accepts every free cast and answers the rest passively until the stack is empty and a player has priority.
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..80 {
        match driver.decision()?.kind.name() {
            "yes-no" => events.extend(driver.answer(Answer::YesNo(YesNoChoice(true)))?),
            "pay-mana" => events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?),
            "priority" if driver.state().zone(Zone::Stack).is_empty() => return Ok(events),
            "priority" => events.extend(driver.pass()?),
            _ => events.extend(driver.answer_passively()?),
        }
    }
    Err(ScriptError::Stuck("the cascade".to_owned()))
}

/// The number of spells cast free.
fn free_casts(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(event, Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Free))
        })
        .count()
}

/// A Mountain on top of `opts` copies of Opt.
fn library(opts: usize) -> Vec<Arc<CardDefinition>> {
    let opt = corpus::card("Opt").unwrap();
    std::iter::once(mountain())
        .chain(std::iter::repeat_n(opt, opts))
        .collect()
}

/// Seat 0 holds `hand` over `mountains` Mountains, with Opt below a Mountain on top of its library.
fn game(seats: usize, hand: &[&str], mountains: usize) -> Result<Driver, ScriptError> {
    let lands = (0..mountains).map(|_| ready(mountain()));
    Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, hand)
            .library(0, library(4))
            .battlefield(0, lands),
    )
}

#[rstest]
fn into_the_time_vortex_cascades_from_the_hand_and_again_from_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, &["Into the Time Vortex"], 5)?;
    let mut events = driver.cast("Into the Time Vortex")?;
    events.extend(settle(&mut driver)?);
    let graveyard = Zone::Graveyard(pid(0));
    assert_eq!(free_casts(&events), 1);
    assert_eq!(count(driver.state(), graveyard, "Opt"), 1);
    assert_eq!(
        count(driver.state(), Zone::Exile, "Into the Time Vortex"),
        1,
        "rebound exiles the spell as it resolves"
    );
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Upkeep)?;
    let logged = driver.engine().events().len();
    settle(&mut driver)?;
    let later = driver.engine().events()[logged..].to_vec();
    assert_eq!(
        free_casts(&later),
        2,
        "the rebound cast and the card it cascades into"
    );
    assert_eq!(count(driver.state(), graveyard, "Opt"), 2);
    assert_eq!(count(driver.state(), graveyard, "Into the Time Vortex"), 1);
    assert_eq!(
        count(driver.state(), Zone::Exile, "Into the Time Vortex"),
        0
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::from_the_hand(true)]
#[case::retraced(false)]
fn throes_of_chaos_cascades_and_returns_to_the_graveyard(
    #[case] in_hand: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .library(0, library(2))
        .battlefield(0, (0..4).map(|_| ready(mountain())));
    let throes = corpus::card("Throes of Chaos").unwrap();
    scenario = if in_hand {
        scenario.hand(0, [throes, island()])
    } else {
        scenario.hand(0, [island()]).graveyard(0, [throes])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Throes of Chaos")?;
    let mut events = Vec::new();
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(_) => {
                let pick = first(driver.state(), Zone::Hand(pid(0)), "Island");
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            DecisionKind::PayMana(_) => {
                events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?);
                break;
            }
            other => panic!("unexpected question {other:?}"),
        }
    }
    let way = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.way),
        _ => None,
    });
    let expected = if in_hand {
        None
    } else {
        Some(CastWay::Retrace)
    };
    assert_eq!(way, Some(expected));
    events.extend(settle(&mut driver)?);
    assert_eq!(free_casts(&events), 1);
    let graveyard = Zone::Graveyard(pid(0));
    assert_eq!(count(driver.state(), graveyard, "Throes of Chaos"), 1);
    assert_eq!(count(driver.state(), graveyard, "Opt"), 1);
    assert_eq!(
        count(driver.state(), graveyard, "Island"),
        usize::from(!in_hand),
        "retrace discards the land"
    );
    assert_replays(&driver);
    Ok(())
}
