//! Goal: every client and server message variant survives a JSON round trip unchanged and keeps its documented wire shape.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::abilities::{AbilityRef, Cause, PutOnStack};
use mtg_engine::choose::{
    CardsChoice, CardsOptions, Choice, ChoiceOption, Choices, CostsChoice, CostsOptions,
    DivideChoice, DivideOptions, ModeChoice, ModeOption, ModeOptions, NumberChoice, NumberOptions,
    OptionChoice, OptionOptions, OptionalCost, OrderChoice, OrderOptions, ReplacementCandidate,
    ReplacementChoice, ReplacementOptions, Target, TargetChoice, TargetOptions, TriggerEntry,
    TriggersChoice, TriggersOptions, YesNoChoice, YesNoOptions,
};
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::counters::CountersChanged;
use mtg_engine::decision::{
    AttackOptions, AttackerOption, BlockOptions, BlockerOption, BottomOptions, DamageOptions,
    DecisionKind, DiscardOptions, MulliganChoice, MulliganOptions, PayChoice, PayOptions,
    PriorityAction, PriorityOptions,
};
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::forecast::{
    AttackerForecast, CombatForecast, CreatureForecast, ForecastDamage, PlayerForecast,
};
use mtg_engine::library::{CardsFound, LibraryLooked, Revealed, Scried};
use mtg_engine::mana::{ManaOption, ManaPool, ManaSource, Payment};
use mtg_engine::object::ObjectRef;
use mtg_engine::snapshot::snapshot;
use mtg_engine::state::GameResult;
use mtg_engine::tokens::TokenCreated;
use mtg_engine::triggers::{DelayedId, Triggered};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
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

/// Every stop a server may list: both sides, each step where a player gets priority, in the order it lists them;
/// lasting and one-turn stops alternate, so both kinds are on every side.
fn every_stop() -> Vec<Stop> {
    SIDES
        .into_iter()
        .flat_map(|side| {
            Step::ALL
                .into_iter()
                .filter(|step| step.grants_priority())
                .enumerate()
                .map(move |(n, step)| Stop {
                    side,
                    step,
                    keep: n % 2 == 0,
                })
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
        Answer::Priority(PriorityAction::Activate(AbilityRef {
            object: a,
            ability: 1,
        })),
        Answer::ChooseTargets(TargetChoice::Targets(vec![
            Target::Player(PlayerId(1)),
            Target::Object(ObjectRef { id: b, counter: 2 }),
        ])),
        Answer::ChooseTargets(TargetChoice::Targets(Vec::new())),
        Answer::ChooseTargets(TargetChoice::Cancel),
        Answer::ChooseMode(ModeChoice::Modes(vec![0, 2])),
        Answer::ChooseMode(ModeChoice::Cancel),
        Answer::ChooseNumber(NumberChoice(4)),
        Answer::DivideAmount(DivideChoice(vec![
            (Target::Player(PlayerId(0)), 2),
            (Target::Object(ObjectRef { id: a, counter: 0 }), 1),
        ])),
        Answer::ChooseCards(CardsChoice(vec![a])),
        Answer::OrderCards(OrderChoice(vec![b, a])),
        Answer::YesNo(YesNoChoice(true)),
        Answer::ChooseOption(OptionChoice(1)),
        Answer::ChooseCosts(CostsChoice(vec![0])),
        Answer::OrderTriggers(TriggersChoice(vec![1, 0])),
        Answer::ChooseReplacement(ReplacementChoice(0)),
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
            abilities: Vec::new(),
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
            min_blockers: Vec::new(),
        }),
        DecisionKind::DeclareBlockers(BlockOptions {
            blockers: vec![BlockerOption {
                object: a,
                can_block: vec![b],
            }],
            min_blockers: vec![(b, 2)],
        }),
        DecisionKind::AssignDamage(DamageOptions {
            attacker: a,
            power: 3,
            blockers: vec![b],
            excess: None,
            lethal: vec![2],
        }),
        DecisionKind::AssignDamage(DamageOptions {
            attacker: a,
            power: 5,
            blockers: vec![b],
            excess: Some(DamageTarget::Player(PlayerId(1))),
            lethal: vec![1],
        }),
        DecisionKind::Discard(DiscardOptions {
            hand: vec![a, b],
            count: 1,
        }),
    ]
}

/// The decisions spells and abilities added, and a priority decision that offers an ability.
#[allow(clippy::too_many_lines)]
fn pipeline_decision_kinds() -> Vec<DecisionKind> {
    let (a, b) = (ObjectId(3), ObjectId(4));
    vec![
        DecisionKind::Priority(PriorityOptions {
            playable_lands: Vec::new(),
            castable: Vec::new(),
            mana_abilities: Vec::new(),
            abilities: vec![AbilityRef {
                object: a,
                ability: 0,
            }],
            can_act: true,
            can_pass: true,
        }),
        DecisionKind::ChooseTargets(TargetOptions {
            source: b,
            spec: 1,
            legal: vec![
                Target::Player(PlayerId(0)),
                Target::Object(ObjectRef { id: a, counter: 0 }),
            ],
            min: 0,
            max: 2,
            can_cancel: false,
            wording: "up to two target creatures".into(),
            triggered: true,
        }),
        DecisionKind::ChooseMode(ModeOptions {
            source: b,
            modes: vec![
                ModeOption {
                    index: 0,
                    label: Some("Draw".into()),
                },
                ModeOption {
                    index: 1,
                    label: None,
                },
            ],
            min: 1,
            max: 2,
            repeat: false,
            can_cancel: true,
        }),
        DecisionKind::ChooseNumber(NumberOptions {
            source: b,
            min: 0,
            max: None,
        }),
        DecisionKind::DivideAmount(DivideOptions {
            source: b,
            total: 4,
            among: vec![Target::Player(PlayerId(1)), Target::Player(PlayerId(0))],
        }),
        DecisionKind::ChooseCards(CardsOptions {
            source: b,
            from: vec![a, b],
            min: 1,
            max: 1,
            text: String::new(),
        }),
        DecisionKind::OrderCards(OrderOptions {
            source: b,
            cards: vec![a, b],
            text: String::new(),
        }),
        DecisionKind::YesNo(YesNoOptions {
            source: b,
            text: "Pay {1}?".into(),
        }),
        DecisionKind::ChooseOption(OptionOptions {
            source: b,
            options: vec![
                ChoiceOption::Player(PlayerId(1)),
                ChoiceOption::Color(Color::Red),
                ChoiceOption::Label("Goblin".into()),
            ],
            text: "Choose a creature type.".into(),
        }),
        DecisionKind::ChooseCosts(CostsOptions {
            source: b,
            optional: vec![OptionalCost {
                index: 0,
                label: "Kicker {R}".into(),
            }],
        }),
        DecisionKind::OrderTriggers(TriggersOptions {
            triggers: vec![
                TriggerEntry {
                    source: ObjectRef { id: a, counter: 1 },
                    ability: 0,
                    when: "When it enters the battlefield".into(),
                },
                TriggerEntry {
                    source: ObjectRef { id: b, counter: 0 },
                    ability: 2,
                    when: String::new(),
                },
            ],
        }),
        DecisionKind::ChooseReplacement(ReplacementOptions {
            candidates: vec![ReplacementCandidate {
                source: a,
                ability: 0,
            }],
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
        deck: None,
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
            .chain(pipeline_decision_kinds())
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
        HiddenEvent::LibraryLooked { player, count: 3 },
        HiddenEvent::CardsFound { player, count: 1 },
        HiddenEvent::Scried {
            player,
            top: 1,
            bottom: 2,
        },
        HiddenEvent::ZoneChanged {
            from: Zone::Library(player),
            to: Zone::Hand(player),
            cause: ZoneCause::Moved,
        },
        HiddenEvent::ChoiceMade { player },
        HiddenEvent::StepDone {
            position: vec![1, 0, 2],
        },
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
        seats: vec![LobbySeat { seat: PlayerId(0), name: "Ana".into(), connected: true, ready: false, deck: None }],
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
        stops: vec![
            Stop { side: StopSide::Own, step: Step::Main1, keep: true },
            Stop { side: StopSide::Opponent, step: Step::End, keep: false },
        ],
        full_control: FullControlMode::Held,
    },
    json!({
        "type": "Welcome", "protocol_version": 1, "game": "g", "player": 0, "seats": 2, "auto_pass": false,
        "yielding": "MyTurn",
        "stops": [
            {"side": "Own", "step": "Main1", "keep": true},
            {"side": "Opponent", "step": "End", "keep": false},
        ],
        "full_control": "Held"
    })
)]
#[case(ServerMessage::Stops { stops: vec![] }, json!({"type": "Stops", "stops": []}))]
#[case(
    ServerMessage::Stops { stops: vec![
        Stop { side: StopSide::Own, step: Step::DeclareBlockers, keep: false },
        Stop { side: StopSide::Opponent, step: Step::CombatDamage, keep: true },
    ] },
    json!({"type": "Stops", "stops": [
        {"side": "Own", "step": "DeclareBlockers", "keep": false},
        {"side": "Opponent", "step": "CombatDamage", "keep": true},
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
            kind: DecisionKind::DeclareBlockers(BlockOptions { blockers: vec![], min_blockers: vec![] }),
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
            kind: DecisionKind::DeclareBlockers(BlockOptions { blockers: vec![], min_blockers: vec![] }),
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
#[case::stops_as_one_object(
    r#"{"type":"Stops","stops":{"side":"Own","step":"Main1","keep":true}}"#
)]
#[case::a_stop_as_text(r#"{"type":"Stops","stops":["Main1"]}"#)]
#[case::a_stop_without_side(r#"{"type":"Stops","stops":[{"step":"Main1","keep":true}]}"#)]
#[case::a_stop_without_step(r#"{"type":"Stops","stops":[{"side":"Own","keep":true}]}"#)]
#[case::a_stop_without_keep(r#"{"type":"Stops","stops":[{"side":"Own","step":"Main1"}]}"#)]
#[case::an_unknown_side(r#"{"type":"Stops","stops":[{"side":"Both","step":"Main1","keep":true}]}"#)]
#[case::an_unknown_step(r#"{"type":"Stops","stops":[{"side":"Own","step":"Combat","keep":true}]}"#)]
#[case::a_step_in_lower_case(
    r#"{"type":"Stops","stops":[{"side":"Own","step":"upkeep","keep":true}]}"#
)]
#[case::keep_as_text(r#"{"type":"Stops","stops":[{"side":"Own","step":"Main1","keep":"yes"}]}"#)]
#[case::keep_as_a_number(r#"{"type":"Stops","stops":[{"side":"Own","step":"Main1","keep":1}]}"#)]
#[case::keep_as_null(r#"{"type":"Stops","stops":[{"side":"Own","step":"Main1","keep":null}]}"#)]
#[case::welcome_with_a_stop_without_keep(
    r#"{"type":"Welcome","protocol_version":1,"game":"g","player":0,"seats":2,"auto_pass":true,"yielding":null,"stops":[{"side":"Own","step":"Main1"}],"full_control":"Off"}"#
)]
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
fn stops_sort_own_side_first_and_each_side_in_turn_order_whether_they_last_or_not() {
    let stop = |side, step, keep| Stop { side, step, keep };
    // Every mix of lasting and one-turn stops sorts the same way: `keep` never decides the place of a stop.
    for mask in 0..16_u32 {
        let keep = |bit: u32| mask & (1 << bit) != 0;
        let mut stops = [
            stop(StopSide::Opponent, Step::Main1, keep(0)),
            stop(StopSide::Own, Step::End, keep(1)),
            stop(StopSide::Opponent, Step::Upkeep, keep(2)),
            stop(StopSide::Own, Step::Upkeep, keep(3)),
        ];
        stops.sort();
        let order: Vec<_> = stops.iter().map(|s| (s.side, s.step)).collect();
        assert_eq!(
            order,
            [
                (StopSide::Own, Step::Upkeep),
                (StopSide::Own, Step::End),
                (StopSide::Opponent, Step::Upkeep),
                (StopSide::Opponent, Step::Main1),
            ],
            "mask {mask:04b}"
        );
    }
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

/// The events of ability objects and of a resolution, with the fields they carry.
fn pipeline_events() -> Vec<Event> {
    let (a, b) = (ObjectId(3), ObjectId(4));
    let player = PlayerId(1);
    let setup = fuzz_setup(2, 3);
    let state = mtg_engine::GameState::from_setup(&setup).unwrap();
    let shot = snapshot(&state, a).unwrap();
    let source = ObjectRef { id: a, counter: 2 };
    let target = Target::Object(ObjectRef { id: b, counter: 1 });
    let choices = Choices {
        modes: vec![1],
        x: Some(3),
        targets: vec![vec![target, Target::Player(player)]],
        division: vec![(target, 2)],
        optional_costs: vec![0],
    };
    vec![
        Event::SpellCast {
            player,
            object: a,
            choices: choices.clone(),
        },
        Event::AbilityPutOnStack(Box::new(PutOnStack {
            object: ObjectId(90),
            source,
            snapshot: shot.clone(),
            ability: 1,
            controller: player,
            choices,
            cause: Some(Cause {
                subjects: vec![target],
                snapshots: vec![shot.clone()],
            }),
            delayed: Some(DelayedId(2)),
        })),
        Event::AbilityActivated {
            player,
            source,
            ability: 0,
        },
        Event::AbilityTriggered(Box::new(Triggered {
            source,
            ability: 0,
            controller: player,
            cause: Cause::default(),
            snapshot: shot.clone(),
            delayed: None,
        })),
        Event::TriggersOrdered {
            player,
            order: vec![1, 0],
        },
        Event::TriggerRemoved {
            source,
            ability: 0,
            delayed: Some(DelayedId(2)),
            cause: Cause::default(),
        },
        Event::DelayedTriggerExpired { id: DelayedId(2) },
        Event::ResolutionBegan { object: a },
        Event::ChoiceMade {
            player,
            choice: Choice::Cards(vec![a, b]),
        },
        Event::ChoiceMade {
            player,
            choice: Choice::YesNo(false),
        },
        Event::StepDone {
            position: vec![2, 0, 1],
            bound: vec![("found".into(), vec![target])],
            snapshots: vec![shot],
        },
        Event::ResolutionEnded {
            object: a,
            outcome: ResolutionOutcome::Resolved,
        },
        Event::ResolutionEnded {
            object: a,
            outcome: ResolutionOutcome::Fizzled,
        },
        Event::ResolutionEnded {
            object: a,
            outcome: ResolutionOutcome::Countered,
        },
    ]
}

/// The events of the first primitives: zones, life, counters, tokens and what players learn.
fn primitive_events() -> Vec<Event> {
    let (a, b) = (ObjectId(3), ObjectId(4));
    let player = PlayerId(1);
    vec![
        Event::ZoneChanged {
            object: a,
            from: Zone::Battlefield,
            to: Zone::Graveyard(player),
            cause: ZoneCause::Destroyed,
        },
        Event::LifeGained { player, amount: 3 },
        Event::LifeLost { player, amount: 2 },
        Event::CountersChanged(CountersChanged {
            on: Target::Player(player),
            counter: CounterKind::Poison,
            delta: -1,
        }),
        Event::TokenCreated(TokenCreated {
            object: ObjectId(91),
            controller: player,
            name: "Treasure".into(),
        }),
        Event::LibraryLooked(LibraryLooked {
            player,
            cards: vec![a],
        }),
        Event::CardsFound(CardsFound {
            player,
            zone: Zone::Library(player),
            cards: vec![a, b],
        }),
        Event::Scried(Scried {
            player,
            top: vec![a],
            bottom: vec![b],
        }),
        Event::Revealed(Revealed {
            player,
            cards: vec![a],
        }),
        Event::Entered {
            object: a,
            tapped: true,
            controller: player,
        },
    ]
}

#[test]
fn every_event_of_the_pipeline_round_trips_as_logged_and_in_a_batch() {
    for event in pipeline_events().into_iter().chain(primitive_events()) {
        round_trip(&event);
        round_trip(&EventView::Public(event));
    }
}

#[test]
fn the_question_of_a_yes_no_or_option_decision_is_left_out_while_empty_and_a_mode_decision_says_if_it_can_be_backed_out_of()
 {
    let source = ObjectId(4);
    let shape = |kind: DecisionKind| {
        serde_json::to_value(Decision {
            player: PlayerId(0),
            kind,
        })
        .unwrap()["kind"]
            .clone()
    };
    assert_eq!(
        shape(DecisionKind::YesNo(YesNoOptions {
            source,
            text: String::new(),
        })),
        json!({"YesNo": {"source": 4}})
    );
    assert_eq!(
        shape(DecisionKind::YesNo(YesNoOptions {
            source,
            text: "Pay {1}?".into(),
        })),
        json!({"YesNo": {"source": 4, "text": "Pay {1}?"}})
    );
    assert_eq!(
        shape(DecisionKind::ChooseOption(OptionOptions {
            source,
            options: vec![ChoiceOption::Color(Color::Red)],
            text: String::new(),
        })),
        json!({"ChooseOption": {"source": 4, "options": [{"Color": "Red"}]}})
    );
    assert_eq!(
        shape(DecisionKind::ChooseMode(ModeOptions {
            source,
            modes: vec![ModeOption {
                index: 1,
                label: None,
            }],
            min: 1,
            max: 1,
            repeat: false,
            can_cancel: false,
        })),
        json!({"ChooseMode": {
            "source": 4, "modes": [{"index": 1, "label": null}],
            "min": 1, "max": 1, "repeat": false, "can_cancel": false
        }})
    );
    // A decision without a question still parses, with an empty one.
    let old: DecisionKind = serde_json::from_value(json!({"YesNo": {"source": 4}})).unwrap();
    assert_eq!(
        old,
        DecisionKind::YesNo(YesNoOptions {
            source,
            text: String::new(),
        })
    );
}

#[test]
fn the_new_decisions_and_answers_have_their_documented_wire_shape() {
    let target = Target::Object(ObjectRef {
        id: ObjectId(9),
        counter: 2,
    });
    let decision = Decision {
        player: PlayerId(0),
        kind: DecisionKind::ChooseTargets(TargetOptions {
            source: ObjectId(4),
            spec: 0,
            legal: vec![Target::Player(PlayerId(1)), target],
            min: 1,
            max: 1,
            can_cancel: true,
            wording: String::new(),
            triggered: false,
        }),
    };
    assert_eq!(
        serde_json::to_value(&decision).unwrap(),
        json!({"player": 0, "kind": {"ChooseTargets": {
            "source": 4, "spec": 0, "min": 1, "max": 1, "can_cancel": true,
            "legal": [{"Player": 1}, {"Object": {"id": 9, "counter": 2}}]
        }}})
    );
    let shapes = [
        (
            Answer::ChooseTargets(TargetChoice::Targets(vec![target])),
            json!({"ChooseTargets": {"Targets": [{"Object": {"id": 9, "counter": 2}}]}}),
        ),
        (
            Answer::ChooseTargets(TargetChoice::Cancel),
            json!({"ChooseTargets": "Cancel"}),
        ),
        (
            Answer::ChooseMode(ModeChoice::Modes(vec![0, 2])),
            json!({"ChooseMode": {"Modes": [0, 2]}}),
        ),
        (
            Answer::ChooseMode(ModeChoice::Cancel),
            json!({"ChooseMode": "Cancel"}),
        ),
        (
            Answer::ChooseNumber(NumberChoice(4)),
            json!({"ChooseNumber": 4}),
        ),
        (Answer::YesNo(YesNoChoice(true)), json!({"YesNo": true})),
        (
            Answer::ChooseCards(CardsChoice(vec![ObjectId(1)])),
            json!({"ChooseCards": [1]}),
        ),
        (
            Answer::Priority(PriorityAction::Activate(AbilityRef {
                object: ObjectId(3),
                ability: 1,
            })),
            json!({"Priority": {"Activate": {"object": 3, "ability": 1}}}),
        ),
    ];
    for (answer, wire) in shapes {
        assert_eq!(serde_json::to_value(&answer).unwrap(), wire);
        assert_eq!(serde_json::from_value::<Answer>(wire).unwrap(), answer);
    }
}

#[test]
fn what_a_vanilla_message_never_carried_stays_out_of_it() {
    let priority = PriorityOptions {
        playable_lands: vec![ObjectId(1)],
        castable: Vec::new(),
        mana_abilities: Vec::new(),
        abilities: Vec::new(),
        can_act: true,
        can_pass: true,
    };
    let json = serde_json::to_value(&priority).unwrap();
    assert!(json.get("abilities").is_none(), "{json}");
    let with = PriorityOptions {
        abilities: vec![AbilityRef {
            object: ObjectId(2),
            ability: 0,
        }],
        ..priority
    };
    assert_eq!(
        serde_json::to_value(&with).unwrap()["abilities"],
        json!([{"object": 2, "ability": 0}])
    );
    // A decision read from a server that predates the field has none.
    let old: PriorityOptions = serde_json::from_value(json).unwrap();
    assert_eq!(
        old.abilities,
        Vec::<mtg_engine::abilities::AbilityRef>::new()
    );
}
