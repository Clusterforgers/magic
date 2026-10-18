//! Goal: every client and server message variant survives a JSON round trip unchanged and keeps its documented wire shape.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::decision::{
    AttackOptions, AttackerOption, BlockOptions, BlockerOption, BottomOptions, DamageOptions,
    DecisionKind, DiscardOptions, MulliganChoice, MulliganOptions, PayChoice, PayOptions,
    PriorityAction, PriorityOptions,
};
use mtg_engine::forecast::{
    AttackerForecast, CombatForecast, CreatureForecast, ForecastDamage, PlayerForecast,
};
use mtg_engine::mana::{ManaOption, ManaPool, ManaSource, Payment};
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Engine, Event};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, EventBatch, EventView, FullControlMode, HiddenEvent,
    LobbySeat, PROTOCOL_VERSION, ServerMessage, StateView, Stop, StopSide, YieldUntil,
    project_events,
};
use mtg_testkit::fuzz_setup;
use proptest::prelude::*;
use rstest::rstest;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

const CLIENT_VARIANTS: usize = 13;
const SERVER_VARIANTS: usize = 12;

const YIELDS: [YieldUntil; 3] = [
    YieldUntil::Resolve,
    YieldUntil::EndOfTurn,
    YieldUntil::MyTurn,
];

const SIDES: [StopSide; 2] = [StopSide::Own, StopSide::Opponent];

const MODES: [FullControlMode; 3] = [
    FullControlMode::Off,
    FullControlMode::NextWindow,
    FullControlMode::Held,
];

/// Every stop a server may list: both sides, each step where a player gets priority, in the order it lists them.
fn every_stop() -> Vec<Stop> {
    SIDES
        .into_iter()
        .flat_map(|side| {
            Step::ALL
                .into_iter()
                .filter(|step| step.grants_priority())
                .map(move |step| Stop { side, step })
        })
        .collect()
}

/// Index of each client variant; adding a variant breaks this match until the table below grows.
fn client_index(message: &ClientMessage) -> usize {
    match message {
        ClientMessage::Join { .. } => 0,
        ClientMessage::Ready => 1,
        ClientMessage::Answer { .. } => 2,
        ClientMessage::Chat { .. } => 3,
        ClientMessage::Concede => 4,
        ClientMessage::Resync => 5,
        ClientMessage::AutoPass { .. } => 6,
        ClientMessage::Preview { .. } => 7,
        ClientMessage::Yield { .. } => 8,
        ClientMessage::CancelYield => 9,
        ClientMessage::NotReady => 10,
        ClientMessage::SetStop { .. } => 11,
        ClientMessage::FullControl { .. } => 12,
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
        ServerMessage::Forecast { .. } => 7,
        ServerMessage::Yielding { .. } => 8,
        ServerMessage::Lobby { .. } => 9,
        ServerMessage::Stops { .. } => 10,
        ServerMessage::FullControl { .. } => 11,
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
            can_act: true,
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

/// A lobby of three seats in every state a seat can be in: ready, here but not ready, away.
fn lobby_seats() -> Vec<LobbySeat> {
    let seat = |index: u64, name: &str, connected: bool, ready: bool| LobbySeat {
        seat: PlayerId(index),
        name: name.to_owned(),
        connected,
        ready,
    };
    vec![
        seat(0, "Ana", true, true),
        seat(1, "Bo \"the\" Ünïcode", true, false),
        seat(2, "Cy", false, false),
    ]
}

/// A forecast with a block, so every field of it is exercised: creature 4 attacks player 1 and creature 3 blocks it.
fn sample_forecast() -> CombatForecast {
    let (blocker, attacker) = (ObjectId(3), ObjectId(4));
    let hit = |source, target, amount| ForecastDamage {
        source,
        target,
        amount,
    };
    CombatForecast {
        attackers: vec![AttackerForecast {
            attacker,
            target: AttackTarget::Player(PlayerId(1)),
            blocked: true,
            blockers: vec![blocker],
            deals: vec![hit(attacker, DamageTarget::Object(blocker), 3)],
            receives: vec![hit(blocker, DamageTarget::Object(attacker), 1)],
        }],
        creatures: vec![
            CreatureForecast {
                object: blocker,
                damage: 3,
                dies: true,
            },
            CreatureForecast {
                object: attacker,
                damage: 1,
                dies: false,
            },
        ],
        players: vec![PlayerForecast {
            player: PlayerId(1),
            damage: 0,
            life: 20,
            life_after: 20,
            lethal: false,
        }],
    }
}

fn client_samples() -> Vec<ClientMessage> {
    let mut out = vec![
        ClientMessage::Join {
            protocol_version: PROTOCOL_VERSION,
            game: "table-1".into(),
            name: "Ana".into(),
        },
        ClientMessage::Ready,
        ClientMessage::NotReady,
        ClientMessage::Chat {
            text: "gg, \"quotes\" and ünïcode".into(),
        },
        ClientMessage::Concede,
        ClientMessage::Resync,
        ClientMessage::AutoPass { enabled: true },
        ClientMessage::AutoPass { enabled: false },
        ClientMessage::CancelYield,
    ];
    out.extend(YIELDS.map(|until| ClientMessage::Yield { until }));
    // Every step is well formed on the wire, the two where nobody gets priority too: the server refuses those by rule.
    for side in SIDES {
        for step in Step::ALL {
            for (on, keep) in [(true, false), (true, true), (false, false)] {
                out.push(ClientMessage::SetStop {
                    side,
                    step,
                    on,
                    keep,
                });
            }
        }
    }
    out.extend(MODES.map(|mode| ClientMessage::FullControl { mode }));
    out.extend(answers().into_iter().map(|answer| ClientMessage::Answer {
        decision: DecisionId(41),
        answer,
    }));
    out.extend(answers().into_iter().map(|answer| ClientMessage::Preview {
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
            auto_pass: true,
            yielding: None,
            stops: Vec::new(),
            full_control: FullControlMode::Off,
        },
        ServerMessage::Lobby {
            seats: lobby_seats(),
            needed: 2,
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
        ServerMessage::Forecast {
            decision: DecisionId(41),
            forecast: Some(sample_forecast()),
        },
        ServerMessage::Forecast {
            decision: DecisionId(41),
            forecast: None,
        },
    ];
    // A welcome states the yield the seat holds, and so does every change of it.
    out.extend(YIELDS.map(|until| ServerMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        game: "table-1".into(),
        player: PlayerId(0),
        seats: 2,
        auto_pass: false,
        yielding: Some(until),
        stops: Vec::new(),
        full_control: FullControlMode::Off,
    }));
    out.extend(YIELDS.map(|until| ServerMessage::Yielding { until: Some(until) }));
    out.push(ServerMessage::Yielding { until: None });
    // A welcome states the stops and the Full Control mode the seat holds, and so does every change of them.
    out.extend(MODES.map(|full_control| ServerMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        game: "table-1".into(),
        player: PlayerId(0),
        seats: 2,
        auto_pass: true,
        yielding: None,
        stops: every_stop(),
        full_control,
    }));
    out.push(ServerMessage::Stops { stops: Vec::new() });
    out.push(ServerMessage::Stops {
        stops: every_stop(),
    });
    out.extend(
        every_stop()
            .into_iter()
            .map(|stop| ServerMessage::Stops { stops: vec![stop] }),
    );
    out.extend(MODES.map(|mode| ServerMessage::FullControl { mode }));
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
    // Every other decision runs on a clock, so both shapes are covered for several kinds.
    out.extend(
        decision_kinds()
            .into_iter()
            .zip([Some(10_000), None].into_iter().cycle())
            .map(|(kind, deadline_ms)| ServerMessage::Decision {
                id: DecisionId(8),
                decision: Decision {
                    player: PlayerId(0),
                    kind,
                },
                deadline_ms,
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
#[case(ClientMessage::NotReady, json!({"type": "NotReady"}))]
#[case(ClientMessage::Resync, json!({"type": "Resync"}))]
#[case(ClientMessage::AutoPass { enabled: false }, json!({"type": "AutoPass", "enabled": false}))]
#[case(ClientMessage::CancelYield, json!({"type": "CancelYield"}))]
#[case(
    ClientMessage::Yield { until: YieldUntil::Resolve },
    json!({"type": "Yield", "until": "Resolve"})
)]
#[case(
    ClientMessage::Yield { until: YieldUntil::EndOfTurn },
    json!({"type": "Yield", "until": "EndOfTurn"})
)]
#[case(
    ClientMessage::Yield { until: YieldUntil::MyTurn },
    json!({"type": "Yield", "until": "MyTurn"})
)]
#[case(
    ClientMessage::SetStop { side: StopSide::Own, step: Step::Upkeep, on: true, keep: false },
    json!({"type": "SetStop", "side": "Own", "step": "Upkeep", "on": true, "keep": false})
)]
#[case(
    ClientMessage::SetStop { side: StopSide::Opponent, step: Step::EndCombat, on: true, keep: true },
    json!({"type": "SetStop", "side": "Opponent", "step": "EndCombat", "on": true, "keep": true})
)]
#[case(
    ClientMessage::SetStop { side: StopSide::Opponent, step: Step::FirstStrikeDamage, on: false, keep: false },
    json!({"type": "SetStop", "side": "Opponent", "step": "FirstStrikeDamage", "on": false, "keep": false})
)]
#[case(
    ClientMessage::FullControl { mode: FullControlMode::Off },
    json!({"type": "FullControl", "mode": "Off"})
)]
#[case(
    ClientMessage::FullControl { mode: FullControlMode::NextWindow },
    json!({"type": "FullControl", "mode": "NextWindow"})
)]
#[case(
    ClientMessage::FullControl { mode: FullControlMode::Held },
    json!({"type": "FullControl", "mode": "Held"})
)]
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
#[case(
    ClientMessage::Preview { decision: DecisionId(7), answer: Answer::DeclareBlockers(vec![(ObjectId(3), ObjectId(4))]) },
    json!({"type": "Preview", "decision": 7, "answer": {"DeclareBlockers": [[3, 4]]}})
)]
fn client_wire_shape_is_flat_and_tagged(
    #[case] message: ClientMessage,
    #[case] expected: serde_json::Value,
) {
    assert_eq!(serde_json::to_value(&message).unwrap(), expected);
}

#[rstest]
#[case(
    ServerMessage::Welcome {
        protocol_version: 1, game: "g".into(), player: PlayerId(2), seats: 4, auto_pass: true, yielding: None,
        stops: vec![], full_control: FullControlMode::Off,
    },
    json!({
        "type": "Welcome", "protocol_version": 1, "game": "g", "player": 2, "seats": 4, "auto_pass": true,
        "yielding": null, "stops": [], "full_control": "Off"
    })
)]
#[case(
    ServerMessage::Lobby {
        seats: vec![LobbySeat { seat: PlayerId(0), name: "Ana".into(), connected: true, ready: false }],
        needed: 1,
    },
    json!({
        "type": "Lobby", "needed": 1,
        "seats": [{"seat": 0, "name": "Ana", "connected": true, "ready": false}]
    })
)]
#[case(
    ServerMessage::Welcome {
        protocol_version: 1, game: "g".into(), player: PlayerId(0), seats: 2, auto_pass: false,
        yielding: Some(YieldUntil::MyTurn),
        stops: vec![Stop { side: StopSide::Own, step: Step::Main1 }, Stop { side: StopSide::Opponent, step: Step::End }],
        full_control: FullControlMode::Held,
    },
    json!({
        "type": "Welcome", "protocol_version": 1, "game": "g", "player": 0, "seats": 2, "auto_pass": false,
        "yielding": "MyTurn",
        "stops": [{"side": "Own", "step": "Main1"}, {"side": "Opponent", "step": "End"}],
        "full_control": "Held"
    })
)]
#[case(ServerMessage::Stops { stops: vec![] }, json!({"type": "Stops", "stops": []}))]
#[case(
    ServerMessage::Stops { stops: vec![
        Stop { side: StopSide::Own, step: Step::DeclareBlockers },
        Stop { side: StopSide::Opponent, step: Step::CombatDamage },
    ] },
    json!({"type": "Stops", "stops": [
        {"side": "Own", "step": "DeclareBlockers"},
        {"side": "Opponent", "step": "CombatDamage"},
    ]})
)]
#[case(
    ServerMessage::FullControl { mode: FullControlMode::NextWindow },
    json!({"type": "FullControl", "mode": "NextWindow"})
)]
#[case(
    ServerMessage::Yielding { until: Some(YieldUntil::EndOfTurn) },
    json!({"type": "Yielding", "until": "EndOfTurn"})
)]
#[case(ServerMessage::Yielding { until: None }, json!({"type": "Yielding", "until": null}))]
#[case(
    ServerMessage::Decision {
        id: DecisionId(3),
        decision: Decision {
            player: PlayerId(1),
            kind: DecisionKind::DeclareBlockers(BlockOptions { blockers: vec![] }),
        },
        deadline_ms: Some(45_000),
    },
    json!({
        "type": "Decision", "id": 3, "deadline_ms": 45_000,
        "decision": {"player": 1, "kind": {"DeclareBlockers": {"blockers": []}}}
    })
)]
#[case(
    ServerMessage::Decision {
        id: DecisionId(4),
        decision: Decision {
            player: PlayerId(0),
            kind: DecisionKind::DeclareBlockers(BlockOptions { blockers: vec![] }),
        },
        deadline_ms: None,
    },
    json!({
        "type": "Decision", "id": 4,
        "decision": {"player": 0, "kind": {"DeclareBlockers": {"blockers": []}}}
    })
)]
#[case(
    ServerMessage::Error { code: ErrorCode::StaleDecision, message: "late".into() },
    json!({"type": "Error", "code": "StaleDecision", "message": "late"})
)]
#[case(
    ServerMessage::GameOver { result: GameResult::Draw },
    json!({"type": "GameOver", "result": "Draw"})
)]
#[case(
    ServerMessage::Forecast { decision: DecisionId(7), forecast: None },
    json!({"type": "Forecast", "decision": 7, "forecast": null})
)]
#[case(
    ServerMessage::Forecast {
        decision: DecisionId(7),
        forecast: Some(CombatForecast {
            attackers: Vec::new(),
            creatures: vec![CreatureForecast { object: ObjectId(3), damage: 2, dies: true }],
            players: Vec::new(),
        }),
    },
    json!({"type": "Forecast", "decision": 7, "forecast": {
        "attackers": [],
        "creatures": [{"object": 3, "damage": 2, "dies": true}],
        "players": [],
    }})
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

#[rstest]
#[case::no_duration(r#"{"type":"Yield"}"#)]
#[case::unknown_duration(r#"{"type":"Yield","until":"Never"}"#)]
#[case::lower_case(r#"{"type":"Yield","until":"endofturn"}"#)]
#[case::null_duration(r#"{"type":"Yield","until":null}"#)]
#[case::number_duration(r#"{"type":"Yield","until":3}"#)]
#[case::object_duration(r#"{"type":"Yield","until":{"EndOfTurn":true}}"#)]
fn a_yield_that_is_not_one_of_the_three_is_refused(#[case] text: &str) {
    let parsed = serde_json::from_str::<ClientMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
}

#[rstest]
#[case::lower_case(r#"{"type":"Yielding","until":"mine"}"#)]
#[case::text_in_place_of_null(r#"{"type":"Yielding","until":"None"}"#)]
#[case::number(r#"{"type":"Yielding","until":0}"#)]
fn a_yielding_notice_with_a_wrong_duration_is_refused(#[case] text: &str) {
    let parsed = serde_json::from_str::<ServerMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
}

#[rstest]
#[case::no_side(r#"{"type":"SetStop","step":"Main1","on":true,"keep":false}"#)]
#[case::no_step(r#"{"type":"SetStop","side":"Own","on":true,"keep":false}"#)]
#[case::no_on(r#"{"type":"SetStop","side":"Own","step":"Main1","keep":false}"#)]
#[case::no_keep(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true}"#)]
#[case::unknown_side(r#"{"type":"SetStop","side":"Both","step":"Main1","on":true,"keep":false}"#)]
#[case::side_in_lower_case(
    r#"{"type":"SetStop","side":"own","step":"Main1","on":true,"keep":false}"#
)]
#[case::side_as_number(r#"{"type":"SetStop","side":0,"step":"Main1","on":true,"keep":false}"#)]
#[case::side_as_null(r#"{"type":"SetStop","side":null,"step":"Main1","on":true,"keep":false}"#)]
#[case::unknown_step(r#"{"type":"SetStop","side":"Own","step":"Combat","on":true,"keep":false}"#)]
#[case::step_in_lower_case(
    r#"{"type":"SetStop","side":"Own","step":"main1","on":true,"keep":false}"#
)]
#[case::step_with_a_space(
    r#"{"type":"SetStop","side":"Own","step":"Main 1","on":true,"keep":false}"#
)]
#[case::step_as_number(r#"{"type":"SetStop","side":"Own","step":3,"on":true,"keep":false}"#)]
#[case::step_as_object(
    r#"{"type":"SetStop","side":"Own","step":{"Main1":true},"on":true,"keep":false}"#
)]
#[case::on_as_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":"yes","keep":false}"#)]
#[case::on_as_number(r#"{"type":"SetStop","side":"Own","step":"Main1","on":1,"keep":false}"#)]
#[case::on_as_null(r#"{"type":"SetStop","side":"Own","step":"Main1","on":null,"keep":false}"#)]
#[case::keep_as_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":"no"}"#)]
#[case::keep_as_null(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":null}"#)]
fn a_stop_request_that_is_not_well_formed_is_refused(#[case] text: &str) {
    let parsed = serde_json::from_str::<ClientMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
}

#[rstest]
#[case::no_mode(r#"{"type":"FullControl"}"#)]
#[case::unknown_mode(r#"{"type":"FullControl","mode":"Always"}"#)]
#[case::lower_case(r#"{"type":"FullControl","mode":"held"}"#)]
#[case::a_flag_word(r#"{"type":"FullControl","mode":"On"}"#)]
#[case::a_flag(r#"{"type":"FullControl","mode":true}"#)]
#[case::null(r#"{"type":"FullControl","mode":null}"#)]
#[case::number(r#"{"type":"FullControl","mode":1}"#)]
#[case::object(r#"{"type":"FullControl","mode":{"Held":true}}"#)]
fn a_full_control_message_that_is_not_one_of_the_three_modes_is_refused(#[case] text: &str) {
    let parsed = serde_json::from_str::<ClientMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
    let parsed = serde_json::from_str::<ServerMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
}

#[rstest]
#[case::stops_missing(r#"{"type":"Stops"}"#)]
#[case::stops_null(r#"{"type":"Stops","stops":null}"#)]
#[case::stops_as_one_object(r#"{"type":"Stops","stops":{"side":"Own","step":"Main1"}}"#)]
#[case::a_stop_as_text(r#"{"type":"Stops","stops":["Main1"]}"#)]
#[case::a_stop_without_side(r#"{"type":"Stops","stops":[{"step":"Main1"}]}"#)]
#[case::a_stop_without_step(r#"{"type":"Stops","stops":[{"side":"Own"}]}"#)]
#[case::an_unknown_side(r#"{"type":"Stops","stops":[{"side":"Both","step":"Main1"}]}"#)]
#[case::an_unknown_step(r#"{"type":"Stops","stops":[{"side":"Own","step":"Combat"}]}"#)]
#[case::a_step_in_lower_case(r#"{"type":"Stops","stops":[{"side":"Own","step":"upkeep"}]}"#)]
#[case::welcome_without_stops(
    r#"{"type":"Welcome","protocol_version":1,"game":"g","player":0,"seats":2,"auto_pass":true,"yielding":null,"full_control":"Off"}"#
)]
#[case::welcome_without_full_control(
    r#"{"type":"Welcome","protocol_version":1,"game":"g","player":0,"seats":2,"auto_pass":true,"yielding":null,"stops":[]}"#
)]
#[case::welcome_with_an_unknown_mode(
    r#"{"type":"Welcome","protocol_version":1,"game":"g","player":0,"seats":2,"auto_pass":true,"yielding":null,"stops":[],"full_control":"Always"}"#
)]
fn a_notice_with_wrong_stops_or_a_wrong_mode_is_refused(#[case] text: &str) {
    let parsed = serde_json::from_str::<ServerMessage>(text);
    assert!(parsed.is_err(), "{text} was accepted as {parsed:?}");
}

#[test]
fn stops_sort_own_side_first_and_each_side_in_turn_order() {
    let stop = |side, step| Stop { side, step };
    let mut stops = vec![
        stop(StopSide::Opponent, Step::Main1),
        stop(StopSide::Own, Step::End),
        stop(StopSide::Opponent, Step::Upkeep),
        stop(StopSide::Own, Step::Upkeep),
    ];
    stops.sort();
    assert_eq!(
        stops,
        [
            stop(StopSide::Own, Step::Upkeep),
            stop(StopSide::Own, Step::End),
            stop(StopSide::Opponent, Step::Upkeep),
            stop(StopSide::Opponent, Step::Main1),
        ]
    );
    let every = every_stop();
    assert!(every.is_sorted(), "the list a server sends is in order");
    assert_eq!(
        every.len(),
        2 * 11,
        "eleven steps give priority, on two sides"
    );
}

#[test]
fn full_control_starts_off() {
    assert_eq!(FullControlMode::default(), FullControlMode::Off);
}
