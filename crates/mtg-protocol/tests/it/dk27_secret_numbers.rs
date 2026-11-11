//! Goal: secret numbers tell the table what they should and no more, at two to four seats: each number is told to the
//! seat that chose it and to nobody else while the others choose, and the finished step then shows every seat each
//! player's number.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Choice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::cards::{cost, island};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// "Each player secretly chooses a number 0 or greater" alone, for no mana.
fn numbers() -> Arc<CardDefinition> {
    let spell = r#"{"Spell":{"effects":[{"ChooseNumbers":{"secret":true,"as":"number","highest":"highest","lowest":"lowest"}}]}}"#;
    Arc::new(
        CardDefinition::new("Numbers")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(spell).unwrap()),
    )
}

/// Casts the card from seat 0; seat n chooses 10 + n. Returns what each seat is told of the log.
fn told(seats: usize) -> Vec<Vec<EventView>> {
    let scenario = Scenario::new(seats)
        .hand(0, [numbers()])
        .library(0, (0..5).map(|_| island()));
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Numbers").unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::ChooseNumber(_) => {
                let number = 10 + u32::try_from(decision.player.index()).unwrap();
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(number)))
                    .unwrap();
            }
            _ => panic!("a number is asked, not {}", decision.kind.name()),
        }
    }
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    project_events(&before, &events, &viewers, 0)
        .unwrap()
        .into_iter()
        .map(|batch| batch.events)
        .collect()
}

#[rstest]
fn a_secret_number_is_told_only_to_its_seat_then_all_are_shown(#[values(2, 3, 4)] seats: usize) {
    for (viewer, views) in told(seats).iter().enumerate() {
        let choices: Vec<(PlayerId, Option<Choice>)> = views
            .iter()
            .filter_map(|view| match view {
                EventView::Public(Event::ChoiceMade { player, choice }) => {
                    Some((*player, Some(choice.clone())))
                }
                EventView::Hidden(HiddenEvent::ChoiceMade { player }) => Some((*player, None)),
                _ => None,
            })
            .collect();
        let want: Vec<(PlayerId, Option<Choice>)> = (0..seats)
            .map(|n| {
                let own =
                    (n == viewer).then(|| Choice::SecretNumber(10 + u32::try_from(n).unwrap()));
                (seat(n), own)
            })
            .collect();
        assert_eq!(choices, want, "seat {viewer} knows only its own number");
        let shown = views
            .iter()
            .find_map(|view| match view {
                EventView::Public(Event::StepDone { bound, .. }) => Some(bound.clone()),
                _ => None,
            })
            .expect("the finished step is told");
        for n in 0..seats {
            let name = format!("~value:number.{n}\t{{\"Number\":{}}}", 10 + n);
            assert!(
                shown.iter().any(|(bound, _)| *bound == name),
                "seat {viewer} reads the number of seat {n}: {shown:?}"
            );
        }
    }
}
