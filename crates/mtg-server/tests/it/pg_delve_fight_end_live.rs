//! Goal: a boss fight's end reaches the run page at once: when the fight's game ends, the server settles the run and
//! every open socket of it gets the settled run without anyone reading it, and a later read folds the fight in no
//! second time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::pg_delve_forfeit::{call, fight_run, read};
use crate::{net, pg_commander};

use mtg_server::ServerConfig;
use net::lobby;
use net::ws::{Frame, open};
use net::{PASSWORD, bot_config, register};
use serde_json::Value;
use sqlx::PgPool;

#[sqlx::test(migrations = false)]
async fn a_socket_hears_the_won_fight_without_a_read(pool: PgPool) {
    let config = ServerConfig {
        test_hooks: true,
        ..bot_config()
    };
    let server = pg_commander::start_with(&pool, config).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    lobby::start(&server.registry, &game, 3).await;

    let path = format!("/ws/delves/{id}");
    let mut socket = open(server.addr, &path, &[("Cookie", host.cookie.as_str())]).await;
    let mut next = async || match socket.next().await {
        Frame::Text(text) => serde_json::from_str::<Value>(&text).unwrap(),
        other => panic!("expected a run, got {other:?}"),
    };
    let first = next().await;
    assert!(first["run"]["status"]["Fight"].is_object(), "{first}");
    // The socket's own first read may save and so send the run again; the socket is quiet before the fight ends.
    while let Ok(echo) = tokio::time::timeout(Duration::from_secs(1), next()).await {
        assert!(echo["run"]["status"]["Fight"].is_object(), "{echo}");
    }

    // The hook only loads the run; nothing it does saves or reads it.
    let hook = format!("/api/test/delves/{id}/boss-concedes");
    let conceded = call(&server, "POST", &hook, &host, None).await;
    assert_eq!(conceded.status, 204, "{}", conceded.text());
    let settled = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let view = next().await;
            if view["run"]["status"] == "Room" {
                return view;
            }
        }
    })
    .await
    .expect("the socket never heard the fight end");
    assert_eq!(settled["run"]["cleared"], serde_json::json!([0]));
    assert_eq!(settled["run"]["hearts"], 3, "a won fight costs nothing");

    let again = read(&server, &id, &host).await;
    assert_eq!(again["run"], settled["run"], "a read folds the fight once");
}
