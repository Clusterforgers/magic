//! Goal: `POST /api/tutorial` starts each lesson of the course as an unlisted game in which the learner meets exactly the decisions of the lesson's script, in order and on the bot's turns too, each scripted act is legal when it is asked, the game ends where the lesson says the same way every time, and the web's traces (`web/src/features/learn/traces/<id>.json`) are the ones these games play.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{MulliganChoice, PayChoice, PriorityAction};
use mtg_engine::mana::Payment;
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{
    ClientMessage, FullControlMode, ServerMessage, StateView, Stop, StopSide, YieldUntil,
};
use mtg_server::tutorial::{
    self, Act, BOT_SEAT, COMBAT_BOT_LIFE, COMMANDER, GUEST_NAME, LESSONS, Lesson, PLAYER_SEAT,
};
use net::{TestServer, windowed_config};
use serde_json::{Value, json};

const ME: PlayerId = PLAYER_SEAT;

/// The web's `decisionName` of `decision`: its `DecisionKind` variant.
fn kind_name(decision: &Decision) -> String {
    match serde_json::to_value(&decision.kind).unwrap() {
        Value::String(name) => name,
        Value::Object(map) => map.keys().next().cloned().unwrap_or_default(),
        other => panic!("a decision kind serializes as {other}"),
    }
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

/// The learner's permanent called `name`.
fn mine(view: &StateView, name: &str) -> ObjectId {
    let found = view.objects.iter().find(|object| {
        object.zone == Zone::Battlefield && object.controller == ME && object.name == name
    });
    found.map_or_else(
        || panic!("{name} is not on the learner's battlefield"),
        |object| object.id,
    )
}

/// Whether an object called `name` is in `zone`.
fn has(view: &StateView, zone: Zone, name: &str) -> bool {
    view.objects
        .iter()
        .any(|object| object.zone == zone && object.name == name)
}

/// The answer `act` gives to `decision`, asserting it is legal now: a land among the playable ones, a spell among
/// the castable ones (castable means payable), sources, attackers, blockers and targets among the offered ones, an
/// automatic payment the game found.
fn answer(act: Act, decision: &Decision, view: &StateView) -> Answer {
    match (act, &decision.kind) {
        (Act::Keep, DecisionKind::Mulligan(_)) => Answer::Mulligan(MulliganChoice::Keep),
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
        (Act::PayTap(names), DecisionKind::PayMana(options)) => {
            let mut taps = Vec::new();
            for name in names {
                let source = options
                    .sources
                    .iter()
                    .find(|option| {
                        name_of(view, option.source.object) == *name
                            && !taps.iter().any(|tapped: &mtg_engine::mana::ManaSource| {
                                tapped.object == option.source.object
                            })
                    })
                    .unwrap_or_else(|| panic!("no untapped {name} to tap"));
                taps.push(source.source);
            }
            Answer::PayMana(PayChoice::Manual(Payment {
                taps,
                ..Payment::default()
            }))
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
        (Act::Choose(yes), DecisionKind::YesNo(options)) => {
            assert!(!(yes && options.only_no), "Yes is not an answer here");
            Answer::YesNo(YesNoChoice(yes))
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

/// The lesson facts of `view` with the pending decision's kind and the learner's stops.
fn facts(
    view: &StateView,
    decision: Option<&str>,
    stops: &[Stop],
    result: Option<&GameResult>,
) -> Value {
    let field = |kind: CardType, mine: bool| {
        named_list(view.objects.iter().filter(move |object| {
            object.zone == Zone::Battlefield
                && (object.controller == ME) == mine
                && object.types.contains(&kind)
        }))
    };
    let zone = |zone: Zone| {
        named_list(
            view.objects
                .iter()
                .filter(move |object| object.zone == zone),
        )
    };
    let ids: Vec<ObjectId> = view
        .objects
        .iter()
        .filter(|object| object.controller == ME)
        .map(|object| object.id)
        .collect();
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
    let stops: Vec<Value> = stops
        .iter()
        .map(|stop| json!({ "side": stop.side, "step": stop.step }))
        .collect();
    json!({
        "turn": view.table.turn.number,
        "mine": view.table.turn.active == ME,
        "step": view.table.turn.step,
        "decision": decision,
        "hand": zone(Zone::Hand(ME)),
        "lands": field(CardType::Land, true),
        "creatures": field(CardType::Creature, true),
        "theirCreatures": field(CardType::Creature, false),
        "command": zone(Zone::Command(ME)),
        "graveyard": zone(Zone::Graveyard(ME)),
        "theirGraveyard": zone(Zone::Graveyard(BOT_SEAT)),
        "stack": stack,
        "attacking": attackers.iter().filter(|a| ids.contains(&a.object)).count(),
        "blocking": attackers.iter().map(|a| a.blockers.iter().filter(|b| ids.contains(b)).count()).sum::<usize>(),
        "attacked": attackers.iter().any(|a| !ids.contains(&a.object)),
        "life": view.table.players[ME.index()].life,
        "theirLife": view.table.players[BOT_SEAT.index()].life,
        "stops": stops,
        "over": result.is_some(),
        "won": result == Some(&GameResult::Winner(ME)),
    })
}

/// What the web lesson needs of an act: its kind and the cards it names.
fn act_json(act: Act) -> Value {
    match act {
        Act::Keep => json!({ "act": "Keep", "cards": [] }),
        Act::Land(card) => json!({ "act": "Land", "cards": [card] }),
        Act::Cast(card) => json!({ "act": "Cast", "cards": [card] }),
        Act::Pass => json!({ "act": "Pass", "cards": [] }),
        Act::PayAuto => json!({ "act": "PayAuto", "cards": [] }),
        Act::PayTap(cards) => json!({ "act": "PayTap", "cards": cards }),
        Act::Attack(cards) => json!({ "act": "Attack", "cards": cards }),
        Act::Block(blocker, attacker) => json!({ "act": "Block", "cards": [blocker, attacker] }),
        Act::Target(card) => json!({ "act": "Target", "cards": [card] }),
        Act::Choose(yes) => json!({ "act": "Choose", "cards": [], "yes": yes }),
        Act::SetStop(side, step) => {
            json!({ "act": "SetStop", "cards": [], "stop": { "side": side, "step": step } })
        }
        Act::EndTurn => json!({ "act": "EndTurn", "cards": [] }),
    }
}

/// One run of a lesson: how it ended, the trace, and at each trace entry the view and the decision pending.
struct Run {
    result: Option<GameResult>,
    trace: Vec<Value>,
    views: Vec<StateView>,
    decisions: Vec<Option<Decision>>,
}

impl Run {
    /// The index of the first trace entry at `turn` and `step`.
    fn at(&self, turn: u32, step: Step) -> usize {
        self.views
            .iter()
            .position(|view| view.table.turn.number == turn && view.table.turn.step == step)
            .unwrap_or_else(|| panic!("no entry at turn {turn} {step:?}"))
    }

    fn last(&self) -> &StateView {
        self.views.last().unwrap()
    }

    /// Adds a trace entry: `entry` with `facts`, and the view and decision behind them.
    fn record(
        &mut self,
        mut entry: Value,
        facts: Value,
        view: StateView,
        decision: Option<Decision>,
    ) {
        entry["facts"] = facts;
        self.trace.push(entry);
        self.views.push(view);
        self.decisions.push(decision);
    }
}

/// Sends what `act` does at decision `id`; returns the stops the server holds after a `SetStop`, `None` for an act
/// that answers the decision.
async fn send_act(
    client: &mut net::WsClient,
    act: Act,
    id: mtg_protocol::DecisionId,
    decision: &Decision,
    view: &StateView,
) -> Option<Vec<Stop>> {
    let message = match act {
        Act::SetStop(side, step) => {
            let message = ClientMessage::SetStop {
                side,
                step,
                on: true,
                keep: true,
            };
            client.send(&message).await;
            loop {
                match client.message().await {
                    ServerMessage::Stops { stops } => return Some(stops),
                    ServerMessage::Error { code, message, .. } => {
                        panic!("refused: {code:?} {message}")
                    }
                    message @ (ServerMessage::Decision { .. }
                    | ServerMessage::Snapshot { .. }
                    | ServerMessage::Events(_)
                    | ServerMessage::GameOver { .. }) => {
                        panic!("expected Stops, got {message:?}")
                    }
                    _ => {}
                }
            }
        }
        Act::EndTurn => ClientMessage::Yield {
            until: YieldUntil::EndOfTurn,
        },
        act => ClientMessage::Answer {
            decision: id,
            answer: answer(act, decision, view),
        },
    };
    client.send(&message).await;
    None
}

/// Starts `lesson` (the first one for `None`) and returns the game and the seat token.
async fn start(server: &TestServer, lesson: Option<&str>) -> (String, String) {
    let body = lesson.map_or_else(String::new, |id| json!({ "lesson": id }).to_string());
    let reply = net::post_json(server.addr, "/api/tutorial", &body).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    assert_eq!(body["seat"], 0);
    (
        body["game"].as_str().unwrap().to_owned(),
        body["token"].as_str().unwrap().to_owned(),
    )
}

/// The line a decision that does not fit the script reports, with what arrived before it.
fn lost(lesson: &Lesson, arrived: &[String], now: &str) -> String {
    format!(
        "lesson {}: {now} does not fit line {} of the script; arrived so far:\n{}",
        lesson.id,
        arrived.len(),
        arrived.join("\n")
    )
}

/// Plays `lesson`'s script as the learner over a real socket: every decision sent to the learner must be the next
/// line's, and the run stops at the end of the game or at the first decision after the script.
async fn play(server: &TestServer, lesson: &Lesson) -> Run {
    let (game, token) = start(server, Some(lesson.id)).await;
    let mut client = net::connect(server.addr, &game, &token).await;
    client.join(&game).await;
    let mut stops = match client.message().await {
        ServerMessage::Welcome { player, stops, .. } => {
            assert_eq!(player, ME);
            stops
        }
        other => panic!("expected Welcome, got {other:?}"),
    };
    let mut view: Option<StateView> = None;
    let mut run = Run {
        result: None,
        trace: Vec::new(),
        views: Vec::new(),
        decisions: Vec::new(),
    };
    let mut arrived = Vec::new();
    let mut script = lesson.script.iter().peekable();
    loop {
        match client.message().await {
            ServerMessage::Snapshot { state, .. } => view = Some(*state),
            ServerMessage::Events(batch) => view.as_mut().expect("a snapshot first").apply(&batch),
            ServerMessage::Stops { stops: now } => stops = now,
            ServerMessage::Decision { id, decision, .. } => {
                assert_eq!(decision.player, ME);
                let kind = kind_name(&decision);
                let current = view.clone().expect("a snapshot first");
                let (turn, step) = (current.table.turn.number, current.table.turn.step);
                let now = format!("({turn}, {step:?}, {kind})");
                if script.peek().is_none() {
                    arrived.push(format!("{now} pending at the end"));
                    eprintln!("lesson {}:\n{}", lesson.id, arrived.join("\n"));
                    let facts = facts(&current, Some(&kind), &stops, None);
                    run.record(
                        json!({ "act": null, "cards": [] }),
                        facts,
                        current,
                        Some(decision),
                    );
                    return run;
                }
                loop {
                    let &(want_turn, want_step, want_kind, act) = script
                        .next()
                        .unwrap_or_else(|| panic!("{}", lost(lesson, &arrived, &now)));
                    assert!(
                        (turn, step, kind.as_str())
                            == (want_turn, want_step, format!("{want_kind:?}").as_str()),
                        "{}",
                        lost(lesson, &arrived, &now)
                    );
                    arrived.push(format!("{now} {act:?}"));
                    let facts = facts(&current, Some(&kind), &stops, None);
                    run.record(
                        act_json(act),
                        facts,
                        current.clone(),
                        Some(decision.clone()),
                    );
                    match send_act(&mut client, act, id, &decision, &current).await {
                        Some(now) => stops = now,
                        None => break,
                    }
                }
            }
            ServerMessage::GameOver { result } => {
                assert!(
                    script.next().is_none(),
                    "{}",
                    lost(lesson, &arrived, "the end of the game")
                );
                eprintln!("lesson {}:\n{}\n(game over)", lesson.id, arrived.join("\n"));
                let view = view.unwrap();
                let facts = facts(&view, None, &stops, Some(&result));
                run.record(json!({ "act": null, "cards": [] }), facts, view, None);
                run.result = Some(result);
                return run;
            }
            ServerMessage::Error { code, message, .. } => panic!("refused: {code:?} {message}"),
            _ => {}
        }
    }
}

/// The `turn` lesson: the bot's end step is asked only once the learner set its stop.
fn turn_ends(run: &Run) {
    let held = |index: usize| {
        run.trace[index]["facts"]["stops"]
            .as_array()
            .unwrap()
            .contains(&json!({ "side": StopSide::Opponent, "step": Step::End }))
    };
    assert_eq!(run.trace[0]["act"], "SetStop");
    assert!(!held(0), "the stop is not held before the learner sets it");
    assert!(held(1), "the stop is held once set");
    let index = run.at(6, Step::End);
    assert_eq!(
        run.trace[index]["act"], "Pass",
        "asked at the bot's end step"
    );
}

/// The `commander` lesson: the return to the command zone is asked and the second cast pays the tax.
fn commander_ends(run: &Run) {
    let asked = run
        .trace
        .iter()
        .any(|entry| entry["facts"]["decision"] == "YesNo" && entry["act"] == "Choose");
    assert!(asked, "the return to the command zone is asked");
    let recast = run.at(7, Step::Main1);
    assert!(has(&run.views[recast], Zone::Command(ME), COMMANDER));
    let pays: Vec<String> = run
        .decisions
        .iter()
        .flatten()
        .filter_map(|decision| match &decision.kind {
            DecisionKind::PayMana(options) => Some(options.cost.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(pays, ["{1}{W}", "{3}{W}"], "the second cast pays the tax");
    mine(run.last(), COMMANDER);
}

/// Asserts the end state each lesson names (`docs/tutorial.md`, the lesson's step table).
fn ends_where_the_lesson_says(lesson: &Lesson, run: &Run) {
    let last = run.last();
    let (turn, step) = (last.table.turn.number, last.table.turn.step);
    let pending = run.decisions.last().unwrap().as_ref().map(kind_name);
    match lesson.id {
        "seat" => {
            assert_eq!((turn, pending.as_deref()), (3, Some("Priority")));
            mine(last, "Forest");
            assert!(
                has(last, Zone::Battlefield, "Swamp"),
                "the bot played a Swamp"
            );
        }
        "mana" => {
            assert_eq!((turn, pending.as_deref()), (5, Some("Priority")));
            mine(last, "Grizzly Bears");
            mine(last, "Llanowar Elves");
            assert!(has(last, Zone::Battlefield, "Dregscape Zombie"));
        }
        "combat" => {
            assert_eq!(run.result, Some(GameResult::Winner(ME)));
            assert_eq!(turn, 7);
            let hit = &run.views[run.at(5, Step::Main2)];
            assert_eq!(
                hit.table.players[BOT_SEAT.index()].life,
                COMBAT_BOT_LIFE - 2,
                "the Bears hit on turn 5"
            );
            let attacked = &run.views[run.at(6, Step::DeclareBlockers)];
            let theirs = attacked
                .table
                .combat
                .attackers
                .iter()
                .filter(|a| name_of(attacked, a.object) == "Dregscape Zombie")
                .count();
            assert_eq!(theirs, 2, "the bot attacks with both Zombies");
            assert!(has(last, Zone::Graveyard(BOT_SEAT), "Dregscape Zombie"));
        }
        "stack" => {
            assert_eq!((turn, pending.as_deref()), (7, Some("Priority")));
            for name in ["Grizzly Bears", "Hero in Training"] {
                mine(last, name);
            }
            assert!(has(last, Zone::Graveyard(BOT_SEAT), "Dregscape Zombie"));
            assert!(has(last, Zone::Graveyard(ME), "Giant Growth"));
        }
        "turn" => {
            turn_ends(run);
            assert_eq!(
                (turn, step, pending.as_deref()),
                (7, Step::Main1, Some("Priority"))
            );
        }
        "commander" => commander_ends(run),
        "help" => {
            assert_eq!(
                (turn, step, pending.as_deref()),
                (5, Step::Main1, Some("Priority"))
            );
            assert!(run.result.is_none());
            for name in ["Serra Angel", "Grizzly Bears"] {
                mine(last, name);
            }
        }
        other => panic!("no end state for lesson {other}"),
    }
}

/// The web's copy of a lesson's trace.
fn trace_path(id: &str) -> String {
    format!(
        "{}/../../web/src/features/learn/traces/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[tokio::test]
async fn every_lesson_meets_its_scripted_decisions_and_ends_where_it_says_every_time() {
    let server = std::sync::Arc::new(TestServer::start(windowed_config()).await);
    let mut stale = Vec::new();
    let mut failed = Vec::new();
    for lesson in &LESSONS {
        // Each lesson in a task of its own, so one that goes astray still lets the others report.
        let lesson = tutorial::lesson(lesson.id).unwrap();
        let shared = std::sync::Arc::clone(&server);
        let task = tokio::spawn(async move {
            let run = play(&shared, lesson).await;
            ends_where_the_lesson_says(lesson, &run);
            let again = play(&shared, lesson).await;
            assert_eq!(
                run.trace, again.trace,
                "lesson {} runs the same way twice",
                lesson.id
            );
            run
        });
        let Ok(run) = task.await else {
            failed.push(lesson.id);
            continue;
        };

        let path = trace_path(lesson.id);
        let text = serde_json::to_string_pretty(&run.trace).unwrap() + "\n";
        if std::env::var_os("MTG_UPDATE_BINDINGS").is_some() {
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        // Compared as data, so the web's formatter may lay the file out its own way.
        let stored: Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        if stored != Value::Array(run.trace) {
            stale.push(lesson.id);
        }
    }
    assert!(failed.is_empty(), "lessons {failed:?} went astray");
    assert!(
        stale.is_empty(),
        "the web's traces of {stale:?} are stale: run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-server tutorial::, then pnpm format in web/"
    );
}

#[tokio::test]
async fn each_lesson_starts_with_its_stops_full_control_off_and_no_automatic_payment() {
    let server = TestServer::start(windowed_config()).await;
    for lesson in &LESSONS {
        let (game, token) = start(&server, Some(lesson.id)).await;
        let mut client = net::connect(server.addr, &game, &token).await;
        client.join(&game).await;
        let ServerMessage::Welcome {
            stops,
            full_control,
            auto_choices,
            ..
        } = client.message().await
        else {
            panic!("expected Welcome");
        };
        let mut want: Vec<Stop> = lesson
            .stops
            .iter()
            .map(|&(side, step)| Stop {
                side,
                step,
                keep: true,
            })
            .collect();
        want.sort_by_key(|stop| (stop.side, stop.step));
        let mut held = stops.clone();
        held.sort_by_key(|stop| (stop.side, stop.step));
        assert_eq!(held, want, "lesson {}", lesson.id);
        assert_eq!(full_control, FullControlMode::Off);
        assert!(!auto_choices.auto_pay, "lesson {}", lesson.id);
    }
}

#[tokio::test]
async fn a_request_without_a_lesson_starts_the_first_and_an_unknown_one_is_refused() {
    let server = TestServer::start(windowed_config()).await;
    let (game, token) = start(&server, None).await;
    let mut client = net::connect(server.addr, &game, &token).await;
    assert_eq!(client.welcome(&game).await, ME);
    loop {
        if let ServerMessage::Decision { decision, .. } = client.message().await {
            assert_eq!(
                kind_name(&decision),
                "Mulligan",
                "the first lesson starts at the mulligan"
            );
            break;
        }
    }
    for body in [
        r#"{"lesson":"nope"}"#,
        r#"{"lesson":7}"#,
        "not json",
        r#"{"other":"seat"}"#,
    ] {
        let reply = net::post_json(server.addr, "/api/tutorial", body).await;
        assert_eq!(reply.status, 400, "{body}: {}", reply.text());
    }
    let reply = net::post_json(server.addr, "/api/tutorial", "{}").await;
    assert_eq!(reply.status, 201, "{}", reply.text());
}

#[tokio::test]
async fn a_tutorial_game_is_never_listed_and_its_learner_without_an_account_is_the_guest() {
    let server = TestServer::start(windowed_config()).await;
    for lesson in &LESSONS {
        let reply = net::post_json(
            server.addr,
            "/api/tutorial",
            &json!({ "lesson": lesson.id }).to_string(),
        )
        .await;
        assert_eq!(reply.status, 201, "{}", reply.text());
        let body = reply.json();
        let keys: Vec<&str> = body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["game", "seat", "token"]);
        let game = body["game"].as_str().unwrap();
        let info = net::get(server.addr, &format!("/api/games/{game}"))
            .await
            .json();
        assert_eq!(info["seats"][0]["name"], GUEST_NAME, "{info}");
        assert_eq!(info["seats"][1]["name"], tutorial::BOT_NAME, "{info}");
    }
    let lobbies = net::get(server.addr, "/api/lobbies").await.json();
    assert_eq!(lobbies["games"].as_array().unwrap().len(), 0);
}

#[test]
fn every_card_of_every_lesson_is_a_real_card_with_its_printing() {
    for lesson in &LESSONS {
        let setup = (lesson.setup)().unwrap();
        assert_eq!(setup.players.len(), 2, "lesson {}", lesson.id);
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
    let ids: Vec<&str> = LESSONS.iter().map(|lesson| lesson.id).collect();
    assert_eq!(
        ids,
        [
            "seat",
            "mana",
            "combat",
            "stack",
            "turn",
            "commander",
            "help"
        ]
    );
    assert!(tutorial::lesson("nope").is_none());
}
