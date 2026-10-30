//! Goal: `CommanderDamageView.name` names the commander that dealt the damage to every seat wherever the commander is (battlefield, command zone, a hand, a library), while the hand or library it is in stays hidden from the seats that may not see it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_protocol::StateView;
use mtg_testkit::cards::free_creature;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::hidden_info::{hidden_in, leaks};

/// Seat 0's commander "Cmd" has dealt 7 combat damage to seat 2, then went to `to` (none: it stays on the battlefield).
fn after_damage(to: Option<ManualZone>) -> Driver {
    let mut scenario = Scenario::new(3)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 0, &[], 7, 7)]);
    // Every card has a name of its own, so the scan for hidden names finds only what leaks.
    for seat in 0..3 {
        let cards = (0..12).map(|n| free_creature(&format!("Secret {seat}.{n}"), 1, 1));
        scenario = scenario.library(seat, cards);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Cmd").unwrap();
    for _ in 0..3 {
        driver.pass().unwrap();
    }
    driver.idle_until(4, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Cmd", 2)]).unwrap();
    driver.idle_until(4, Step::EndCombat).unwrap();
    if let Some(to) = to {
        let object = driver.state().commanders().next().unwrap();
        driver.manual(ManualAction::Move { object, to }).unwrap();
        // The owner keeps it in the hand or library instead of moving it to the command zone (CR 903.9a).
        if to != ManualZone::Command {
            driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
        }
    }
    driver
}

#[rstest]
#[case::battlefield(None, true)]
#[case::command_zone(Some(ManualZone::Command), true)]
#[case::hand(Some(ManualZone::Hand), false)]
#[case::library(Some(ManualZone::LibraryTop), false)]
fn every_seat_names_the_commander_wherever_it_is(
    #[case] to: Option<ManualZone>,
    #[case] public: bool,
    #[values(0, 1, 2)] viewer: usize,
) {
    let driver = after_damage(to);
    let id = driver.state().commanders().next().unwrap();
    let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
    let damage = &view.table.players[2].commander_damage;
    assert_eq!(damage.len(), 1);
    assert_eq!(
        (
            damage[0].commander,
            damage[0].name.as_str(),
            damage[0].damage
        ),
        (id, "Cmd", 7)
    );
    // The zone it is in keeps its own rules: a hand only its owner sees, a library no seat.
    let seen = view.objects.iter().any(|object| object.id == id);
    let may_see = public || (to == Some(ManualZone::Hand) && viewer == 0);
    assert_eq!(seen, may_see, "seat {viewer} identifies the card");
    if !public && viewer != 0 {
        assert_eq!(view.table.players[0].hand.cards, None);
    }
}

#[rstest]
fn the_name_leaks_no_other_card_of_the_hand_or_library(
    #[values(ManualZone::Hand, ManualZone::LibraryTop)] to: ManualZone,
    #[values(1, 2)] viewer: usize,
) {
    let driver = after_damage(Some(to));
    let viewer = PlayerId::from_index(viewer);
    let hidden = hidden_in(driver.state(), viewer);
    assert!(!hidden.is_empty(), "seat 0's library stays hidden");
    let view = serde_json::to_value(StateView::for_player(driver.state(), viewer)).unwrap();
    assert_eq!(leaks(&view, &hidden), Vec::<String>::new());
}

#[test]
fn the_name_is_in_the_json_beside_the_damage() {
    let driver = after_damage(Some(ManualZone::Hand));
    let view = StateView::for_player(driver.state(), PlayerId(1));
    let json = serde_json::to_value(&view.table.players[2]).unwrap();
    assert_eq!(json["commander_damage"][0]["name"], "Cmd");
    assert_eq!(json["commander_damage"][0]["damage"], 7);
}
