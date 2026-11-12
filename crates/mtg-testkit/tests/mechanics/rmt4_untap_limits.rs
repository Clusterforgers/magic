//! Goal: untap limits ("players can't untap more than one land during their untap steps", Winter Orb, Static Orb, Smoke, Mungha Wurm, Winter Moon) hold back the permanents past each limit at tables of two, three and four seats: the active player is asked which permanents untap and the limits keep the first ones chosen that they allow; no question while the limits let everything untap, while the Orb is tapped or when the limit names another player; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::PermanentState;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::c4_control::replays;
use crate::common::name_of;

/// Seat 0 has `mine` (tapped lands and creatures); the last seat, whose turn it is, has `theirs`. The game is at the
/// end of turn 1, so turn 2 is seat 0's.
fn table(seats: usize, mine: Vec<PresetPermanent>, theirs: Vec<PresetPermanent>) -> Scenario {
    Scenario::new(seats)
        .at(1, seats - 1, Step::End)
        .battlefield(0, mine)
        .battlefield(seats - 1, theirs)
}

fn tapped_lands(count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| tapped(forest())).collect()
}

fn bear(name: &str) -> Arc<CardDefinition> {
    free_creature(name, 2, 2)
}

/// Seat 0's permanents, in battlefield order.
fn seat_zero(driver: &Driver) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.controller().index() == 0)
        })
        .collect()
}

fn is_tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .is_some_and(PermanentState::tapped)
}

/// Plays to seat 0's untap step and answers its question, if one is asked, with the permanents named in order; then to
/// its upkeep.
fn untap_choosing(driver: &mut Driver, chosen: &[ObjectId]) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..40 {
        if driver.state().turn().number() == 2 && driver.state().turn().step() > Step::Untap {
            break;
        }
        let decision = driver.decision()?.clone();
        if driver.state().turn().number() == 2
            && driver.state().turn().step() == Step::Untap
            && matches!(decision.kind, DecisionKind::ChooseCards(_))
        {
            asked = true;
            driver.answer(Answer::ChooseCards(CardsChoice(chosen.to_vec())))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(asked)
}

#[rstest]
#[case::winter_orb_one_land("Winter Orb", 3, 1)]
#[case::static_orb_two_permanents("Static Orb", 3, 2)]
#[case::winter_orb_with_one_tapped_land("Winter Orb", 1, 1)]
fn the_limit_untaps_the_first_lands_chosen(
    #[case] limiter: &str,
    #[case] lands: usize,
    #[case] untapped: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        tapped_lands(lands),
        vec![ready(card(limiter).unwrap())],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let mine = seat_zero(&driver);
    let mut chosen = mine.clone();
    chosen.reverse();
    let asked = untap_choosing(&mut driver, &chosen)?;
    assert_eq!(
        asked,
        lands > untapped,
        "a question only when a land is held back"
    );
    let up: Vec<ObjectId> = mine
        .iter()
        .copied()
        .filter(|id| !is_tapped(&driver, *id))
        .collect();
    assert_eq!(up.len(), untapped);
    if asked {
        assert_eq!(
            up,
            chosen[..untapped].iter().rev().copied().collect::<Vec<_>>()
        );
    }
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_tapped_winter_orb_limits_nothing("Winter Orb", true)]
#[case::mungha_wurm_limits_only_its_controller("Mungha Wurm", false)]
fn no_limit_untaps_everything(
    #[case] limiter: &str,
    #[case] limiter_tapped: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let permanent = if limiter_tapped {
        tapped(card(limiter).unwrap())
    } else {
        ready(card(limiter).unwrap())
    };
    let mut driver = Driver::from_scenario(table(seats, tapped_lands(3), vec![permanent]))?;
    let mine = seat_zero(&driver);
    assert!(!untap_choosing(&mut driver, &[])?);
    assert!(mine.iter().all(|id| !is_tapped(&driver, *id)));
    replays(&driver);
    Ok(())
}

#[rstest]
fn two_limits_each_hold_their_own_permanents(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = tapped_lands(2);
    mine.push(tapped(bear("Bear A")));
    mine.push(tapped(bear("Bear B")));
    let theirs = vec![
        ready(card("Winter Orb").unwrap()),
        ready(card("Smoke").unwrap()),
    ];
    let mut driver = Driver::from_scenario(table(seats, mine, theirs))?;
    let ids = seat_zero(&driver);
    // Both Bears first, then both lands: one of each kind untaps, the first chosen.
    let chosen = vec![ids[2], ids[3], ids[0], ids[1]];
    assert!(untap_choosing(&mut driver, &chosen)?);
    let up: Vec<String> = ids
        .iter()
        .filter(|id| !is_tapped(&driver, **id))
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert_eq!(up, vec!["Forest".to_owned(), "Bear A".to_owned()]);
    assert!(!is_tapped(&driver, ids[0]));
    replays(&driver);
    Ok(())
}
