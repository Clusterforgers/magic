//! Goal: the yes-or-no questions about a commander say what they ask, at two to four seats: the return to the command zone (from a graveyard, or instead of a hand) is marked as that choice, while the commander's own "you may" (a land entering under Aesi-like "you may draw a card") and the Partner with question of a commander cast from the command zone (Zndrsplt fetching Okaun) are plain questions in their own words; and Zndrsplt's and Okaun's beginning-of-combat flips happen in the beginning of combat step of their controller's turn and in no other step or turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice, YesNoChoice, YesNoOptions};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

const ZNDRSPLT: &str = "Zndrsplt, Eye of Wisdom";
const OKAUN: &str = "Okaun, Eye of Chaos";

/// A legend with Aesi, Tyrant of Gyre Strait's trigger: "Whenever a land you control enters, you may draw a card."
fn aesi() -> Arc<CardDefinition> {
    let rule = r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Land"},{"ControlledBy":"You"}]}},"effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#;
    let card = legend("Aesi", 1, &[Color::Green], 2, 2).as_ref().clone();
    Arc::new(card.with_rules(serde_json::from_str(rule).unwrap()))
}

/// Seat 0 has `commander` in its command zone, Forests and Islands to cast it, a Forest in hand and Okaun in its
/// library; the other seats have libraries to draw from.
fn table(seats: usize, commander: Arc<CardDefinition>) -> Driver {
    let mut lands: Vec<_> = (0..5).map(|_| ready(island())).collect();
    lands.extend((0..2).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .battlefield(0, lands)
        .hand(0, [forest()])
        .library(0, [island(), island(), card(OKAUN).unwrap(), island()]);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..4).map(|_| island()).collect::<Vec<_>>());
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Answers passively until a yes-or-no question is pending, choosing seat 0 for any target, and returns it.
fn next_yes_no(driver: &mut Driver) -> YesNoOptions {
    for _ in 0..60 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::YesNo(options) => return options,
            DecisionKind::ChooseTargets(_) => {
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
                        Target::Player(pid(0)),
                    ])))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("no yes-or-no question came");
}

/// Casts the commander and lets it resolve, answering no to any question on the way.
fn cast_aesi(driver: &mut Driver) {
    driver.cast_tapping("Aesi", &["Forest", "Island"]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.answer_passively().unwrap();
    }
}

#[derive(Debug, Clone, Copy)]
enum Asked {
    /// The commander is put into a graveyard: the state-based question.
    Died,
    /// The commander would go to its owner's hand: the replacement question.
    Bounced,
    /// A land enters under the commander's "you may draw a card".
    OwnMay,
    /// The Partner with trigger of Zndrsplt cast from the command zone.
    PartnerWith,
}

#[rstest]
#[case::died(Asked::Died, true, "Put Aesi into the command zone?")]
#[case::bounced(
    Asked::Bounced,
    true,
    "Put Aesi into the command zone instead of a hand?"
)]
#[case::own_may(Asked::OwnMay, false, "Draw a card?")]
#[case::partner_with(
    Asked::PartnerWith,
    false,
    "Put Okaun, Eye of Chaos into your hand from your library?"
)]
fn a_commander_question_says_what_it_asks(
    #[values(2, 3, 4)] seats: usize,
    #[case] asked: Asked,
    #[case] commander_return: bool,
    #[case] text: &str,
) {
    let commander = if matches!(asked, Asked::PartnerWith) {
        card(ZNDRSPLT).unwrap()
    } else {
        aesi()
    };
    let mut driver = table(seats, commander);
    match asked {
        Asked::Died | Asked::Bounced => {
            cast_aesi(&mut driver);
            let to = if matches!(asked, Asked::Died) {
                ManualZone::Graveyard
            } else {
                ManualZone::Hand
            };
            let object = driver.id_of("Aesi").unwrap();
            driver.manual(ManualAction::Move { object, to }).unwrap();
        }
        Asked::OwnMay => {
            cast_aesi(&mut driver);
            driver.play_land("Forest").unwrap();
        }
        Asked::PartnerWith => {
            driver.cast_tapping(ZNDRSPLT, &["Island"; 5]).unwrap();
        }
    }
    let options = next_yes_no(&mut driver);
    assert_eq!(
        (options.commander_return, options.text.as_str()),
        (commander_return, text)
    );
    if matches!(asked, Asked::PartnerWith) {
        driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
        for _ in 0..20 {
            if let DecisionKind::ChooseCards(options) = driver.decision().unwrap().kind.clone() {
                driver
                    .answer(Answer::ChooseCards(CardsChoice(options.from)))
                    .unwrap();
                break;
            }
            driver.answer_passively().unwrap();
        }
        let state = driver.state();
        let in_hand = state.zone(Zone::Hand(pid(0))).iter();
        assert!(
            in_hand
                .map(|id| state.object(*id).unwrap())
                .any(|object| object.card().name == OKAUN),
            "a yes puts Okaun into the hand"
        );
    }
}

/// Zndrsplt and Okaun on the battlefield of `seat` flip only at the beginning of combat of that seat's own turns, over
/// two whole rounds of the table.
#[rstest]
fn beginning_of_combat_flips_happen_at_the_beginning_of_combat_of_their_controllers_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] seat: usize,
) {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .battlefield(
            seat,
            [ready(card(ZNDRSPLT).unwrap()), ready(card(OKAUN).unwrap())],
        );
    for each in 0..seats {
        scenario = scenario.library(each, (0..40).map(|_| island()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let rounds = u32::try_from(2 * seats).unwrap();
    let mut flipped_in = Vec::new();
    while driver.state().turn().number() <= rounds {
        let at = (
            driver.state().turn().number(),
            driver.state().active_player(),
            driver.state().turn().step(),
        );
        let events = driver.answer_passively().unwrap();
        if events
            .iter()
            .any(|event| matches!(event, Event::CoinFlipped { .. }))
        {
            flipped_in.push(at);
        }
    }
    let turns: Vec<u32> = (1..=rounds)
        .filter(|turn| (turn - 1) as usize % seats == seat)
        .collect();
    for (turn, active, step) in &flipped_in {
        assert_eq!(
            (*active, *step),
            (Some(pid(seat)), Step::BeginCombat),
            "turn {turn}"
        );
    }
    let mut seen: Vec<u32> = flipped_in.iter().map(|(turn, _, _)| *turn).collect();
    seen.dedup();
    assert_eq!(seen, turns, "each of the controller's turns flips");
}
