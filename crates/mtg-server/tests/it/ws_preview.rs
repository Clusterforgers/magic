//! Goal: over real sockets a seat can preview its own pending block or attack and is answered with the engine's forecast, exactly once per preview and to that seat alone; stale, foreign, illegal and non-combat previews and previews over the rate limit get no forecast and no error; the bucket refills; a real answer sent right after a flood still goes through; and none of it changes the game's log, store or state hash.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use common::{Client, game_id, roomy};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::forecast::CombatForecast;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, EventView, ServerMessage};
use mtg_server::{
    Dwell, EventStore, GameConfig, GameHandle, MemoryStore, PREVIEW_BURST, PREVIEWS_PER_SECOND,
    Pace, ServerConfig, spawn_game_with,
};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Scenario, ready};
use net::{Frame, TestServer, WsClient, bot_config, connect};
use rstest::rstest;

/// Player 0 has a 4/4 wolf and a 2/2 imp; player 1 a 2/2 guard and a 3/3 bear and 6 life. The game
/// starts at player 0's declare attackers step.
fn duel() -> GameSetup {
    Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(free_creature("Wolf", 4, 4)),
                ready(free_creature("Imp", 2, 2)),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Guard", 2, 2)),
                ready(free_creature("Bear", 3, 3)),
            ],
        )
        .life(1, 6)
        .setup()
        .unwrap()
}

/// What the server sent that is not a decision, an event or chat: a forecast, or something that must not happen.
async fn forecast_of(client: &mut WsClient) -> (DecisionId, Option<CombatForecast>) {
    loop {
        match client.message().await {
            ServerMessage::Forecast { decision, forecast } => return (decision, forecast),
            ServerMessage::Error { code, message } => {
                panic!("a preview must never be an error: {code:?} {message}")
            }
            _ => {}
        }
    }
}

/// The next decision for the client, skipping everything else.
async fn next_decision(client: &mut WsClient) -> (DecisionId, mtg_engine::Decision) {
    loop {
        if let ServerMessage::Decision { id, decision, .. } = client.message().await {
            return (id, decision);
        }
    }
}

/// Waits (briefly) to be sure the client is sent no forecast and no error; news of the game may well be queued for it.
async fn no_forecast(client: &mut WsClient) {
    let until = Instant::now() + Duration::from_millis(250);
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match tokio::time::timeout(left, client.next()).await {
            Err(_) => return,
            Ok(Frame::Message(
                message @ (ServerMessage::Forecast { .. } | ServerMessage::Error { .. }),
            )) => panic!("unexpected message: {message:?}"),
            Ok(_) => {}
        }
    }
}

/// The first batch that carries the declaration of blockers.
async fn blocks_batch(client: &mut WsClient) -> mtg_protocol::EventBatch {
    loop {
        if let ServerMessage::Events(batch) = client.message().await {
            let declared = batch
                .events
                .iter()
                .any(|event| matches!(event, EventView::Public(Event::BlockersDeclared { .. })));
            if declared {
                return batch;
            }
        }
    }
}

/// Two seats on a real server, in a game registered from `duel()`.
struct Duel {
    /// Kept so the server runs for as long as the duel does.
    _server: TestServer,
    handle: GameHandle,
    a: WsClient,
    b: WsClient,
}

/// Where a duel has got to when the block decision of player 1 is pending.
struct Blocks {
    id: DecisionId,
    wolf: ObjectId,
    imp: ObjectId,
    guard: ObjectId,
    bear: ObjectId,
}

impl Duel {
    /// Both seats connected and ready, with every decision waiting for its player and no clock; player 0 faces the
    /// declare attackers decision.
    async fn start() -> Duel {
        Duel::start_with(bot_config()).await
    }

    /// Like [`Duel::start`] on a server with this configuration.
    async fn start_with(config: ServerConfig) -> Duel {
        let server = TestServer::start(config).await;
        let names = vec!["Ann".to_owned(), "Bo".to_owned()];
        let created = server.registry.register(duel(), names).await.unwrap();
        let game = created.id.to_string();
        let handle = server.registry.handle(created.id).unwrap();
        let mut seats = Vec::new();
        for (_, token) in &created.seats {
            let mut client = connect(server.addr, &game, token.expose()).await;
            client.welcome(&game).await;
            seats.push(client);
        }
        let mut b = seats.pop().unwrap();
        let mut a = seats.pop().unwrap();
        a.send(&ClientMessage::Ready).await;
        b.send(&ClientMessage::Ready).await;
        Duel {
            _server: server,
            handle,
            a,
            b,
        }
    }

    /// Player 0 attacks with both creatures; returns their ids and the priority decision player 0 then faces.
    async fn attack(&mut self) -> (ObjectId, ObjectId, DecisionId) {
        let (id, decision) = next_decision(&mut self.a).await;
        let DecisionKind::DeclareAttackers(options) = decision.kind else {
            panic!("declare attackers expected, got {:?}", decision.kind);
        };
        let (wolf, imp) = (options.attackers[0].object, options.attackers[1].object);
        let attacks = [wolf, imp]
            .into_iter()
            .map(|creature| {
                (
                    creature,
                    mtg_engine::combat::AttackTarget::Player(PlayerId(1)),
                )
            })
            .collect();
        self.a
            .send(&ClientMessage::Answer {
                decision: id,
                answer: Answer::DeclareAttackers(attacks),
            })
            .await;
        let (priority, decision) = next_decision(&mut self.a).await;
        assert!(matches!(decision.kind, DecisionKind::Priority(_)));
        (wolf, imp, priority)
    }

    /// Passes priority for both seats until player 1 must block.
    async fn reach_blocks(&mut self) -> Blocks {
        let (wolf, imp, priority) = self.attack().await;
        self.a
            .send(&ClientMessage::Answer {
                decision: priority,
                answer: Answer::Priority(PriorityAction::Pass),
            })
            .await;
        loop {
            let (id, decision) = next_decision(&mut self.b).await;
            match decision.kind {
                DecisionKind::Priority(_) => {
                    self.b
                        .send(&ClientMessage::Answer {
                            decision: id,
                            answer: Answer::Priority(PriorityAction::Pass),
                        })
                        .await;
                }
                DecisionKind::DeclareBlockers(options) => {
                    return Blocks {
                        id,
                        wolf,
                        imp,
                        guard: options.blockers[0].object,
                        bear: options.blockers[1].object,
                    };
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    async fn status(&self) -> mtg_server::GameStatus {
        self.handle.status().await.unwrap()
    }
}

fn preview(decision: DecisionId, answer: Answer) -> ClientMessage {
    ClientMessage::Preview { decision, answer }
}

fn block(pairs: &[(ObjectId, ObjectId)]) -> Answer {
    Answer::DeclareBlockers(pairs.to_vec())
}

#[tokio::test]
async fn a_seat_previews_its_own_block_and_the_game_does_not_move() {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let before = duel.status().await;

    duel.b
        .send(&preview(b.id, block(&[(b.guard, b.imp)])))
        .await;
    let (decision, forecast) = forecast_of(&mut duel.b).await;
    let forecast = forecast.expect("a forecast");
    assert_eq!(decision, b.id);

    // The guard and the imp trade, the wolf is unblocked and takes player 1 from 6 to 2.
    let dying: BTreeSet<_> = forecast.dying().collect();
    assert_eq!(dying, BTreeSet::from([b.imp, b.guard]));
    let [player] = forecast.players.as_slice() else {
        panic!("one defender expected: {:?}", forecast.players);
    };
    assert_eq!(
        (player.damage, player.life, player.life_after, player.lethal),
        (4, 6, 2, false)
    );

    let after = duel.status().await;
    assert_eq!(after.events, before.events, "the log grew");
    assert_eq!(after.state_hash, before.state_hash, "the state changed");
    assert_eq!(after.waiting_on, before.waiting_on);
    assert_eq!(after.waiting_on, Some(PlayerId(1)));
    no_forecast(&mut duel.a).await;
    no_forecast(&mut duel.b).await;
}

#[tokio::test]
async fn the_forecast_is_what_every_seat_sees_after_the_same_answer_is_given() {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let blocks = [(b.guard, b.wolf), (b.bear, b.wolf)];

    duel.b.send(&preview(b.id, block(&blocks))).await;
    let (_, previewed) = forecast_of(&mut duel.b).await;
    let previewed = previewed.expect("a forecast");
    // The double block kills the wolf (2 + 3 damage), which kills the guard (2 damage) and only hurts the bear.
    let dying: BTreeSet<_> = previewed.dying().collect();
    assert_eq!(dying, BTreeSet::from([b.wolf, b.guard]));

    duel.b
        .send(&ClientMessage::Answer {
            decision: b.id,
            answer: block(&blocks),
        })
        .await;
    for client in [&mut duel.a, &mut duel.b] {
        let batch = blocks_batch(client).await;
        assert_eq!(
            batch.table.combat_forecast.as_ref(),
            Some(&previewed),
            "combat is public: everyone sees what the preview showed"
        );
    }
}

#[tokio::test]
async fn at_the_default_pace_the_preview_is_still_the_damage_that_then_happens() {
    // Auto-pass on and the response clocks running: nobody here can act, so after the blocks the server passes for
    // both seats and the damage is dealt before the batch is cut.
    let config = ServerConfig {
        game: GameConfig {
            pace: Pace {
                dwell: Dwell::OFF,
                ..Pace::default()
            },
            ..bot_config().game
        },
        ..bot_config()
    };
    let mut duel = Duel::start_with(config).await;
    let (id, decision) = next_decision(&mut duel.a).await;
    let DecisionKind::DeclareAttackers(options) = decision.kind else {
        panic!("declare attackers expected");
    };
    let attacks = options
        .attackers
        .iter()
        .map(|option| (option.object, option.defenders[0]))
        .collect();
    duel.a
        .send(&ClientMessage::Answer {
            decision: id,
            answer: Answer::DeclareAttackers(attacks),
        })
        .await;

    // The next decision is the block itself, with its clock, as the idle priority rounds were passed for them.
    let (block_id, clock, decision) = loop {
        if let ServerMessage::Decision {
            id,
            decision,
            deadline_ms,
            ..
        } = duel.b.message().await
        {
            break (id, deadline_ms, decision);
        }
    };
    assert_eq!(clock, Some(45_000), "blocking has the longer clock");
    let DecisionKind::DeclareBlockers(options) = decision.kind else {
        panic!("declare blockers expected, got {:?}", decision.kind);
    };
    let (guard, imp) = (options.blockers[0].object, options.blockers[0].can_block[1]);

    duel.b
        .send(&preview(block_id, block(&[(guard, imp)])))
        .await;
    let (_, forecast) = forecast_of(&mut duel.b).await;
    let forecast = forecast.expect("a forecast");
    duel.b
        .send(&ClientMessage::Answer {
            decision: block_id,
            answer: block(&[(guard, imp)]),
        })
        .await;

    // What really happened, in one batch: the damage the forecast gave, and the deaths it named.
    let batch = blocks_batch(&mut duel.b).await;
    assert!(batch.table.combat_forecast.is_none(), "the damage is past");
    let mut to_player = 0;
    let mut died = BTreeSet::new();
    for event in &batch.events {
        match event {
            EventView::Public(Event::DamageDealt {
                target: DamageTarget::Player(PlayerId(1)),
                amount,
                ..
            }) => to_player += amount,
            EventView::Public(Event::CreatureDied { object, .. }) => {
                died.insert(*object);
            }
            _ => {}
        }
    }
    let [player] = forecast.players.as_slice() else {
        panic!("one defender expected");
    };
    assert_eq!(to_player, player.damage);
    assert_eq!(died, forecast.dying().collect::<BTreeSet<_>>());
    assert_eq!(player.damage, 4, "the wolf got through");
    assert_eq!(died.len(), 2, "the guard and the imp traded");
}

#[derive(Debug, Clone, Copy)]
enum Wrong {
    /// An id the server never issued yet.
    NextId,
    /// Zero.
    Zero,
    /// The largest id there is.
    Max,
    /// The other seat names player 1's decision.
    Foreign,
    /// The decision was answered for real already.
    Answered,
}

#[rstest]
#[case(Wrong::NextId)]
#[case(Wrong::Zero)]
#[case(Wrong::Max)]
#[case(Wrong::Foreign)]
#[case(Wrong::Answered)]
#[tokio::test]
async fn a_preview_for_a_decision_that_is_not_pending_for_the_seat_gets_no_forecast(
    #[case] wrong: Wrong,
) {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let answer = block(&[(b.guard, b.imp)]);
    let (sender, named) = match wrong {
        Wrong::NextId => (true, DecisionId(b.id.0 + 1)),
        Wrong::Zero => (true, DecisionId(0)),
        Wrong::Max => (true, DecisionId(u64::MAX)),
        Wrong::Foreign => (false, b.id),
        Wrong::Answered => (true, b.id),
    };
    if matches!(wrong, Wrong::Answered) {
        duel.b
            .send(&ClientMessage::Answer {
                decision: b.id,
                answer: answer.clone(),
            })
            .await;
        blocks_batch(&mut duel.b).await;
    }
    let before = duel.status().await;

    let asker = if sender { &mut duel.b } else { &mut duel.a };
    asker.send(&preview(named, answer.clone())).await;
    let (decision, forecast) = forecast_of(asker).await;
    assert_eq!(decision, named, "the reply names the decision it answers");
    assert_eq!(forecast, None);

    let after = duel.status().await;
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );
    if sender {
        no_forecast(&mut duel.a).await;
    } else {
        no_forecast(&mut duel.b).await;
    }
    if !matches!(wrong, Wrong::Answered) {
        // The decision is still there to be answered with its own id.
        duel.b
            .send(&ClientMessage::Answer {
                decision: b.id,
                answer,
            })
            .await;
        blocks_batch(&mut duel.b).await;
    }
}

#[derive(Debug, Clone, Copy)]
enum Spoiled {
    UnknownBlocker,
    BlockerTwice,
    UnknownAttacker,
    AttackAnswerToABlock,
    PriorityAnswerToABlock,
}

#[rstest]
#[case(Spoiled::UnknownBlocker)]
#[case(Spoiled::BlockerTwice)]
#[case(Spoiled::UnknownAttacker)]
#[case(Spoiled::AttackAnswerToABlock)]
#[case(Spoiled::PriorityAnswerToABlock)]
#[tokio::test]
async fn an_answer_the_engine_would_refuse_gets_no_forecast_and_no_error(#[case] spoiled: Spoiled) {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let answer = match spoiled {
        Spoiled::UnknownBlocker => block(&[(ObjectId(u64::MAX), b.imp)]),
        Spoiled::BlockerTwice => block(&[(b.guard, b.imp), (b.guard, b.wolf)]),
        Spoiled::UnknownAttacker => block(&[(b.guard, ObjectId(u64::MAX))]),
        Spoiled::AttackAnswerToABlock => Answer::DeclareAttackers(Vec::new()),
        Spoiled::PriorityAnswerToABlock => Answer::Priority(PriorityAction::Pass),
    };
    let before = duel.status().await;
    duel.b.send(&preview(b.id, answer)).await;
    assert_eq!(forecast_of(&mut duel.b).await, (b.id, None));
    let after = duel.status().await;
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );

    // The seat can go on: a legal preview still works, and so does the real answer.
    duel.b
        .send(&preview(b.id, block(&[(b.guard, b.imp)])))
        .await;
    assert!(forecast_of(&mut duel.b).await.1.is_some());
    duel.b
        .send(&ClientMessage::Answer {
            decision: b.id,
            answer: block(&[]),
        })
        .await;
    blocks_batch(&mut duel.b).await;
}

#[tokio::test]
async fn a_decision_that_is_not_about_combat_gets_no_forecast() {
    let mut duel = Duel::start().await;
    let (_, _, priority) = duel.attack().await;
    duel.a
        .send(&preview(priority, Answer::Priority(PriorityAction::Pass)))
        .await;
    assert_eq!(forecast_of(&mut duel.a).await, (priority, None));
    // And the attack decision, answered already, has nothing left to preview.
    duel.a
        .send(&preview(
            DecisionId(1),
            Answer::DeclareAttackers(Vec::new()),
        ))
        .await;
    assert_eq!(forecast_of(&mut duel.a).await, (DecisionId(1), None));
}

#[tokio::test]
async fn an_attack_can_be_previewed_before_it_is_declared() {
    let mut duel = Duel::start().await;
    let (id, decision) = next_decision(&mut duel.a).await;
    let DecisionKind::DeclareAttackers(options) = decision.kind else {
        panic!("declare attackers expected");
    };
    let all: Vec<_> = options
        .attackers
        .iter()
        .map(|option| (option.object, option.defenders[0]))
        .collect();
    let before = duel.status().await;
    duel.a
        .send(&preview(id, Answer::DeclareAttackers(all)))
        .await;
    let (decision, forecast) = forecast_of(&mut duel.a).await;
    let forecast = forecast.expect("a forecast of the attack");
    assert_eq!(decision, id);
    let [player] = forecast.players.as_slice() else {
        panic!("one defender expected");
    };
    // 4 + 2 damage against 6 life.
    assert_eq!((player.damage, player.lethal), (6, true));
    assert_eq!(duel.status().await.state_hash, before.state_hash);

    // Nobody attacking is nothing to forecast.
    duel.a
        .send(&preview(id, Answer::DeclareAttackers(Vec::new())))
        .await;
    assert_eq!(forecast_of(&mut duel.a).await, (id, None));
}

#[tokio::test]
async fn previews_over_the_rate_limit_get_no_forecast_and_the_bucket_refills() {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let burst = PREVIEW_BURST as usize;
    let asked = burst + 30;
    let started = Instant::now();
    for _ in 0..asked {
        duel.b
            .send(&preview(b.id, block(&[(b.guard, b.imp)])))
            .await;
    }
    let mut replies = Vec::new();
    for _ in 0..asked {
        replies.push(forecast_of(&mut duel.b).await);
    }
    let elapsed = started.elapsed();
    assert!(
        replies.iter().all(|(decision, _)| *decision == b.id),
        "every preview is answered, in order, for its decision"
    );
    let given = replies
        .iter()
        .filter(|(_, forecast)| forecast.is_some())
        .count();
    // The burst, plus what the steady rate refilled while the flood was being sent.
    let refilled = usize::try_from(elapsed.as_millis() * u128::from(PREVIEWS_PER_SECOND) / 1000)
        .unwrap_or(usize::MAX);
    assert!(
        (burst..=burst + refilled + 1).contains(&given),
        "{given} forecasts for {asked} previews in {elapsed:?}"
    );
    // The ones over the limit come last: the bucket only ever empties.
    let first_none = replies.iter().position(|(_, forecast)| forecast.is_none());
    assert!(first_none.is_some_and(|at| at >= burst));

    // A little later a token is back.
    let mut recovered = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        duel.b
            .send(&preview(b.id, block(&[(b.guard, b.imp)])))
            .await;
        if forecast_of(&mut duel.b).await.1.is_some() {
            recovered = true;
            break;
        }
    }
    assert!(recovered, "the bucket never refilled");
    no_forecast(&mut duel.a).await;
}

#[tokio::test]
async fn a_real_answer_sent_right_after_a_flood_of_previews_goes_through() {
    let mut duel = Duel::start().await;
    let b = duel.reach_blocks().await;
    let flood = 150;
    for _ in 0..flood {
        duel.b
            .send(&preview(b.id, block(&[(b.guard, b.imp)])))
            .await;
    }
    duel.b
        .send(&ClientMessage::Answer {
            decision: b.id,
            answer: block(&[(b.guard, b.imp)]),
        })
        .await;

    // Every preview is answered, before the batch of the real answer, and nothing went wrong.
    let mut forecasts = 0;
    let batch = loop {
        match duel.b.message().await {
            ServerMessage::Forecast { .. } => forecasts += 1,
            ServerMessage::Error { code, message } => panic!("{code:?}: {message}"),
            ServerMessage::Events(batch)
                if batch.events.iter().any(|event| {
                    matches!(event, EventView::Public(Event::BlockersDeclared { .. }))
                }) =>
            {
                break batch;
            }
            _ => {}
        }
    };
    assert_eq!(forecasts, flood);
    assert!(batch.table.combat_forecast.is_some());
    assert!(duel.status().await.events > 0);
    assert_ne!(duel.status().await.waiting_on, Some(PlayerId(1)));
}

#[tokio::test]
async fn a_socket_that_has_not_joined_cannot_preview() {
    let server = TestServer::start(bot_config()).await;
    let created = server
        .registry
        .register(duel(), vec!["Ann".into(), "Bo".into()])
        .await
        .unwrap();
    let game = created.id.to_string();
    let mut client = connect(server.addr, &game, created.seats[0].1.expose()).await;
    client
        .send(&preview(
            DecisionId(1),
            Answer::DeclareAttackers(Vec::new()),
        ))
        .await;
    match client.next().await {
        Frame::Message(ServerMessage::Error { code, .. }) => {
            assert_eq!(code, ErrorCode::BadMessage);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(client.closed().await, Some(1002));
}

#[tokio::test]
async fn a_preview_before_the_game_starts_gets_no_forecast() {
    let server = TestServer::start(bot_config()).await;
    let created = server
        .registry
        .register(duel(), vec!["Ann".into(), "Bo".into()])
        .await
        .unwrap();
    let game = created.id.to_string();
    let mut client = connect(server.addr, &game, created.seats[0].1.expose()).await;
    client.welcome(&game).await;
    client
        .send(&preview(
            DecisionId(1),
            Answer::DeclareAttackers(Vec::new()),
        ))
        .await;
    assert_eq!(forecast_of(&mut client).await, (DecisionId(1), None));
    let handle = server.registry.handle(created.id).unwrap();
    assert!(!handle.status().await.unwrap().started);
}

#[tokio::test]
async fn a_seat_that_does_not_exist_is_ignored() {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(7), duel(), store.clone(), roomy()).unwrap();
    let mut a = Client::join(&handle, 0).await;
    let mut b = Client::join(&handle, 1).await;
    a.say(&handle, ClientMessage::Ready).await;
    b.say(&handle, ClientMessage::Ready).await;
    let before = handle.status().await.unwrap();
    handle
        .send(
            PlayerId(9),
            preview(DecisionId(1), Answer::DeclareAttackers(Vec::new())),
        )
        .await
        .unwrap();
    assert_eq!(handle.status().await.unwrap(), before);
    assert!(
        a.drain()
            .iter()
            .all(|m| !matches!(m, ServerMessage::Forecast { .. }))
    );
    assert!(
        b.drain()
            .iter()
            .all(|m| !matches!(m, ServerMessage::Forecast { .. }))
    );
}

#[tokio::test]
async fn previews_write_nothing_to_the_store() {
    let store = MemoryStore::new();
    let game = game_id(8);
    let handle = spawn_game_with(game, duel(), store.clone(), roomy()).unwrap();
    let mut a = Client::join(&handle, 0).await;
    let mut b = Client::join(&handle, 1).await;
    a.say(&handle, ClientMessage::Ready).await;
    b.say(&handle, ClientMessage::Ready).await;

    // Player 0 attacks with everything, both seats pass, and player 1 faces the block.
    let (id, decision) = a.decision().await;
    let DecisionKind::DeclareAttackers(options) = decision.kind else {
        panic!("declare attackers expected");
    };
    let attacks = options
        .attackers
        .iter()
        .map(|option| (option.object, option.defenders[0]))
        .collect();
    a.say(
        &handle,
        ClientMessage::Answer {
            decision: id,
            answer: Answer::DeclareAttackers(attacks),
        },
    )
    .await;
    let (priority, _) = a.decision().await;
    a.say(
        &handle,
        ClientMessage::Answer {
            decision: priority,
            answer: Answer::Priority(PriorityAction::Pass),
        },
    )
    .await;
    let (id, decision) = loop {
        let (id, decision) = b.decision().await;
        if matches!(decision.kind, DecisionKind::DeclareBlockers(_)) {
            break (id, decision);
        }
        b.say(
            &handle,
            ClientMessage::Answer {
                decision: id,
                answer: Answer::Priority(PriorityAction::Pass),
            },
        )
        .await;
    };
    let DecisionKind::DeclareBlockers(options) = decision.kind else {
        unreachable!()
    };
    let blocks = vec![(options.blockers[0].object, options.blockers[0].can_block[0])];

    let stored = store.load(game).await.unwrap();
    let before = handle.status().await.unwrap();
    assert_eq!(stored.len() as u64, before.events);
    for _ in 0..40 {
        b.say(
            &handle,
            preview(id, Answer::DeclareBlockers(blocks.clone())),
        )
        .await;
        a.say(
            &handle,
            preview(id, Answer::DeclareBlockers(blocks.clone())),
        )
        .await;
    }
    let after = handle.status().await.unwrap();
    assert_eq!(store.load(game).await.unwrap(), stored, "the store changed");
    assert_eq!(after, before);
    // Only the asking seats were answered, one forecast per preview.
    let forecasts = |messages: Vec<ServerMessage>| {
        messages
            .into_iter()
            .filter(|m| matches!(m, ServerMessage::Forecast { .. }))
            .count()
    };
    assert_eq!(forecasts(a.drain()), 40);
    assert_eq!(forecasts(b.drain()), 40);
}
