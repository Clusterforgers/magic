//! Goal: a creature card cast face down with Illusionary Mask reaches no seat but its controller's by name, in the event
//! batches of the activation and the cast, while every seat sees a 2/2; an instant card Magar of the Magic Strings puts
//! down shows every seat the listed 3/3 with no name, and only its controller sees the card behind it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::choose::{CardsChoice, NumberChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_protocol::{EventBatch, ObjectView, project_events};
use mtg_testkit::cards::{creature, instant, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

const HIDDEN: &str = "Secret Leviathan";
const SPELL: &str = "Noted Bolt";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text.contains(name),
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn object_of(batch: &EventBatch, id: ObjectId) -> Option<&ObjectView> {
    batch.objects.iter().find(|object| object.id == id)
}

/// Answers until the stack is empty and seat 0 has priority: X is `x`, the first card and target offered are taken,
/// mana is paid automatically and every yes-or-no is yes.
fn settle(driver: &mut Driver, x: u32) {
    for _ in 0..300 {
        let decision: Decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == seat(0) =>
            {
                return;
            }
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::ChooseCards(options) => {
                Answer::ChooseCards(CardsChoice(options.from.iter().copied().take(1).collect()))
            }
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options.legal.iter().copied().take(1).collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn face_down(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .unwrap()
}

fn table(seats: usize, scenario: Scenario) -> Scenario {
    (0..seats).fold(
        scenario.at(1, 0, mtg_engine::turn::Step::Main1),
        |scenario, other| scenario.library(other, vec![island(); 6]),
    )
}

#[rstest]
fn a_card_cast_with_illusionary_mask_is_named_to_its_controller_alone(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Illusionary Mask").unwrap()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ],
            )
            .hand(
                0,
                [creature(HIDDEN, 1, &[mtg_core::color::Color::Blue], 6, 6)],
            ),
    );
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Illusionary Mask").unwrap();
    settle(&mut driver, 3);
    let id = face_down(&driver);
    let events = driver.engine().events().to_vec();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees the permanent");
        assert!(object.face_down);
        assert_eq!((object.power, object.toughness), (Some(2), Some(2)));
        assert_eq!(batch_names(batch, HIDDEN), owner, "{viewer:?}");
    }
}

#[rstest]
fn magar_s_card_shows_every_seat_the_listed_3_3(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Magar of the Magic Strings").unwrap()),
                    ready(swamp()),
                    ready(mountain()),
                    ready(island()),
                ],
            )
            .graveyard(0, [instant(SPELL, 0, &[mtg_core::color::Color::Red])]),
    );
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .begin_activation("Magar of the Magic Strings")
        .unwrap();
    settle(&mut driver, 0);
    let id = face_down(&driver);
    let events = driver.engine().events().to_vec();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees the permanent");
        assert!(object.face_down);
        assert_eq!(object.types, [CardType::Creature]);
        assert_eq!((object.power, object.toughness), (Some(3), Some(3)));
        if owner {
            assert_eq!(object.name, SPELL);
        } else {
            assert_eq!(object.name, "");
        }
    }
}
