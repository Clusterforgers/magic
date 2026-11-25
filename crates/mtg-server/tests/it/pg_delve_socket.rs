//! Goal: a run's socket (`/ws/delves/{id}`) follows the run on Postgres: the first frame is the run as the caller sees
//! it, and each member's action reaches every open socket of the run at once, as that member sees it, while another
//! run's changes send nothing; a socket without a session, at a malformed or unknown run or from a foreign origin is
//! refused before the upgrade.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::ws::{Frame, WsClient, open, refused_at};
use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};

async fn call(server: &TestServer, method: &str, path: &str, who: &Browser, body: Value) -> Reply {
    let body = body.to_string();
    let headers = [
        ("Cookie", who.cookie.as_str()),
        ("Content-Type", "application/json"),
    ];
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) {
    let reply = call(
        server,
        "POST",
        &format!("/api/delves/{id}/act"),
        who,
        action,
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
}

async fn create(server: &TestServer, host: &Browser) -> (String, String) {
    let reply = call(server, "POST", "/api/delves", host, json!({"heat": 0})).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let created = reply.json();
    let field = |key: &str| created[key].as_str().unwrap().to_owned();
    (field("id"), field("code"))
}

async fn follow(server: &TestServer, id: &str, who: &Browser) -> WsClient {
    open(
        server.addr,
        &format!("/ws/delves/{id}"),
        &[("Cookie", who.cookie.as_str())],
    )
    .await
}

/// The next frame, which must be a run.
async fn frame(socket: &mut WsClient) -> Value {
    match socket.next().await {
        Frame::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("expected a run, got {other:?}"),
    }
}

fn names(view: &Value) -> Vec<String> {
    view["run"]["heroes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hero| hero["name"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = false)]
async fn every_open_socket_of_a_run_hears_each_change(pool: sqlx::PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let guest = register(server.addr, "guest", PASSWORD).await;
    let other = register(server.addr, "other", PASSWORD).await;
    let (id, code) = create(&server, &host).await;
    let (elsewhere, _) = create(&server, &other).await;

    let mut hosts = follow(&server, &id, &host).await;
    let first = frame(&mut hosts).await;
    assert_eq!(first["id"], id.as_str());
    assert_eq!(first["host"], true);
    assert_eq!(names(&first), ["hostess"]);

    let reply = call(
        &server,
        "POST",
        &format!("/api/delves/join/{code}"),
        &guest,
        json!({}),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let joined = frame(&mut hosts).await;
    assert_eq!(names(&joined), ["hostess", "guest"]);
    assert_eq!(joined["me"], 0);

    let mut guests = follow(&server, &id, &guest).await;
    assert_eq!(frame(&mut guests).await["me"], 1);

    // Another run's change sends nothing: the next frame on each socket is this run's bot ally.
    act(
        &server,
        &elsewhere,
        &other,
        json!({"kind": "add_bot", "level": "Easy"}),
    )
    .await;
    act(
        &server,
        &id,
        &host,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    for (socket, me, host) in [(&mut hosts, 0, true), (&mut guests, 1, false)] {
        let view = frame(socket).await;
        assert_eq!(view["id"], id.as_str());
        assert_eq!(view["run"]["heroes"].as_array().unwrap().len(), 3);
        assert_eq!(view["me"], me);
        assert_eq!(view["host"], host);
    }

    let deck = pg_commander::on(pg_commander::BEAST);
    let choose = json!({"kind": "choose_deck", "hero": 1, "deck": deck});
    act(&server, &id, &guest, choose).await;
    for socket in [&mut hosts, &mut guests] {
        let view = frame(socket).await;
        assert!(
            view["run"]["heroes"][1]["deck"].is_object(),
            "{}",
            view["run"]["heroes"][1]
        );
    }
    hosts.close().await;
    guests.close().await;
}

#[sqlx::test(migrations = false)]
async fn a_socket_is_refused_before_the_upgrade(pool: sqlx::PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, _) = create(&server, &host).await;
    let cookie = host.cookie.as_str();
    let unknown = uuid::Uuid::new_v4().to_string();
    let cases: [(&str, &str, Vec<(&str, &str)>, u16); 4] = [
        ("no session", &id, vec![], 401),
        ("malformed run", "not-a-run", vec![("Cookie", cookie)], 404),
        ("unknown run", &unknown, vec![("Cookie", cookie)], 404),
        (
            "foreign origin",
            &id,
            vec![("Cookie", cookie), ("Origin", "https://evil.example")],
            403,
        ),
    ];
    for (name, run, headers, status) in cases {
        let (got, body) = refused_at(server.addr, &format!("/ws/delves/{run}"), &headers).await;
        assert_eq!(got, status, "{name}: {body}");
    }
}
