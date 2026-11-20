//! Goal: "The top card of your library has plot. The plot cost is equal to its mana cost." with "You may plot nonland
//! cards from the top of your library." (CR 702.170a) offers the top card of its controller's library alone, a nonland
//! one, for its mana cost at sorcery timing, and plotting it exiles it face up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, keyworded_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A creature for {1}{G} without plot.
fn bear() -> Arc<CardDefinition> {
    keyworded_creature("Top Bear", 1, &[Color::Green], 2, 2, &[])
}

/// Fblthp for `owner` among `seats` players, seat 0 with three Forests and `top` on top of its library, in its main phase.
fn game(seats: usize, owner: usize, top: &Arc<CardDefinition>) -> Result<Driver, ScriptError> {
    let fblthp = ready(card("Fblthp, Lost on the Range").unwrap());
    let mut own = lands(3, forest);
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    if owner == 0 {
        own.push(fblthp);
    } else {
        scenario = scenario.battlefield(owner, [fblthp]);
    }
    scenario = scenario.battlefield(0, own);
    for seat in 0..seats {
        let mut library = vec![forest(); 10];
        if seat == 0 {
            library.insert(0, top.clone());
        }
        scenario = scenario.library(seat, library);
    }
    Driver::from_scenario(scenario)
}

fn plottable(driver: &Driver) -> Vec<ObjectId> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options.plottable.clone()
}

fn top(driver: &Driver) -> ObjectId {
    *driver.state().zone(Zone::Library(pid(0))).last().unwrap()
}

#[rstest]
fn r111_the_top_card_is_plotted_for_its_mana_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 0, &bear())?;
    let id = top(&driver);
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), id)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Plot(_))),
        "{:?}",
        mtg_engine::keywords::keywords_of(driver.state(), id)
    );
    assert_eq!(plottable(&driver), vec![id]);
    driver.answer(Answer::Priority(PriorityAction::Plot(id)))?;
    driver.pay_tapping(&["Forest", "Forest"], 0)?;
    assert_eq!(
        driver
            .state()
            .object(id)
            .map(mtg_engine::object::GameObject::zone),
        Some(Zone::Exile)
    );
    Ok(())
}

#[rstest]
fn r111_a_land_on_top_or_another_players_fblthp_offers_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] land_on_top: bool,
) -> Result<(), ScriptError> {
    let (owner, top) = if land_on_top {
        (0, forest())
    } else {
        (seats - 1, bear())
    };
    let driver = game(seats, owner, &top)?;
    assert_eq!(plottable(&driver), Vec::<ObjectId>::new());
    Ok(())
}
