//! Goal: cards cloaked or manifested from a hand play at two to four seats: Vannifar's first mode cloaks the card its
//! controller chooses from their hand (face down, ward {2}) and its second puts a +1/+1 counter on each colorless creature
//! they control only; Kozilek's cast trigger has each target player manifest two cards of their own hand (what they hold
//! when they hold fewer) and its caster draws one card for each card manifested.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack is empty: `modes` for a choice of modes, `targets` for a choice of targets, the passive answer
/// to anything else.
fn r4710_settle(
    driver: &mut Driver,
    modes: &[usize],
    targets: &[usize],
) -> Result<(), ScriptError> {
    for _ in 0..120 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseMode(_) => {
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.to_vec())))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let chosen: Vec<_> = targets
                    .iter()
                    .map(|seat| driver.target_player(*seat))
                    .collect();
                driver.choose_targets(&chosen)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

/// The face-down permanents `seat` controls, with how each was put down.
fn r4710_face_down(state: &GameState, seat: usize) -> Vec<(ObjectId, FaceDown)> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(PlayerId::from_index(seat)))
        .filter_map(|id| Some((*id, state.object(*id)?.face_down()?)))
        .collect()
}

/// Seat 0 controls Vannifar and a colorless 1/1 and holds a Grizzly Bears; at the beginning of combat it chooses `mode`.
#[rstest]
#[case::cloak_two_seats(2, 0)]
#[case::cloak_four_seats(4, 0)]
#[case::counters_three_seats(3, 1)]
#[case::counters_four_seats(4, 1)]
fn r4710_vannifar_cloaks_from_hand_or_grows_colorless(
    #[case] seats: usize,
    #[case] mode: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Vannifar, Evolved Enigma").unwrap()),
                ready(free_creature("Golem", 1, 1)),
            ],
        )
        .hand(0, [card("Grizzly Bears").unwrap()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    r4710_settle(&mut driver, &[mode], &[])?;
    let state = driver.state();
    let hand = state.zone(Zone::Hand(PlayerId::from_index(0))).len();
    let counters = |name: &str| {
        let id = driver.id_of(name).unwrap();
        state
            .object(id)
            .and_then(|o| o.permanent())
            .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
    };
    if mode == 0 {
        assert_eq!(hand, 0);
        assert_eq!(
            r4710_face_down(state, 0)
                .iter()
                .map(|(_, how)| *how)
                .collect::<Vec<_>>(),
            [FaceDown::Cloak]
        );
        assert_eq!(counters("Golem"), 0);
    } else {
        assert_eq!(hand, 1);
        assert!(r4710_face_down(state, 0).is_empty());
        assert_eq!(counters("Golem"), 1);
        assert_eq!(counters("Vannifar, Evolved Enigma"), 0);
    }
    Ok(())
}

/// Seat 0 casts Kozilek with nine Wastes, targeting `targets`; every other seat holds three Grizzly Bears and seat 0 one
/// besides Kozilek. Each target then controls `manifested[i]` face-down permanents and seat 0 has drawn their total.
#[rstest]
#[case::one_opponent(2, &[1], &[2])]
#[case::two_opponents(3, &[1, 2], &[2, 2])]
#[case::the_caster_holds_one(4, &[0, 3], &[1, 2])]
#[case::nobody(4, &[], &[])]
fn r4710_kozilek_manifests_from_each_target_hand(
    #[case] seats: usize,
    #[case] targets: &[usize],
    #[case] manifested: &[usize],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, (0..9).map(|_| ready(wastes())))
        .hand(0, [card("Kozilek, the Broken Reality").unwrap(), bear()])
        .library(0, (0..10).map(|_| bear()))
        .at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.hand(seat, [bear(), bear(), bear()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Kozilek, the Broken Reality")?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r4710_settle(&mut driver, &[], targets)?;
    let state = driver.state();
    for (seat, count) in targets.iter().zip(manifested) {
        let down = r4710_face_down(state, *seat);
        assert_eq!(down.len(), *count, "seat {seat}");
        assert!(down.iter().all(|(_, how)| *how == FaceDown::Manifest));
    }
    let drawn: usize = manifested.iter().sum();
    let held = usize::from(!targets.contains(&0));
    assert_eq!(
        state.zone(Zone::Hand(PlayerId::from_index(0))).len(),
        held + drawn
    );
    Ok(())
}
