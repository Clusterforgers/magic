//! Goal: a take-back the other seat approves cuts the stored log back to before the action, appends `TookBack` and sends every seat a `rewound` snapshot that identifies no card back in a hand to another seat; a decline, silence past the wait, a refusal by the engine (each sent to the asker alone), a second request and a stray vote change nothing; a cast backed out of after tapping mana for it is rewound the same way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, poke, roomy, seat};
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction, PriorityOptions};
use mtg_engine::take_back::{TakeBackKind, TakeBackRefusal};
use mtg_engine::{Answer, Decision, Event};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, EventView, ServerMessage, TakeBackOutcome,
};
use mtg_server::{EventStore, GameHandle, MemoryStore, spawn_game_with};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Scenario, ready};

/// A table where seat 0 holds a Forest and Grizzly Bears over two ready Forests and seat 1 holds nothing; every
/// decision is sent, and seat 0's latest one is kept.
struct Table {
    handle: GameHandle,
    store: MemoryStore,
    clients: Vec<Client>,
    decision: Option<(DecisionId, Decision)>,
}

impl Table {
    async fn new(rules: HouseRules) -> Table {
        let setup = Scenario::new(2)
            .hand(0, [forest(), bear()])
            .battlefield(0, [ready(forest()), ready(forest())])
            .house_rules(rules)
            .setup()
            .unwrap();
        let store = MemoryStore::new();
        let handle = spawn_game_with(game_id(1), setup, store.clone(), roomy()).unwrap();
        let clients = vec![
            Client::join(&handle, 0).await,
            Client::join(&handle, 1).await,
        ];
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
        let mut table = Table {
            handle,
            store,
            clients,
            decision: None,
        };
        table.handle.status().await.unwrap();
        let first = table.clients[0].drain();
        table.clients[1].drain();
        table.note(&first);
        table
    }

    fn note(&mut self, messages: &[ServerMessage]) {
        for message in messages {
            match message {
                ServerMessage::Decision { id, decision, .. } => {
                    self.decision = Some((*id, decision.clone()));
                }
                ServerMessage::Snapshot { .. } => self.decision = None,
                _ => {}
            }
        }
    }

    async fn poke(&mut self, from: usize, message: ClientMessage) -> Vec<Vec<ServerMessage>> {
        let received = poke(&self.handle, &mut self.clients, from, message).await;
        self.note(&received[0]);
        received
    }

    async fn answer(&mut self, answer: Answer) -> Vec<Vec<ServerMessage>> {
        let (id, _) = self.decision.clone().expect("seat 0 has a decision");
        self.poke(
            0,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await
    }

    fn options(&self) -> PriorityOptions {
        match self.decision.as_ref().map(|(_, decision)| &decision.kind) {
            Some(DecisionKind::Priority(options)) => (**options).clone(),
            other => panic!("seat 0 has no priority: {other:?}"),
        }
    }

    async fn play_forest(&mut self) {
        let land = self.options().playable_lands[0];
        self.answer(Answer::Priority(PriorityAction::PlayLand(land)))
            .await;
    }

    async fn cast_bear(&mut self) -> ObjectId {
        let spell = self.options().castable[0];
        self.answer(Answer::Priority(PriorityAction::CastSpell(spell)))
            .await;
        self.answer(Answer::PayMana(PayChoice::Auto)).await;
        spell
    }

    async fn log(&self) -> Vec<Event> {
        self.store.load(game_id(1)).await.unwrap()
    }
}

fn ended(messages: &[ServerMessage]) -> Option<TakeBackOutcome> {
    messages.iter().find_map(|m| match m {
        ServerMessage::TakeBackEnded { outcome, .. } => Some(outcome.clone()),
        _ => None,
    })
}

fn rewound(messages: &[ServerMessage]) -> Option<&ServerMessage> {
    messages
        .iter()
        .find(|m| matches!(m, ServerMessage::Snapshot { rewound: true, .. }))
}

#[tokio::test]
async fn an_approved_take_back_rebuilds_the_game_without_the_cast_for_every_seat() {
    let mut table = Table::new(HouseRules::default()).await;
    let before = table.log().await;
    let card = table.cast_bear().await;
    assert!(table.log().await.len() > before.len());

    let asked = table.poke(0, ClientMessage::TakeBack).await;
    for seen in &asked {
        assert!(seen.iter().any(|m| matches!(
            m,
            ServerMessage::TakeBackAsked { offer, waiting, .. }
                if offer.kind == TakeBackKind::Cast && offer.name == "Grizzly Bears" && waiting == &[seat(1)]
        )));
    }
    let voted = table
        .poke(1, ClientMessage::TakeBackVote { approve: true })
        .await;
    for seen in &voted {
        assert_eq!(ended(seen), Some(TakeBackOutcome::Approved));
        let Some(ServerMessage::Snapshot { log, state, .. }) = rewound(seen) else {
            panic!("every seat gets a rewound snapshot: {seen:?}");
        };
        assert!(matches!(
            log.last(),
            Some(EventView::Public(Event::TookBack { player, name, .. })) if *player == seat(0) && name == "Grizzly Bears"
        ));
        let identifies = state.objects.iter().any(|object| object.id == card);
        assert_eq!(
            identifies,
            seen == &voted[0],
            "only the owner sees the card back in hand"
        );
    }
    let stored = table.log().await;
    assert_eq!(stored[..before.len()], before[..]);
    assert_eq!(stored.len(), before.len() + 1);
    assert_eq!(
        table.options().castable,
        vec![card],
        "seat 0 has priority again"
    );
}

#[tokio::test]
async fn a_land_play_taken_back_can_be_played_again() {
    let mut table = Table::new(HouseRules::default()).await;
    let before = table.log().await.len();
    table.play_forest().await;
    table.poke(0, ClientMessage::TakeBack).await;
    table
        .poke(1, ClientMessage::TakeBackVote { approve: true })
        .await;
    assert_eq!(table.log().await.len(), before + 1);
    table.play_forest().await;
}

#[tokio::test]
async fn a_decline_ends_the_request_and_changes_nothing() {
    let mut table = Table::new(HouseRules::default()).await;
    table.cast_bear().await;
    let log = table.log().await;
    table.poke(0, ClientMessage::TakeBack).await;
    let voted = table
        .poke(1, ClientMessage::TakeBackVote { approve: false })
        .await;
    for seen in &voted {
        assert_eq!(ended(seen), Some(TakeBackOutcome::Declined { by: seat(1) }));
        assert!(rewound(seen).is_none());
    }
    assert_eq!(table.log().await, log);
}

#[tokio::test(start_paused = true)]
async fn silence_past_the_wait_declines() {
    let mut table = Table::new(HouseRules::default()).await;
    table.cast_bear().await;
    let log = table.log().await;
    table.poke(0, ClientMessage::TakeBack).await;
    let outcome = table.clients[0]
        .until(|m| match m {
            ServerMessage::TakeBackEnded { outcome, .. } => Some(outcome.clone()),
            _ => None,
        })
        .await;
    assert_eq!(outcome, TakeBackOutcome::TimedOut { by: seat(1) });
    assert_eq!(table.log().await, log);
}

#[tokio::test]
async fn a_refusal_reaches_the_asker_alone() {
    let rules = HouseRules {
        take_backs: Some(false),
        ..HouseRules::default()
    };
    for (rules, act, reason) in [
        (rules, true, TakeBackRefusal::Off),
        (HouseRules::default(), false, TakeBackRefusal::Nothing),
    ] {
        let mut table = Table::new(rules).await;
        if act {
            table.cast_bear().await;
        }
        let log = table.log().await;
        let received = table.poke(0, ClientMessage::TakeBack).await;
        assert_eq!(received[0], vec![ServerMessage::TakeBackRefused { reason }]);
        assert!(received[1].is_empty());
        assert_eq!(table.log().await, log);
    }
}

#[tokio::test]
async fn a_second_request_and_a_stray_vote_are_bad_messages() {
    let mut table = Table::new(HouseRules::default()).await;
    table.cast_bear().await;
    let stray = table
        .poke(1, ClientMessage::TakeBackVote { approve: true })
        .await;
    assert!(matches!(
        stray[1][..],
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
    table.poke(0, ClientMessage::TakeBack).await;
    let again = table.poke(0, ClientMessage::TakeBack).await;
    assert!(matches!(
        again[0][..],
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
    let own = table
        .poke(0, ClientMessage::TakeBackVote { approve: true })
        .await;
    assert!(matches!(
        own[0][..],
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
}

#[tokio::test]
async fn the_seat_is_told_what_it_may_take_back() {
    let mut table = Table::new(HouseRules::default()).await;
    let land = table.options().playable_lands[0];
    let played = table
        .answer(Answer::Priority(PriorityAction::PlayLand(land)))
        .await;
    assert!(played[0].iter().any(|m| matches!(
        m,
        ServerMessage::TakeBackOffer { offer: Some(offer) } if offer.kind == TakeBackKind::Land
    )));
    assert!(
        !played[1]
            .iter()
            .any(|m| matches!(m, ServerMessage::TakeBackOffer { .. }))
    );
}

#[tokio::test]
async fn a_cast_backed_out_of_untaps_the_mana_tapped_for_it() {
    let mut table = Table::new(HouseRules::default()).await;
    let before = table.log().await;
    let source = table.options().mana_abilities[0].source;
    table
        .answer(Answer::Priority(PriorityAction::ActivateMana(source)))
        .await;
    assert!(table.log().await.len() > before.len());
    let spell = table.options().castable[0];
    table
        .answer(Answer::Priority(PriorityAction::CastSpell(spell)))
        .await;
    let backed = table.answer(Answer::PayMana(PayChoice::Cancel)).await;
    for seen in &backed {
        let Some(ServerMessage::Snapshot { state, .. }) = rewound(seen) else {
            panic!("every seat gets a rewound snapshot: {seen:?}");
        };
        assert!(
            state.table.players[0].mana.is_empty(),
            "the pool is empty again"
        );
    }
    assert_eq!(table.log().await, before);
    assert_eq!(table.options().castable, vec![spell]);
}

#[tokio::test(start_paused = true)]
async fn a_bot_agrees_at_once() {
    use mtg_server::bots::{BotLevel, BotSeat};
    use mtg_server::config::ServerConfig;
    use mtg_server::demo;
    use mtg_server::registry::{GameOptions, Registry, SeatRequest};
    use std::time::Duration;

    let registry = Registry::new(&ServerConfig {
        game: roomy(),
        bot_delay: Duration::from_millis(50),
        ..ServerConfig::default()
    });
    let requests = (0..2)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[0],
        })
        .collect::<Vec<_>>();
    let options = GameOptions {
        bots: vec![BotSeat {
            seat: seat(1),
            level: BotLevel::Easy,
        }],
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests, options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut me = Client::join(&handle, 0).await;
    me.say(&handle, ClientMessage::Ready).await;
    loop {
        let (id, decision) = me.decision().await;
        let land = match &decision.kind {
            DecisionKind::Priority(options) => options.playable_lands.first().copied(),
            _ => None,
        };
        let answer = land.map_or_else(
            || common::passive(&decision),
            |land| Answer::Priority(PriorityAction::PlayLand(land)),
        );
        me.say(
            &handle,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await;
        if land.is_some() {
            break;
        }
    }
    me.decision().await;
    me.say(&handle, ClientMessage::TakeBack).await;
    let outcome = me
        .until(|m| match m {
            ServerMessage::TakeBackEnded { outcome, .. } => Some(outcome.clone()),
            ServerMessage::TakeBackRefused { reason } => panic!("refused: {reason:?}"),
            _ => None,
        })
        .await;
    assert_eq!(outcome, TakeBackOutcome::Approved);
}
