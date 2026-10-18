//! Goal: every client and server message variant survives a JSON round trip unchanged and keeps its documented wire shape.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{
    AttackOptions, AttackerOption, BlockOptions, BlockerOption, BottomOptions, DamageOptions,
    DecisionKind, DiscardOptions, MulliganChoice, MulliganOptions, PayChoice, PayOptions,
    PriorityAction, PriorityOptions,
};
use mtg_engine::mana::{ManaOption, ManaPool, ManaSource, Payment};
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Decision, Engine, Event};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, EventBatch, EventView, HiddenEvent, PROTOCOL_VERSION,
    ServerMessage, StateView, project_events,
};
use mtg_testkit::fuzz_setup;
use proptest::prelude::*;
use rstest::rstest;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

const CLIENT_VARIANTS: usize = 6;
const SERVER_VARIANTS: usize = 7;

/// Index of each client variant; adding a variant breaks this match until the table below grows.
fn client_index(message: &ClientMessage) -> usize {
    match message {
        ClientMessage::Join { .. } => 0,
        ClientMessage::Ready => 1,
        ClientMessage::Answer { .. } => 2,
        ClientMessage::Chat { .. } => 3,
        ClientMessage::Concede => 4,
        ClientMessage::Resync => 5,
    }
}

/// Index of each server variant, as for [`client_index`].
fn server_index(message: &ServerMessage) -> usize {
    match message {
        ServerMessage::Welcome { .. } => 0,
        ServerMessage::Decision { .. } => 1,
        ServerMessage::Events(_) => 2,
        ServerMessage::Snapshot { .. } => 3,
        ServerMessage::Chat { .. } => 4,
        ServerMessage::Error { .. } => 5,
        ServerMessage::GameOver { .. } => 6,
    }
}

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
    let text = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, value, "round trip changed {text}");
}

fn answers() -> Vec<Answer> {
    let (a, b) = (ObjectId(3), ObjectId(4));
    let source = ManaSource {
        object: a,
        ability: 0,
    };
    vec![
        Answer::Mulligan(MulliganChoice::Keep),
        Answer::Mulligan(MulliganChoice::Mulligan),
        Answer::BottomCards(vec![a, b]),
        Answer::Priority(PriorityAction::Pass),
        Answer::Priority(PriorityAction::PlayLand(a)),
        Answer::Priority(PriorityAction::CastSpell(b)),
        Answer::Priority(PriorityAction::ActivateMana(source)),
        Answer::PayMana(PayChoice::Auto),
        Answer::PayMana(PayChoice::Cancel),
        Answer::PayMana(PayChoice::Manual(Payment {
            pool: vec![ManaType::Red],
            taps: vec![source],
            life: 0,
        })),
        Answer::PayMana(PayChoice::Manual(Payment {
            pool: vec![],
            taps: vec![],
            life: 4,
        })),
        Answer::PayMana(PayChoice::Complete(Payment {
            pool: vec![],
            taps: vec![source],
            life: 2,
        })),
        Answer::DeclareAttackers(vec![
            (a, AttackTarget::Player(PlayerId(1))),
            (b, AttackTarget::Planeswalker(ObjectId(9))),
        ]),
        Answer::DeclareBlockers(vec![(a, b)]),
        Answer::AssignDamage(vec![(a, 2), (b, 1)]),
        Answer::Discard(vec![a]),
    ]
}

fn decision_kinds() -> Vec<DecisionKind> {
    let (a, b) = (ObjectId(3), ObjectId(4));
    let option = ManaOption {
        source: ManaSource {
            object: a,
            ability: 0,
        },
        produces: ManaType::Green,
    };
    vec![
        DecisionKind::Mulligan(MulliganOptions {
            hand: vec![a, b],
            mulligans_taken: 1,
            can_mulligan: true,
            free_mulligans: 1,
            bottom_if_keep: 0,
            bottom_if_mulligan: 1,
        }),
        DecisionKind::BottomCards(BottomOptions {
            hand: vec![a, b],
            count: 1,
        }),
        DecisionKind::Priority(PriorityOptions {
            playable_lands: vec![a],
            castable: vec![b],
            mana_abilities: vec![option],
            can_pass: true,
        }),
        DecisionKind::PayMana(PayOptions {
            spell: b,
            cost: ManaCost::parse("{1}{G}").unwrap(),
            pool: ManaPool::default(),
            sources: vec![option],
            auto_available: true,
            can_cancel: true,
            life: 20,
            max_life: 0,
        }),
        DecisionKind::PayMana(PayOptions {
            spell: b,
            cost: ManaCost::parse("{2}{W/P}{G/U/P}").unwrap(),
            pool: ManaPool::default(),
            sources: vec![option],
            auto_available: false,
            can_cancel: false,
            life: 3,
            max_life: 2,
        }),
        DecisionKind::DeclareAttackers(AttackOptions {
            attackers: vec![AttackerOption {
                object: a,
                defenders: vec![AttackTarget::Player(PlayerId(1))],
            }],
        }),
        DecisionKind::DeclareBlockers(BlockOptions {
            blockers: vec![BlockerOption {
                object: a,
                can_block: vec![b],
            }],
        }),
        DecisionKind::AssignDamage(DamageOptions {
            attacker: a,
            power: 3,
            blockers: vec![b],
        }),
        DecisionKind::Discard(DiscardOptions {
            hand: vec![a, b],
            count: 1,
        }),
    ]
}

/// A real snapshot and event batch from the opening of a two-player game.
fn sample_view_and_batch() -> (StateView, EventBatch) {
    let setup = fuzz_setup(2, 7);
    let before = mtg_engine::GameState::from_setup(&setup).unwrap();
    let engine = Engine::new(setup).unwrap();
    let batch = project_events(&before, engine.events(), &[PlayerId(0)], 0)
        .unwrap()
        .remove(0);
    (StateView::for_player(engine.state(), PlayerId(0)), batch)
}

fn client_samples() -> Vec<ClientMessage> {
    let mut out = vec![
        ClientMessage::Join {
            protocol_version: PROTOCOL_VERSION,
            game: "table-1".into(),
            name: "Ana".into(),
        },
        ClientMessage::Ready,
        ClientMessage::Chat {
            text: "gg, \"quotes\" and ünïcode".into(),
        },
        ClientMessage::Concede,
        ClientMessage::Resync,
    ];
    out.extend(answers().into_iter().map(|answer| ClientMessage::Answer {
        decision: DecisionId(41),
        answer,
    }));
    out
}

fn server_samples() -> Vec<ServerMessage> {
    let (view, batch) = sample_view_and_batch();
    let mut out = vec![
        ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            game: "table-1".into(),
            player: PlayerId(1),
            seats: 4,
        },
        ServerMessage::Events(batch.clone()),
        ServerMessage::Snapshot {
            seq: 12,
            state: Box::new(view),
        },
        ServerMessage::Chat {
            from: PlayerId(2),
            text: "hello".into(),
        },
        ServerMessage::GameOver {
            result: GameResult::Winner(PlayerId(3)),
        },
        ServerMessage::GameOver {
            result: GameResult::Draw,
        },
    ];
    for code in [
        ErrorCode::UnsupportedVersion,
        ErrorCode::BadMessage,
        ErrorCode::UnknownGame,
        ErrorCode::StaleDecision,
        ErrorCode::IllegalAnswer,
        ErrorCode::Internal,
    ] {
        out.push(ServerMessage::Error {
            code,
            message: format!("{code:?}"),
        });
    }
    out.extend(
        decision_kinds()
            .into_iter()
            .map(|kind| ServerMessage::Decision {
                id: DecisionId(8),
                decision: Decision {
                    player: PlayerId(0),
                    kind,
                },
            }),
    );
    out
}

#[test]
fn every_client_variant_round_trips() {
    let samples = client_samples();
    let seen: BTreeSet<usize> = samples.iter().map(client_index).collect();
    assert_eq!(
        seen,
        (0..CLIENT_VARIANTS).collect(),
        "a variant has no sample"
    );
    samples.iter().for_each(round_trip);
}

#[test]
fn every_server_variant_round_trips() {
    let samples = server_samples();
    let seen: BTreeSet<usize> = samples.iter().map(server_index).collect();
    assert_eq!(
        seen,
        (0..SERVER_VARIANTS).collect(),
        "a variant has no sample"
    );
    samples.iter().for_each(round_trip);
}

#[test]
fn every_event_view_variant_round_trips() {
    let player = PlayerId(1);
    let hidden = [
        HiddenEvent::CardDrawn { player },
        HiddenEvent::HandDealt { player, count: 7 },
        HiddenEvent::LibraryShuffled { player },
        HiddenEvent::MulliganTaken { player },
        HiddenEvent::CardsBottomed { player, count: 2 },
        HiddenEvent::ObjectLeftGame,
    ];
    for event in hidden {
        round_trip(&EventView::Hidden(event));
    }
    round_trip(&EventView::Public(Event::MulliganPhaseBegan));
    round_trip(&EventView::Public(Event::CardDrawn {
        player,
        object: ObjectId(5),
    }));
}

#[rstest]
#[case(ClientMessage::Ready, json!({"type": "Ready"}))]
#[case(ClientMessage::Resync, json!({"type": "Resync"}))]
#[case(
    ClientMessage::Join { protocol_version: 1, game: "g".into(), name: "n".into() },
    json!({"type": "Join", "protocol_version": 1, "game": "g", "name": "n"})
)]
#[case(
    ClientMessage::Answer { decision: DecisionId(7), answer: Answer::Priority(PriorityAction::Pass) },
    json!({"type": "Answer", "decision": 7, "answer": {"Priority": "Pass"}})
)]
#[case(
    ClientMessage::Chat { text: "hi".into() },
    json!({"type": "Chat", "text": "hi"})
)]
fn client_wire_shape_is_flat_and_tagged(
    #[case] message: ClientMessage,
    #[case] expected: serde_json::Value,
) {
    assert_eq!(serde_json::to_value(&message).unwrap(), expected);
}

#[rstest]
#[case(
    ServerMessage::Welcome { protocol_version: 1, game: "g".into(), player: PlayerId(2), seats: 4 },
    json!({"type": "Welcome", "protocol_version": 1, "game": "g", "player": 2, "seats": 4})
)]
#[case(
    ServerMessage::Error { code: ErrorCode::StaleDecision, message: "late".into() },
    json!({"type": "Error", "code": "StaleDecision", "message": "late"})
)]
#[case(
    ServerMessage::GameOver { result: GameResult::Draw },
    json!({"type": "GameOver", "result": "Draw"})
)]
fn server_wire_shape_is_flat_and_tagged(
    #[case] message: ServerMessage,
    #[case] expected: serde_json::Value,
) {
    assert_eq!(serde_json::to_value(&message).unwrap(), expected);
}

#[test]
fn mulligan_options_carry_the_bottom_counts_by_name() {
    let options = MulliganOptions {
        hand: vec![ObjectId(3)],
        mulligans_taken: 1,
        can_mulligan: true,
        free_mulligans: 1,
        bottom_if_keep: 0,
        bottom_if_mulligan: 1,
    };
    assert_eq!(
        serde_json::to_value(DecisionKind::Mulligan(options)).unwrap(),
        json!({"Mulligan": {
            "hand": [3],
            "mulligans_taken": 1,
            "can_mulligan": true,
            "free_mulligans": 1,
            "bottom_if_keep": 0,
            "bottom_if_mulligan": 1,
        }})
    );
}

#[test]
fn a_cost_is_the_list_of_its_symbols_in_canonical_order() {
    let cost = ManaCost::parse("{G/U/P}{C/P}{W/P}{2/R}{B/R}{C/W}{G}{C}{2}").unwrap();
    assert_eq!(
        serde_json::to_value(&cost).unwrap(),
        json!([
            {"Generic": 2},
            "Colorless",
            {"Colored": "Green"},
            {"ColorlessHybrid": "White"},
            {"Hybrid": ["Black", "Red"]},
            {"MonoHybrid": "Red"},
            {"Phyrexian": "White"},
            "ColorlessPhyrexian",
            {"HybridPhyrexian": ["Green", "Blue"]},
        ])
    );
    assert_eq!(serde_json::to_value(ManaCost::ZERO).unwrap(), json!([]));
    // A list written another way round, or with a pair the other way round, reads as the same cost.
    let jumbled: ManaCost = serde_json::from_value(json!([
        {"Colored": "Green"},
        {"Hybrid": ["Red", "Black"]},
        {"Generic": 1},
        {"Generic": 1},
    ]))
    .unwrap();
    assert_eq!(jumbled, ManaCost::parse("{2}{G}{B/R}").unwrap());
}

#[test]
fn a_payment_names_the_life_it_pays_and_may_leave_it_out() {
    let payment = Payment {
        pool: vec![ManaType::Red],
        taps: vec![],
        life: 2,
    };
    assert_eq!(
        serde_json::to_value(&payment).unwrap(),
        json!({"pool": ["Red"], "taps": [], "life": 2})
    );
    let without: Payment = serde_json::from_value(json!({"pool": [], "taps": []})).unwrap();
    assert_eq!(
        without.life, 0,
        "a client that knows no Phyrexian mana pays no life"
    );
}

#[test]
fn pay_options_carry_the_life_total_and_the_most_life_that_may_be_paid() {
    let options = PayOptions {
        spell: ObjectId(4),
        cost: ManaCost::parse("{W/P}").unwrap(),
        pool: ManaPool::default(),
        sources: vec![],
        auto_available: false,
        can_cancel: true,
        life: 2,
        max_life: 2,
    };
    let value = serde_json::to_value(DecisionKind::PayMana(options)).unwrap();
    assert_eq!(value["PayMana"]["life"], 2);
    assert_eq!(value["PayMana"]["max_life"], 2);
    assert_eq!(value["PayMana"]["cost"], json!([{"Phyrexian": "White"}]));
}

#[test]
fn event_batches_are_flattened_into_the_tagged_object() {
    let (_, batch) = sample_view_and_batch();
    let value = serde_json::to_value(ServerMessage::Events(batch)).unwrap();
    assert_eq!(value["type"], "Events");
    assert!(value["from_seq"].is_number() && value["events"].is_array());
    assert!(value["objects"].is_array() && value["gone"].is_array());
    assert!(
        value["table"]["players"].is_array(),
        "the batch carries the table"
    );
    assert!(
        value["table"].get("objects").is_none(),
        "objects sit beside the table"
    );
}

#[test]
fn snapshots_stay_one_flat_object_with_the_same_table_fields_as_a_batch() {
    let (view, batch) = sample_view_and_batch();
    let snapshot = ServerMessage::Snapshot {
        seq: 3,
        state: Box::new(view),
    };
    let value = serde_json::to_value(snapshot).unwrap();
    let state = value["state"].as_object().unwrap();
    assert!(
        !state.contains_key("table"),
        "the table is flattened into the state"
    );
    let mut expected: BTreeSet<String> = serde_json::to_value(&batch.table)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    expected.insert("objects".into());
    assert_eq!(state.keys().cloned().collect::<BTreeSet<_>>(), expected);
}

proptest! {
    #[test]
    fn arbitrary_text_and_ids_round_trip(
        text in any::<String>(),
        id in any::<u32>(),
        cards in prop::collection::vec(any::<u32>(), 0..8),
    ) {
        let join = ClientMessage::Join { protocol_version: id, game: text.clone(), name: text.clone() };
        let answer = ClientMessage::Answer {
            decision: DecisionId(u64::from(id)),
            answer: Answer::Discard(cards.iter().map(|c| ObjectId(u64::from(*c))).collect()),
        };
        let error = ServerMessage::Error { code: ErrorCode::BadMessage, message: text };
        round_trip(&join);
        round_trip(&answer);
        round_trip(&error);
    }
}
