//! Goal: `POST /api/tutorial` makes a running, unlisted game in which the player meets exactly the decisions of the lesson's script, in order and on the bot's turns too, each scripted action is legal and affordable when it is asked, a player who follows it wins on their fifth turn every time, and the web lesson's trace (`web/src/features/learn/tutorialTrace.json`) is the one this game plays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ClientMessage, ServerMessage, StateView, Stop, StopSide};
use mtg_server::tutorial::{self, BOT_LIFE, BOT_SEAT, PLAYER_SEAT};
use net::{TestServer, windowed_config};
use serde_json::{Value, json};

const ME: PlayerId = PLAYER_SEAT;

/// What the script does at one decision.
#[derive(Debug, Clone, Copy)]
enum Act {
    Land(&'static str),
    Cast(&'static str),
    Pass,
    PayAuto,
    Attack(&'static [&'static str]),
    Block(&'static str, &'static str),
    Target(&'static str),
}

/// The decision kinds the script meets, by their wire name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Priority,
    PayMana,
    DeclareAttackers,
    DeclareBlockers,
    ChooseTargets,
}

/// The lesson, one line per decision the player is sent: the turn, the step, the kind and what the learner does.
const SCRIPT: &[(u32, Step, Kind, Act)] = &[
    (1, Step::Main1, Kind::Priority, Act::Land("Forest")),
    (1, Step::Main1, Kind::Priority, Act::Cast("Grizzly Bears")),
    (1, Step::Main1, Kind::PayMana, Act::PayAuto),
    (1, Step::Main2, Kind::Priority, Act::Pass),
    (3, Step::Main1, Kind::Priority, Act::Land("Plains")),
    (3, Step::Main1, Kind::Priority, Act::Pass),
    (
        3,
        Step::DeclareAttackers,
        Kind::DeclareAttackers,
        Act::Attack(&["Grizzly Bears"]),
    ),
    (3, Step::DeclareAttackers, Kind::Priority, Act::Pass),
    (
        3,
        Step::Main2,
        Kind::Priority,
        Act::Cast("Hero in Training"),
    ),
    (3, Step::Main2, Kind::PayMana, Act::PayAuto),
    (3, Step::Main2, Kind::Priority, Act::Pass),
    (3, Step::Main2, Kind::Priority, Act::Pass),
    (3, Step::Main2, Kind::Priority, Act::Pass),
    (4, Step::DeclareAttackers, Kind::Priority, Act::Pass),
    (
        4,
        Step::DeclareBlockers,
        Kind::DeclareBlockers,
        Act::Block("Hero in Training", "Dregscape Zombie"),
    ),
    (
        4,
        Step::DeclareBlockers,
        Kind::Priority,
        Act::Cast("Giant Growth"),
    ),
    (
        4,
        Step::DeclareBlockers,
        Kind::ChooseTargets,
        Act::Target("Hero in Training"),
    ),
    (4, Step::DeclareBlockers, Kind::PayMana, Act::PayAuto),
    (5, Step::Main1, Kind::Priority, Act::Land("Forest")),
    (
        5,
        Step::Main1,
        Kind::Priority,
        Act::Cast("Giada, Font of Hope"),
    ),
    (5, Step::Main1, Kind::PayMana, Act::PayAuto),
    (
        5,
        Step::DeclareAttackers,
        Kind::DeclareAttackers,
        Act::Attack(&["Grizzly Bears", "Hero in Training"]),
    ),
    (5, Step::DeclareAttackers, Kind::Priority, Act::Pass),
];

/// The kind of `decision`, if the script knows it.
fn kind_of(decision: &Decision) -> Option<Kind> {
    Some(match &decision.kind {
        DecisionKind::Priority(_) => Kind::Priority,
        DecisionKind::PayMana(_) => Kind::PayMana,
        DecisionKind::DeclareAttackers(_) => Kind::DeclareAttackers,
        DecisionKind::DeclareBlockers(_) => Kind::DeclareBlockers,
        DecisionKind::ChooseTargets(_) => Kind::ChooseTargets,
        _ => return None,
    })
}

/// The name of object `id` in `view`.
fn name_of(view: &StateView, id: ObjectId) -> &str {
    view.objects
        .iter()
        .find(|object| object.id == id)
        .map_or("", |object| object.name.as_str())
}

/// The first of `ids` called `name`, which the script requires to be there.
fn named(view: &StateView, ids: &[ObjectId], name: &str, what: &str) -> ObjectId {
    ids.iter()
        .copied()
        .find(|&id| name_of(view, id) == name)
        .unwrap_or_else(|| panic!("{name} is not among the {what} offered: {ids:?}"))
}

/// The player's creature called `name` on the battlefield.
fn mine(view: &StateView, name: &str) -> ObjectId {
    let found = view.objects.iter().find(|object| {
        object.zone == Zone::Battlefield && object.controller == ME && object.name == name
    });
    found.map_or_else(
        || panic!("{name} is not on the player's battlefield"),
        |object| object.id,
    )
}

/// The answer `act` gives to `decision`, asserting the action is legal now: a land among the playable ones, a spell
/// among the castable ones (castable means payable with the mana available), an attacker, blocker or target among the
/// offered ones, and an automatic payment the game found.
fn answer(act: Act, decision: &Decision, view: &StateView) -> Answer {
    match (act, &decision.kind) {
        (Act::Land(name), DecisionKind::Priority(options)) => Answer::Priority(
            PriorityAction::PlayLand(named(view, &options.playable_lands, name, "lands")),
        ),
        (Act::Cast(name), DecisionKind::Priority(options)) => Answer::Priority(
            PriorityAction::CastSpell(named(view, &options.castable, name, "castable spells")),
        ),
        (Act::Pass, DecisionKind::Priority(_)) => Answer::Priority(PriorityAction::Pass),
        (Act::PayAuto, DecisionKind::PayMana(options)) => {
            assert!(options.auto_available, "no automatic payment");
            Answer::PayMana(PayChoice::Auto)
        }
        (Act::Attack(names), DecisionKind::DeclareAttackers(options)) => {
            let ids: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
            Answer::DeclareAttackers(
                names
                    .iter()
                    .map(|name| {
                        (
                            named(view, &ids, name, "attackers"),
                            AttackTarget::Player(BOT_SEAT),
                        )
                    })
                    .collect(),
            )
        }
        (Act::Block(blocker, attacker), DecisionKind::DeclareBlockers(options)) => {
            let id = mine(view, blocker);
            let option = options
                .blockers
                .iter()
                .find(|option| option.object == id)
                .expect("the blocker is offered");
            let attacker = named(view, &option.can_block, attacker, "attackers to block");
            Answer::DeclareBlockers(vec![(id, attacker)])
        }
        (Act::Target(name), DecisionKind::ChooseTargets(options)) => {
            let id = mine(view, name);
            let target = options
                .legal
                .iter()
                .find(|target| matches!(target, Target::Object(object) if object.id == id))
                .copied()
                .expect("the target is legal");
            Answer::ChooseTargets(TargetChoice::Targets(vec![target]))
        }
        (act, kind) => panic!("the script's {act:?} cannot answer {kind:?}"),
    }
}

/// Names and ids of `objects`, as the web lesson reads them.
fn named_list<'a>(objects: impl Iterator<Item = &'a mtg_protocol::ObjectView>) -> Value {
    objects
        .map(|object| json!({ "name": object.name, "id": object.id.0 }))
        .collect()
}

/// The lesson's facts of `view` (`lessonFacts` in `web/src/features/learn/lesson.ts`), with the pending decision.
fn facts(view: &StateView, decision: Option<Kind>, result: Option<&GameResult>) -> Value {
    let own = || view.objects.iter().filter(|object| object.controller == ME);
    let field = |kind: CardType, mine: bool| {
        named_list(view.objects.iter().filter(move |object| {
            object.zone == Zone::Battlefield
                && (object.controller == ME) == mine
                && object.types.contains(&kind)
        }))
    };
    let zone = |zone: Zone| {
        named_list(own().filter(move |object| object.zone == zone && object.owner == ME))
    };
    let ids: Vec<ObjectId> = own().map(|object| object.id).collect();
    let attackers = &view.table.combat.attackers;
    let stack: Vec<Value> = view
        .table
        .stack
        .cards
        .iter()
        .flatten()
        .map(|&id| {
            let controller = view
                .objects
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.controller);
            json!({ "name": name_of(view, id), "id": id.0, "mine": controller == Some(ME) })
        })
        .collect();
    json!({
        "turn": view.table.turn.number,
        "mine": view.table.turn.active == ME,
        "step": view.table.turn.step,
        "decision": decision.map(|kind| format!("{kind:?}")),
        "hand": zone(Zone::Hand(ME)),
        "lands": field(CardType::Land, true),
        "creatures": field(CardType::Creature, true),
        "theirCreatures": field(CardType::Creature, false),
        "command": zone(Zone::Command(ME)),
        "stack": stack,
        "attacking": attackers.iter().filter(|a| ids.contains(&a.object)).count(),
        "blocking": attackers.iter().map(|a| a.blockers.iter().filter(|b| ids.contains(b)).count()).sum::<usize>(),
        "attacked": attackers.iter().any(|a| !ids.contains(&a.object)),
        "theirLife": view.table.players[BOT_SEAT.index()].life,
        "byHand": 0,
        "over": result.is_some(),
        "won": result == Some(&GameResult::Winner(ME)),
    })
}

/// What the web lesson needs of an action: its kind and the card it names.
fn act_json(act: Act) -> Value {
    match act {
        Act::Land(card) => json!({ "act": "Land", "cards": [card] }),
        Act::Cast(card) => json!({ "act": "Cast", "cards": [card] }),
        Act::Pass => json!({ "act": "Pass", "cards": [] }),
        Act::PayAuto => json!({ "act": "PayAuto", "cards": [] }),
        Act::Attack(cards) => json!({ "act": "Attack", "cards": cards }),
        Act::Block(blocker, attacker) => json!({ "act": "Block", "cards": [blocker, attacker] }),
        Act::Target(card) => json!({ "act": "Target", "cards": [card] }),
    }
}

/// Plays the script as the player over a real socket and returns the result, the final view and the trace: the facts
/// at each decision with the scripted action, then the facts at the end.
async fn play_script(server: &TestServer) -> (GameResult, StateView, Vec<Value>) {
    let reply = net::post_json(server.addr, "/api/tutorial", "").await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    let game = body["game"].as_str().unwrap().to_owned();
    assert_eq!(body["seat"], 0);
    let mut client = net::connect(server.addr, &game, body["token"].as_str().unwrap()).await;
    assert_eq!(client.welcome(&game).await, ME);
    let mut view: Option<StateView> = None;
    let mut trace = Vec::new();
    let mut script = SCRIPT.iter();
    loop {
        match client.message().await {
            ServerMessage::Snapshot { state, .. } => view = Some(*state),
            ServerMessage::Events(batch) => view.as_mut().expect("a snapshot first").apply(&batch),
            ServerMessage::Decision { id, decision, .. } => {
                assert_eq!(decision.player, ME);
                let current = view.as_ref().unwrap();
                let (turn, step) = (current.table.turn.number, current.table.turn.step);
                let kind = kind_of(&decision);
                let &(want_turn, want_step, want_kind, act) = script.next().unwrap_or_else(|| {
                    panic!("an unscripted decision on turn {turn} {step:?}: {decision:?}")
                });
                assert_eq!(
                    (turn, step, kind),
                    (want_turn, want_step, Some(want_kind)),
                    "the decision the script waits for at line {}",
                    trace.len()
                );
                let mut entry = act_json(act);
                entry["facts"] = facts(current, kind, None);
                trace.push(entry);
                let answer = answer(act, &decision, current);
                client
                    .send(&ClientMessage::Answer {
                        decision: id,
                        answer,
                    })
                    .await;
            }
            ServerMessage::GameOver { result } => {
                assert!(script.next().is_none(), "the game ended before the script");
                let view = view.unwrap();
                trace.push(
                    json!({ "act": null, "cards": [], "facts": facts(&view, None, Some(&result)) }),
                );
                return (result, view, trace);
            }
            ServerMessage::Error { code, message } => panic!("refused: {code:?} {message}"),
            _ => {}
        }
    }
}

/// The web lesson's copy of the trace.
const TRACE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/src/features/learn/tutorialTrace.json"
);

#[tokio::test]
async fn the_player_meets_the_scripted_decisions_and_wins_on_turn_five_every_time() {
    let server = TestServer::start(windowed_config()).await;
    let (result, view, trace) = play_script(&server).await;
    assert_eq!(result, GameResult::Winner(ME));
    assert_eq!(view.table.turn.number, 5);
    // Hero in Training survived the block thanks to Giant Growth; the Zombie did not; Giada left the command zone.
    for name in ["Grizzly Bears", "Hero in Training", "Giada, Font of Hope"] {
        mine(&view, name);
    }
    let zombie = view
        .objects
        .iter()
        .find(|object| object.name == "Dregscape Zombie")
        .unwrap();
    assert_eq!(zombie.zone, Zone::Graveyard(BOT_SEAT));
    let (_, _, again) = play_script(&server).await;
    assert_eq!(trace, again, "the lesson runs the same way twice");

    let text = serde_json::to_string_pretty(&trace).unwrap() + "\n";
    if std::env::var_os("MTG_UPDATE_BINDINGS").is_some() {
        std::fs::write(TRACE_PATH, &text).unwrap();
    }
    // Compared as data, so the web's formatter may lay the file out its own way.
    let stored: Value = std::fs::read_to_string(TRACE_PATH)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    assert!(
        stored == Value::Array(trace),
        "the web lesson's trace is stale: run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-server tutorial::, then pnpm format in web/"
    );
}

#[tokio::test]
async fn the_player_holds_the_lessons_stops_from_the_start() {
    let server = TestServer::start(windowed_config()).await;
    let body = net::post_json(server.addr, "/api/tutorial", "")
        .await
        .json();
    let game = body["game"].as_str().unwrap();
    let mut client = net::connect(server.addr, game, body["token"].as_str().unwrap()).await;
    client.join(game).await;
    let ServerMessage::Welcome { stops, .. } = client.message().await else {
        panic!("expected Welcome");
    };
    let want: Vec<Stop> = tutorial::PLAYER_STOPS
        .iter()
        .map(|&step| Stop {
            side: StopSide::Own,
            step,
            keep: true,
        })
        .collect();
    assert_eq!(stops, want);
}

#[tokio::test]
async fn a_tutorial_game_is_never_listed_and_needs_no_account() {
    let server = TestServer::start(windowed_config()).await;
    let reply = net::post_json(server.addr, "/api/tutorial", "").await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    let keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["game", "seat", "token"]);
    let lobbies = net::get(server.addr, "/api/lobbies").await.json();
    assert_eq!(lobbies["games"].as_array().unwrap().len(), 0);
}

#[test]
fn every_card_of_the_lesson_is_a_real_card_with_its_printing() {
    let setup = tutorial::setup().unwrap();
    assert_eq!(setup.players.len(), 2);
    let player = &setup.players[ME.index()];
    let names: Vec<&str> = player.hand.iter().map(|card| card.name.as_str()).collect();
    assert_eq!(names, ["Forest", "Grizzly Bears", "Hero in Training"]);
    assert_eq!(player.commanders[0].name, "Giada, Font of Hope");
    assert_eq!(player.library[0].name, "Plains");
    assert_eq!(player.library[1].name, "Giant Growth");
    assert_eq!(setup.players[BOT_SEAT.index()].life, Some(BOT_LIFE));
    for seat in &setup.players {
        let cards = seat
            .hand
            .iter()
            .chain(&seat.library)
            .chain(&seat.commanders)
            .chain(seat.battlefield.iter().map(|permanent| &permanent.card));
        for card in cards {
            assert!(
                card.identity
                    .as_ref()
                    .is_some_and(|identity| identity.printing_id.is_some()),
                "{} has no printed identity",
                card.name
            );
            assert!(card.manual.is_empty(), "{} is played by hand", card.name);
        }
    }
}
