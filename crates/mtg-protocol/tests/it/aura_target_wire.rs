//! Goal: a seat casting an Aura it holds face down in exile (a card Gonti's trigger hid) is sent the target decision for its host before the payment, sees the card it casts, and its answer read from the wire puts the spell on the stack with that target, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Decision, Event};
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage, StateView};
use mtg_testkit::attach_cards::canny_thief;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::json;

#[rstest]
fn the_target_decision_of_an_aura_cast_from_hidden_exile_is_sent(
    #[values(2, 3, 4)] seats: usize,
    #[values("Control Magic", "Pacifism")] aura: &str,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(canny_thief("Canny Thief").unwrap())]
                .into_iter()
                .chain((0..4).map(|_| ready(island()))),
        )
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![island(), island()]);
    }
    let scenario = scenario.library(1, vec![card(aura).unwrap(), island()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Canny Thief", 1)]).unwrap();
    driver.idle_until(1, Step::Main2).unwrap();
    let spell = driver
        .state()
        .objects()
        .find(|object| object.card().name == aura)
        .unwrap()
        .id();
    // The caster sees the card it may cast.
    let view = StateView::for_player(driver.state(), PlayerId(0));
    let seen = view
        .objects
        .iter()
        .find(|object| object.id == spell)
        .unwrap();
    assert_eq!(seen.name, aura);
    driver.begin_cast(aura).unwrap();
    let decision: Decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("no target decision: {:?}", decision.kind);
    };
    let ogre = driver.target_named("Ogre").unwrap();
    assert!(options.legal.contains(&ogre));
    let sent = serde_json::to_value(ServerMessage::Decision {
        id: DecisionId(5),
        decision: decision.clone(),
        deadline_ms: None,
        show_after_ms: None,
    })
    .unwrap();
    assert_eq!(sent["type"], "Decision");
    assert_eq!(sent["decision"]["player"], 0);
    let wire = &sent["decision"]["kind"]["ChooseTargets"];
    assert_eq!(wire["source"], json!(spell.0));
    assert_eq!(wire["min"], 1);
    let Target::Object(host) = ogre else {
        unreachable!()
    };
    assert!(
        wire["legal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target.to_string().contains(&host.id.0.to_string()))
    );
    let reply = json!({
        "type": "Answer",
        "decision": 5,
        "answer": serde_json::to_value(Answer::ChooseTargets(TargetChoice::Targets(vec![ogre]))).unwrap(),
    });
    let ClientMessage::Answer { answer, .. } = serde_json::from_value(reply).unwrap() else {
        panic!("not an answer");
    };
    driver.answer(answer).unwrap();
    let events = driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::SpellCast { object, choices, .. } if *object == spell && choices.targets == vec![vec![ogre]]
    )));
}
