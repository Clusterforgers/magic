//! Goal: the hideaway lands that read what happened this turn play the hidden card exactly when their condition holds, at tables of two, three and four seats: Windbrisk Heights after three or more attackers, Spinerock Knoll once an opponent was dealt 7 or more damage; the tallies count the creatures a player attacked with and the damage dealt to a player, are read from the point of view of each seat, and start again with the next turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, pid};
use crate::hideaway::{Outcome, library, settle, spell};
use mtg_cards::CardDefinition;
use mtg_cards::value::Condition;
use mtg_engine::resolve::Scope;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A land that hides a card and the basic land that pays for its activation.
type Land = (&'static str, fn() -> Arc<CardDefinition>);

const WINDBRISK: Land = ("Windbrisk Heights", plains);
const SPINEROCK: Land = ("Spinerock Knoll", mountain);

/// Seat 0 holds the land `name` and has the land `mana` and `giants` creatures of `power` on the battlefield.
fn combat_table(
    seats: usize,
    (name, mana): Land,
    (giants, power): (usize, i32),
) -> Result<Driver, ScriptError> {
    let mut table = lands(1, mana);
    table.extend((0..giants).map(|i| ready(free_creature(&format!("Giant {i}"), power, power))));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, [name])
            .battlefield(0, table),
        |scenario, seat| {
            let cards = if seat == 0 {
                library(vec![spell()], 3)
            } else {
                library(vec![], 4)
            };
            scenario.library(seat, cards)
        },
    );
    Driver::from_scenario(scenario)
}

/// The land hides the card on turn 1; on seat 0's next turn the creatures attack seat 1 and the land is activated in the
/// step of the attack, or in the second main phase when `after_damage`.
fn attack_and_activate(
    seats: usize,
    land: Land,
    giants: (usize, i32),
    after_damage: bool,
) -> Outcome {
    let name = land.0;
    let mut driver = combat_table(seats, land, giants).unwrap();
    driver.play_land(name).unwrap();
    settle(&mut driver, "Hidden", true).unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let names: Vec<String> = (0..giants.0).map(|i| format!("Giant {i}")).collect();
    let attacks: Vec<(&str, usize)> = names.iter().map(|n| (n.as_str(), 1)).collect();
    driver.attack(&attacks).unwrap();
    if after_damage {
        driver.idle_until(turn, Step::Main2).unwrap();
    }
    driver.begin_activation(name).unwrap();
    pay(&mut driver).unwrap();
    let asked = settle(&mut driver, "Hidden", true).unwrap().len();
    let on_battlefield = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Hidden");
    Outcome {
        asked,
        on_battlefield,
    }
}

#[rstest]
#[case::three_attackers_two_seats(2, (3, 1), true)]
#[case::three_attackers_three_seats(3, (3, 1), true)]
#[case::four_attackers_four_seats(4, (4, 2), true)]
#[case::two_attackers_two_seats(2, (2, 5), false)]
#[case::two_attackers_three_seats(3, (2, 5), false)]
#[case::one_attacker_four_seats(4, (1, 9), false)]
fn windbrisk_heights_plays_the_card_when_three_creatures_attacked(
    #[case] seats: usize,
    #[case] giants: (usize, i32),
    #[case] played: bool,
) {
    let outcome = attack_and_activate(seats, WINDBRISK, giants, false);
    assert_eq!(
        outcome,
        Outcome {
            asked: usize::from(played),
            on_battlefield: played
        }
    );
}

#[rstest]
#[case::seven_damage_two_seats(2, (7, 1), true, true)]
#[case::eight_damage_three_seats(3, (2, 4), true, true)]
#[case::seven_damage_four_seats(4, (1, 7), true, true)]
#[case::six_damage_two_seats(2, (2, 3), true, false)]
#[case::six_damage_three_seats(3, (1, 6), true, false)]
#[case::the_damage_is_not_dealt_yet_four_seats(4, (3, 7), false, false)]
fn spinerock_knoll_plays_the_card_when_an_opponent_was_dealt_seven_damage(
    #[case] seats: usize,
    #[case] giants: (usize, i32),
    #[case] after_damage: bool,
    #[case] played: bool,
) {
    let outcome = attack_and_activate(seats, SPINEROCK, giants, after_damage);
    assert_eq!(
        outcome,
        Outcome {
            asked: usize::from(played),
            on_battlefield: played
        }
    );
}

const ATTACKED: &str =
    r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"Attackers"}},"op":"Ge","right":3}}"#;
const HURT: &str = r#"{"Compare":{"left":{"Tallied":{"who":"Opponents","what":"DamageTaken"}},"op":"Ge","right":9}}"#;

fn holds(driver: &Driver, seat: usize, json: &str) -> bool {
    let condition: Condition = serde_json::from_str(json).unwrap();
    let giant = driver
        .state()
        .object(driver.id_of("Giant 0").unwrap())
        .unwrap()
        .reference();
    Scope::announcing(driver.state(), pid(seat), giant)
        .holds(&condition)
        .unwrap()
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn the_tallies_are_read_by_each_seat_and_start_again_with_the_next_turn(#[case] seats: usize) {
    let mut driver = combat_table(seats, WINDBRISK, (3, 3)).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Giant 0", 1), ("Giant 1", 1), ("Giant 2", 1)])
        .unwrap();
    assert!(holds(&driver, 0, ATTACKED));
    assert!(!holds(&driver, 1, ATTACKED), "only seat 0 attacked");
    driver.idle_until(1, Step::Main2).unwrap();
    assert!(holds(&driver, 0, HURT), "seat 1 was dealt 9 damage");
    assert!(!holds(&driver, 1, HURT), "seat 0 was dealt none");
    for seat in 2..seats {
        assert!(
            holds(&driver, seat, HURT),
            "seat 1 is an opponent of {seat}"
        );
    }
    driver.idle_until(2, Step::Main1).unwrap();
    for seat in 0..seats {
        assert!(!holds(&driver, seat, ATTACKED), "seat {seat}");
        assert!(!holds(&driver, seat, HURT), "seat {seat}");
    }
}
