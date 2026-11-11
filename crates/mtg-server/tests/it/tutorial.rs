//! Goal: `POST /api/tutorial` makes a running, unlisted game in which the scripted bot plays the lesson the same way every time: a player who follows the lesson (land, Grizzly Bears, attack, Hero in Training and its trigger, block with Hero and Giant Growth, Isamaru from the command zone, attack) wins on their fifth turn, and the bot's answers are fixed by the decision alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ClientMessage, ServerMessage, StateView};
use mtg_server::tutorial::{self, BOT_LIFE, BOT_SEAT, PLAYER_SEAT};
use net::{TestServer, windowed_config};

const ME: PlayerId = PLAYER_SEAT;

/// The name of object `id` in `view`.
fn name_of(view: &StateView, id: mtg_core::ids::ObjectId) -> &str {
    view.objects
        .iter()
        .find(|object| object.id == id)
        .map_or("", |object| object.name.as_str())
}

/// The player's own creature called `name` on the battlefield.
fn mine(view: &StateView, name: &str) -> Option<mtg_core::ids::ObjectId> {
    view.objects
        .iter()
        .find(|object| {
            object.zone == Zone::Battlefield && object.controller == ME && object.name == name
        })
        .map(|object| object.id)
}

/// What a player who follows the lesson answers: a land first, the creature spells (Hero in Training only after
/// combat), attack with everything, block with Hero in Training (the untapped one), Giant Growth on it, the engine's payment.
fn learner(decision: &Decision, view: &StateView) -> Answer {
    let step = view.table.turn.step;
    match &decision.kind {
        DecisionKind::Priority(options) => {
            if let Some(&land) = options.playable_lands.first() {
                return Answer::Priority(PriorityAction::PlayLand(land));
            }
            let named = |name: &str| {
                options
                    .castable
                    .iter()
                    .copied()
                    .find(|&id| name_of(view, id) == name)
            };
            let blocking = view.table.turn.active == BOT_SEAT
                && !view.table.combat.blockers_declared.is_empty();
            let pick = named("Grizzly Bears")
                .or_else(|| {
                    named("Isamaru, Hound of Konda").filter(|_| view.table.turn.number >= 5)
                })
                .or_else(|| named("Hero in Training").filter(|_| step == Step::Main2))
                .or_else(|| {
                    named("Giant Growth").filter(|_| step == Step::DeclareBlockers && blocking)
                });
            Answer::Priority(pick.map_or(PriorityAction::Pass, PriorityAction::CastSpell))
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(
            options
                .attackers
                .iter()
                .map(|option| (option.object, AttackTarget::Player(BOT_SEAT)))
                .collect(),
        ),
        DecisionKind::DeclareBlockers(options) => Answer::DeclareBlockers(
            options
                .blockers
                .iter()
                .filter(|option| Some(option.object) == mine(view, "Hero in Training"))
                .filter_map(|option| {
                    option
                        .can_block
                        .first()
                        .map(|&attacker| (option.object, attacker))
                })
                .collect(),
        ),
        DecisionKind::ChooseTargets(options) => {
            let blocker = mine(view, "Hero in Training");
            let target = options
                .legal
                .iter()
                .find(
                    |target| matches!(target, Target::Object(object) if Some(object.id) == blocker),
                )
                .or_else(|| options.legal.first())
                .copied();
            Answer::ChooseTargets(TargetChoice::Targets(target.into_iter().collect()))
        }
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
        _ => crate::common::passive(decision),
    }
}

/// Plays the lesson as the player over a real socket and returns the result, the turn it came on and the bot's life
/// after each of the player's turns.
async fn play_lesson(server: &TestServer) -> (GameResult, u32, Vec<i32>, StateView) {
    let reply = net::post_json(server.addr, "/api/tutorial", "").await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    let game = body["game"].as_str().unwrap().to_owned();
    assert_eq!(body["seat"], 0);
    let mut client = net::connect(server.addr, &game, body["token"].as_str().unwrap()).await;
    assert_eq!(client.welcome(&game).await, ME);
    let mut view: Option<StateView> = None;
    let mut lives = Vec::new();
    let mut seen_turn = 0;
    loop {
        match client.message().await {
            ServerMessage::Snapshot { state, .. } => view = Some(*state),
            ServerMessage::Events(batch) => {
                let current = view.as_mut().expect("a snapshot comes first");
                current.apply(&batch);
                let turn = current.table.turn.number;
                if turn != seen_turn && current.table.turn.active == BOT_SEAT {
                    lives.push(current.table.players[BOT_SEAT.index()].life);
                }
                seen_turn = turn;
            }
            ServerMessage::Decision { id, decision, .. } => {
                assert_eq!(decision.player, ME);
                let answer = learner(&decision, view.as_ref().unwrap());
                client
                    .send(&ClientMessage::Answer {
                        decision: id,
                        answer,
                    })
                    .await;
            }
            ServerMessage::GameOver { result } => {
                let view = view.unwrap();
                return (result, view.table.turn.number, lives, view);
            }
            ServerMessage::Error { code, message } => panic!("refused: {code:?} {message}"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn a_player_who_follows_the_lesson_wins_on_turn_five_every_time() {
    let server = TestServer::start(windowed_config()).await;
    let mut runs = Vec::new();
    for _ in 0..2 {
        let (result, turn, lives, view) = play_lesson(&server).await;
        assert_eq!(result, GameResult::Winner(ME));
        assert_eq!(turn, 5);
        // The bot starts at its lesson life, the Bears' attack on turn 3 takes 2 and turn 5 takes the rest.
        assert_eq!(lives.first(), Some(&BOT_LIFE));
        assert_eq!(lives.get(1), Some(&(BOT_LIFE - 2)));
        // Hero in Training survived the block thanks to Giant Growth; the Piker did not; Isamaru left the command zone.
        assert!(mine(&view, "Grizzly Bears").is_some());
        assert!(mine(&view, "Hero in Training").is_some());
        assert!(mine(&view, "Isamaru, Hound of Konda").is_some());
        let piker = view
            .objects
            .iter()
            .find(|object| object.name == "Goblin Piker")
            .unwrap();
        assert_eq!(piker.zone, Zone::Graveyard(BOT_SEAT));
        runs.push((turn, lives));
    }
    assert_eq!(runs[0], runs[1], "the lesson runs the same way twice");
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
fn the_setup_starts_in_the_players_first_main_phase_with_the_lesson_cards() {
    let setup = tutorial::setup().unwrap();
    assert_eq!(setup.players.len(), 2);
    let player = &setup.players[ME.index()];
    let names: Vec<&str> = player.hand.iter().map(|card| card.name.as_str()).collect();
    assert_eq!(
        names,
        ["Forest", "Plains", "Grizzly Bears", "Hero in Training"]
    );
    assert_eq!(player.commanders[0].name, "Isamaru, Hound of Konda");
    assert_eq!(player.library[1].name, "Giant Growth");
    assert_eq!(setup.players[BOT_SEAT.index()].life, Some(BOT_LIFE));
}
