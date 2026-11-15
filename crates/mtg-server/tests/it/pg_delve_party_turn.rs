//! Goal: a Delves fight the server makes for a party of four (a human host and three bot heroes) plays shared team
//! turns: the two sides take turns, each led by its first seat (the boss, not a minion), and every hero, the bots
//! included, holds priority in the party's main phase.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::common::{self, Client};
use crate::{net, pg_commander};

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_protocol::events::EventView;
use mtg_protocol::{ClientMessage, ServerMessage};
use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) -> Reply {
    let body = action.to_string();
    let headers = [
        ("Cookie", who.cookie.as_str()),
        ("Content-Type", "application/json"),
    ];
    request(
        server.addr,
        "POST",
        &format!("/api/delves/{id}/act"),
        &headers,
        body.as_bytes(),
    )
    .await
}

#[sqlx::test(migrations = false)]
async fn a_party_of_four_shares_its_turn_against_the_boss(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let created = request(
        server.addr,
        "POST",
        "/api/delves",
        &[
            ("Cookie", host.cookie.as_str()),
            ("Content-Type", "application/json"),
        ],
        json!({"heat": 0, "seed": "PARTY"}).to_string().as_bytes(),
    )
    .await
    .json();
    let id = created["id"].as_str().unwrap().to_owned();
    for _ in 0..3 {
        let bot = act(
            &server,
            &id,
            &host,
            json!({"kind": "add_bot", "level": "Normal"}),
        )
        .await;
        assert_eq!(bot.status, 200, "{}", bot.text());
    }
    for hero in 0..4 {
        let deck = pg_commander::on(pg_commander::BEAST);
        let chosen = act(
            &server,
            &id,
            &host,
            json!({"kind": "choose_deck", "hero": hero, "deck": deck}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(&server, &id, &host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    let game = start.json()["run"]["status"]["Fight"]["game"]
        .as_str()
        .unwrap()
        .to_owned();

    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let mut client = Client::join(&handle, 0).await;
    client.say(&handle, ClientMessage::Ready).await;
    let boss = PlayerId::from_index(4);
    let mut turns: Vec<PlayerId> = Vec::new();
    let mut step = Step::Untap;
    let mut holders = std::collections::BTreeSet::new();
    let played = tokio::time::timeout(Duration::from_secs(600), async {
        while turns.len() < 4 {
            let Some(message) = client.inbox.recv().await else {
                return;
            };
            match message {
                ServerMessage::Events(batch) => {
                    for event in &batch.events {
                        match event {
                            EventView::Public(Event::TurnBegan { active, .. }) => {
                                turns.push(*active);
                            }
                            EventView::Public(Event::StepBegan { step: began }) => {
                                step = *began;
                            }
                            EventView::Public(Event::PriorityPassed { player })
                                if step == Step::Main1
                                    && turns.last() == Some(&PlayerId::from_index(0)) =>
                            {
                                holders.insert(player.index());
                            }
                            _ => {}
                        }
                    }
                }
                ServerMessage::Decision { id, decision, .. } => {
                    let answer = common::passive(&decision);
                    client
                        .say(
                            &handle,
                            ClientMessage::Answer {
                                decision: id,
                                answer,
                            },
                        )
                        .await;
                }
                ServerMessage::GameOver { .. } => return,
                _ => {}
            }
        }
    })
    .await;
    assert!(played.is_ok(), "the fight stalled after turns {turns:?}");
    let hero = PlayerId::from_index(0);
    let (first, second) = if turns.first() == Some(&boss) {
        (boss, hero)
    } else {
        (hero, boss)
    };
    assert_eq!(
        turns,
        [first, second, first, second],
        "the sides take turns, each led by its first seat"
    );
    for seat in 0..4 {
        assert!(
            holders.contains(&seat),
            "hero {seat} held priority in the party's main phase: {holders:?}"
        );
    }
}
