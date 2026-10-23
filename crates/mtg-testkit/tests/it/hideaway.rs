//! Goal: hideaway plays whole at tables of two, three and four seats: the permanent that enters hides one of the top four cards face down (fewer when the library is short, none from an empty one) and puts the rest on the bottom; the hidden card is seen by its owner alone, stays linked to the permanent, is played or cast free only while the condition of the permanent holds (a land only in a main phase with a land play left), and the link is lost when the permanent leaves.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::FaceDown;
use mtg_engine::resolve::Scope;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A library of `pads` plain cards around the cards given, which sit in the middle where both ends of the library reach them.
pub(crate) fn library(extra: Vec<Arc<CardDefinition>>, pads: usize) -> Vec<Arc<CardDefinition>> {
    let pad = |i: usize| free_creature(&format!("Pad {i}"), 1, 1);
    let mut cards = vec![pad(0), pad(1)];
    cards.extend(extra);
    cards.extend((0..pads).map(|i| pad(i + 2)));
    cards
}

/// What the players were asked while the stack emptied, and how they answered the yes-or-no questions.
pub(crate) fn settle(
    driver: &mut Driver,
    hide: &str,
    yes: bool,
) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::ChooseCards(options) => {
                assert_eq!(decision.player, pid(0), "the controller chooses");
                let pick = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| name_of(driver.state(), *id) == hide)
                    .unwrap_or(options.from[0]);
                Answer::ChooseCards(CardsChoice(vec![pick]))
            }
            DecisionKind::YesNo(options) => {
                asked.push(options.text.clone());
                Answer::YesNo(YesNoChoice(yes))
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
                continue;
            }
            other => panic!("unexpected question: {}", other.name()),
        };
        driver.answer(answer)?;
    }
    panic!("the stack did not empty");
}

/// Seat 0 holds Mosswort Bridge and a Forest, and has a Forest and two creatures of `powers` on the battlefield.
fn bridge_table(
    seats: usize,
    library: &[Arc<CardDefinition>],
    powers: [i32; 2],
) -> Result<Driver, ScriptError> {
    let mut table = lands(1, forest);
    table.extend(powers.map(|power| ready(free_creature(&format!("Giant {power}"), power, power))));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, ["Mosswort Bridge", "Forest"])
            .battlefield(0, table),
        |scenario, seat| {
            scenario.library(
                seat,
                if seat == 0 {
                    library.to_vec()
                } else {
                    self::library(vec![], 4)
                },
            )
        },
    );
    Driver::from_scenario(scenario)
}

fn exiled_named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Exile)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == name)
        .collect()
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_permanent_hides_one_of_the_top_four_face_down_and_the_others_go_to_the_bottom(
    #[case] seats: usize,
) {
    let mut driver = bridge_table(
        seats,
        &library(
            vec![free_creature("Hidden", 2, 2), forest(), forest(), forest()],
            0,
        ),
        [5, 5],
    )
    .unwrap();
    let before = driver.state().zone(Zone::Library(pid(0))).to_vec();
    let top: Vec<ObjectId> = before.iter().rev().take(4).copied().collect();
    let untouched: Vec<ObjectId> = before[..before.len() - 4].to_vec();
    driver.play_land("Mosswort Bridge").unwrap();
    settle(&mut driver, "Hidden", true).unwrap();
    let state = driver.state();
    let bridge = state
        .object(driver.id_of("Mosswort Bridge").unwrap())
        .unwrap();
    let hidden = exiled_named(&driver, "Hidden");
    assert_eq!(hidden.len(), 1, "one card is hidden");
    assert!(top.contains(&hidden[0]));
    let card = state.object(hidden[0]).unwrap();
    assert_eq!(
        card.face_down(),
        Some(FaceDown::Hideaway {
            by: bridge.reference()
        })
    );
    for seat in 0..seats {
        assert_eq!(
            state.may_look_at(pid(seat), hidden[0]),
            seat == 0,
            "seat {seat}"
        );
    }
    let after = state.zone(Zone::Library(pid(0))).to_vec();
    assert_eq!(after.len(), before.len() - 1);
    assert_eq!(
        after[after.len() - untouched.len()..],
        untouched[..],
        "the rest of the library is on top"
    );
    let mut bottom = after[..3].to_vec();
    let mut others: Vec<ObjectId> = top.into_iter().filter(|id| *id != hidden[0]).collect();
    bottom.sort();
    others.sort();
    assert_eq!(bottom, others, "the other three are on the bottom");
}

#[rstest]
#[case::empty(0, 0)]
#[case::one_card(1, 1)]
#[case::two_cards(2, 1)]
#[case::three_cards(3, 1)]
fn a_short_library_is_looked_at_whole_and_an_empty_one_hides_nothing(
    #[case] cards: usize,
    #[case] hidden: usize,
) {
    let pads: Vec<_> = (0..cards)
        .map(|i| free_creature(&format!("Short {i}"), 1, 1))
        .collect();
    let mut driver = bridge_table(2, &pads, [5, 5]).unwrap();
    driver.play_land("Mosswort Bridge").unwrap();
    settle(&mut driver, "none", true).unwrap();
    let state = driver.state();
    let in_exile = state
        .zone(Zone::Exile)
        .iter()
        .filter(|id| state.object(**id).unwrap().face_down().is_some())
        .count();
    assert_eq!(in_exile, hidden);
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), cards - hidden);
}

/// How the played card turns out: whether the question to play it was asked and where the card is.
#[derive(Debug, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) asked: usize,
    pub(crate) on_battlefield: bool,
}

/// The Bridge hides the card named "Hidden" on turn 1; on seat 0's next turn they hold creatures of `powers`, have played
/// a land first when `land_first`, and activate it.
fn activate(
    seats: usize,
    hidden: Arc<CardDefinition>,
    powers: [i32; 2],
    land_first: bool,
) -> Outcome {
    let name = hidden.name.clone();
    let mut driver = bridge_table(seats, &library(vec![hidden], 3), powers).unwrap();
    driver.play_land("Mosswort Bridge").unwrap();
    settle(&mut driver, &name, true).unwrap();
    let again = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(again, Step::Main1).unwrap();
    assert_eq!(exiled_named(&driver, &name).len(), 1, "still hidden");
    assert!(
        driver
            .state()
            .may_look_at(pid(0), exiled_named(&driver, &name)[0])
    );
    if land_first {
        driver.play_land("Forest").unwrap();
    }
    driver.begin_activation("Mosswort Bridge").unwrap();
    pay(&mut driver).unwrap();
    let asked = settle(&mut driver, &name, true).unwrap().len();
    let on_battlefield = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == name);
    Outcome {
        asked,
        on_battlefield,
    }
}

pub(crate) fn spell() -> Arc<CardDefinition> {
    free_creature("Hidden", 2, 2)
}

#[rstest]
#[case::a_land_with_enough_power_two_seats(2, false, [5, 5], false, 1, true)]
#[case::a_land_with_enough_power_three_seats(3, false, [6, 4], false, 1, true)]
#[case::a_land_with_enough_power_four_seats(4, false, [5, 5], false, 1, true)]
#[case::a_land_with_too_little_power(2, false, [5, 4], false, 0, false)]
#[case::a_land_when_a_land_was_played_already(2, false, [5, 5], true, 0, false)]
#[case::a_spell_with_enough_power(2, true, [5, 5], false, 1, true)]
#[case::a_spell_with_enough_power_after_a_land(3, true, [5, 5], true, 1, true)]
#[case::a_spell_with_too_little_power(4, true, [5, 4], false, 0, false)]
fn the_hidden_card_is_played_or_cast_free_only_while_the_condition_holds(
    #[case] seats: usize,
    #[case] cast: bool,
    #[case] powers: [i32; 2],
    #[case] land_first: bool,
    #[case] asked: usize,
    #[case] on_battlefield: bool,
) {
    let hidden = if cast {
        spell()
    } else {
        forest_named("Hidden")
    };
    let outcome = activate(seats, hidden, powers, land_first);
    assert_eq!(
        outcome,
        Outcome {
            asked,
            on_battlefield
        }
    );
}

fn forest_named(name: &str) -> Arc<CardDefinition> {
    let mut land = (*forest()).clone();
    name.clone_into(&mut land.name);
    Arc::new(land)
}

#[test]
fn the_link_is_lost_when_the_permanent_leaves_and_the_card_stays_hidden() {
    let mut driver =
        bridge_table(2, &library(vec![free_creature("Hidden", 2, 2)], 3), [5, 5]).unwrap();
    driver.play_land("Mosswort Bridge").unwrap();
    settle(&mut driver, "Hidden", true).unwrap();
    let bridge = driver.id_of("Mosswort Bridge").unwrap();
    let old = driver.state().object(bridge).unwrap().reference();
    let hidden = exiled_named(&driver, "Hidden")[0];
    let mut state = driver.state().clone();
    let gone = Event::ZoneChanged {
        object: bridge,
        from: Zone::Battlefield,
        to: Zone::Graveyard(pid(0)),
        cause: ZoneCause::Destroyed,
    };
    apply_event(&mut state, &gone).unwrap();
    let new = state.object(bridge).unwrap().reference();
    let linked: Selector =
        serde_json::from_str(r#"[{"In":"Exile"},{"ExiledWith":"This"}]"#).unwrap();
    let found = |source| {
        Scope::announcing(&state, pid(0), source)
            .matching(&linked)
            .unwrap()
            .len()
    };
    assert_eq!(found(old), 1, "the permanent as it was still names it");
    assert_eq!(
        found(new),
        0,
        "a permanent that left is a new object with no link"
    );
    assert!(matches!(
        state.object(hidden).unwrap().face_down(),
        Some(FaceDown::Hideaway { .. })
    ));
    assert!(
        state.may_look_at(pid(0), hidden),
        "its owner may still look"
    );
    assert!(!state.may_look_at(pid(1), hidden));
}
