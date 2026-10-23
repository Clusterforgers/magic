//! Goal: on Postgres the deck check takes `precon:<code>` as its deck without an account and answers the report of the precon alone or its verdict at a table (a game, or described rules) that agrees with what a seat choosing the deck there is told, at two to four seats and under each deck rule a table can relax; an unknown or malformed code, a body of the wrong shape and a bad table are refused, the stored-deck check keeps asking for an account, and the check counts against the precon budget of the address.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::net::{self, Browser, PASSWORD, TestServer, bot_config, register};
use crate::pg_commander::{
    BEAR, BEAST, GHOST, RED, SHORT, bear_pack, beast_pack, choose, create_body, fixture,
    ghost_pack, on, precon_deck, red_pack, short_pack, source,
};

use mtg_db::Database;
use mtg_import::mtgjson::PreconDeck;
use mtg_import::scryfall::cards;
use mtg_server::migrate;
use serde_json::{Value, json};
use sqlx::PgPool;

const TWICE: &str = "cmd-twice-pack";
const PAIR: &str = "cmd-pair-pack";
const EVERGREEN: &str = "cmd-evergreen-pack";

/// A second copy of a card under a commander.
fn twice_pack() -> PreconDeck {
    precon_deck(
        "Twice Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Auspicious Starrix", 2), ("Forest", 97)],
    )
}

/// Two commanders that are no partners.
fn pair_pack() -> PreconDeck {
    precon_deck(
        "Pair Pack",
        "Commander Deck",
        &[("Questing Beast", 1), ("Rosheen Meanderer", 1)],
        &[("Forest", 98)],
    )
}

/// A Starter Deck of sixty Forests: a Normal game takes it, a Commander game does not.
fn evergreen_pack() -> PreconDeck {
    precon_deck("Evergreen Pack", "Starter Deck", &[], &[("Forest", 60)])
}

/// A server on a corpus and nine precons; an address may check `per_ip` precons per window.
async fn start(pool: &PgPool, per_ip: u32) -> TestServer {
    migrate(pool).await.unwrap();
    let database = Database::from_pool(pool.clone());
    database.migrate().await.unwrap();
    database
        .importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
    let packs = [
        beast_pack(),
        short_pack(),
        red_pack(),
        bear_pack(),
        ghost_pack(),
        twice_pack(),
        pair_pack(),
        evergreen_pack(),
    ];
    database
        .importer()
        .import_precons(packs.into_iter().map(Ok))
        .await
        .unwrap();
    let mut config = bot_config();
    config.auth.limits.precon_requests_per_ip = per_ip;
    config.auth.limits.lobby_window = Duration::from_secs(2);
    TestServer::start_with_cards(pool.clone(), config, source(pool)).await
}

/// `POST /api/decks/check` with no account.
async fn anonymous(server: &TestServer, body: &Value) -> net::Reply {
    net::post_json(server.addr, "/api/decks/check", &body.to_string()).await
}

/// [`anonymous`] with a body that is not used again.
async fn verdict(server: &TestServer, body: Value) -> net::Reply {
    anonymous(server, &body).await
}

/// `POST /api/decks/check` as `who`.
async fn as_account(server: &TestServer, who: &Browser, body: &Value) -> net::Reply {
    who.post(server.addr, "/api/decks/check", &body.to_string())
        .await
}

/// The codes of the issues of a report, in order.
fn codes(report: &Value) -> Vec<String> {
    report["legality"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap().to_owned())
        .collect()
}

/// A game of `seats` seats, the first the creator's, in `format` under `rules`.
async fn open(
    server: &TestServer,
    ann: &Browser,
    format: &str,
    seats: usize,
    rules: &Value,
) -> String {
    let mut open_seats = vec![json!({"name": "Ann"})];
    open_seats.resize(seats, json!({}));
    let mut body = create_body(&open_seats, 1);
    body["format"] = json!(format);
    if rules.as_object().is_some_and(|rules| !rules.is_empty()) {
        body["house_rules"] = rules.clone();
    }
    ann.create_game(server.addr, &body).await.0
}

/// A precon, the state its report gives alone and what keeps it out of any table.
struct Alone {
    code: &'static str,
    format: &'static str,
    state: &'static str,
    issues: &'static [&'static str],
}

#[sqlx::test(migrations = false)]
async fn a_precon_is_checked_alone_with_or_without_an_account_as_the_list_judges_it(pool: PgPool) {
    let server = start(&pool, 10_000).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let list = net::get(server.addr, "/api/precons").await.json();
    let cases = [
        Alone {
            code: BEAST,
            format: "commander",
            state: "manual_play",
            issues: &[],
        },
        Alone {
            code: EVERGREEN,
            format: "freeform",
            state: "verified",
            issues: &[],
        },
        Alone {
            code: SHORT,
            format: "commander",
            state: "blocked",
            issues: &["deck_size"],
        },
        Alone {
            code: RED,
            format: "commander",
            state: "blocked",
            issues: &["color_identity"],
        },
        Alone {
            code: BEAR,
            format: "commander",
            state: "blocked",
            issues: &["commander_eligibility"],
        },
        Alone {
            code: GHOST,
            format: "commander",
            state: "blocked",
            issues: &["deck_size", "unknown_card"],
        },
    ];
    for case in cases {
        let body = json!({"deck": on(case.code)});
        let without = anonymous(&server, &body).await;
        assert_eq!(without.status, 200, "{}: {}", case.code, without.text());
        let with = as_account(&server, &ann, &body).await;
        assert_eq!(with.status, 200, "{}: {}", case.code, with.text());
        let report = without.json();
        assert_eq!(
            report,
            with.json(),
            "{}: an account is no different",
            case.code
        );

        assert_eq!(report["format"], case.format, "{}", case.code);
        assert_eq!(report["summary"]["state"], case.state, "{}", case.code);
        assert_eq!(codes(&report), case.issues, "{}", case.code);
        assert!(
            !report["cards"].as_array().unwrap().is_empty(),
            "{}",
            case.code
        );

        // The list shows the same verdict, so the picker and the table never disagree.
        let listed = list["precons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == case.code)
            .unwrap();
        assert_eq!(listed["state"], report["summary"]["state"], "{}", case.code);
        assert_eq!(listed["summary"], report["summary"], "{}", case.code);
    }

    // A precon with a card played by hand names how many.
    let report = anonymous(&server, &json!({"deck": on(BEAST)})).await.json();
    let counts = &report["summary"]["counts"];
    assert!(counts["verified"].as_u64() < counts["total"].as_u64());
    // The code is read in lower case, like the precon endpoints read it.
    let shouted = anonymous(&server, &json!({"deck": on(&BEAST.to_uppercase())})).await;
    assert_eq!(shouted.json(), report);
}

/// One precon at the tables of a deck rule: refused at a plain Commander table for `issue`, taken at
/// a table that relaxed `rules`.
struct Relaxed {
    code: &'static str,
    rules: Value,
    issue: &'static str,
}

#[sqlx::test(migrations = false)]
async fn the_verdict_at_a_table_agrees_with_the_seat_choice_at_two_to_four_seats(pool: PgPool) {
    let server = start(&pool, 10_000).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let relaxed = [
        Relaxed {
            code: TWICE,
            rules: json!({"deck": {"singleton": false}}),
            issue: "copies",
        },
        Relaxed {
            code: RED,
            rules: json!({"deck": {"colour_identity": false}}),
            issue: "color_identity",
        },
        Relaxed {
            code: SHORT,
            rules: json!({"deck": {"deck_size": 51}}),
            issue: "deck_size",
        },
        Relaxed {
            code: PAIR,
            rules: json!({"commanders_per_seat": 2}),
            issue: "commander_pair",
        },
    ];
    // What a seat is told when it chooses `code` in `game`; the check must say the same.
    let seat_says = |code: &'static str, game: String| {
        let (server, ann) = (&server, &ann);
        async move {
            let reply = choose(server, ann, &game, 0, &on(code)).await;
            match reply.status {
                200 => Vec::new(),
                422 => reply.json()["issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|issue| issue["code"].as_str().unwrap().to_owned())
                    .collect(),
                other => panic!("{code}: {other}: {}", reply.text()),
            }
        }
    };

    for seats in 2..=4 {
        let plain = open(&server, &ann, "commander", seats, &json!({})).await;
        let normal = open(&server, &ann, "normal", seats, &json!({})).await;
        let everything = json!({
            "deck": {"singleton": false, "colour_identity": false},
            "commanders_per_seat": 2,
        });
        let lenient = open(&server, &ann, "commander", seats, &everything).await;

        for case in &relaxed {
            let at = format!("{} at {seats} seats", case.code);
            let relaxed_game = open(&server, &ann, "commander", seats, &case.rules).await;

            // The game that relaxed the rule takes the deck, the plain game refuses it for the rule.
            let taken = verdict(
                &server,
                json!({"deck": on(case.code), "game": relaxed_game}),
            )
            .await;
            assert_eq!(taken.status, 200, "{at}: {}", taken.text());
            let report = taken.json();
            assert!(codes(&report).is_empty(), "{at}: {report}");
            assert_ne!(report["summary"]["state"], "blocked", "{at}");
            assert_eq!(report["summary"]["issues"], 0, "{at}");
            assert!(seat_says(case.code, relaxed_game).await.is_empty(), "{at}");

            let refused = verdict(&server, json!({"deck": on(case.code), "game": plain}))
                .await
                .json();
            assert_eq!(codes(&refused), [case.issue], "{at}");
            assert_eq!(refused["summary"]["state"], "blocked", "{at}");
            assert_eq!(refused["summary"]["issues"], 1, "{at}");
            assert_eq!(
                seat_says(case.code, plain.clone()).await,
                [case.issue],
                "{at}"
            );

            // Rules that no game holds yet give the same two answers.
            let described = json!({"format": "commander", "house_rules": case.rules});
            let report = verdict(&server, json!({"deck": on(case.code), "table": described}))
                .await
                .json();
            assert!(codes(&report).is_empty(), "{at}: {report}");
            let report = verdict(
                &server,
                json!({"deck": on(case.code), "table": {"format": "commander"}}),
            )
            .await
            .json();
            assert_eq!(codes(&report), [case.issue], "{at}");
        }

        // The format of the table decides what a deck is for.
        let by_format: [(&str, &str, &str, &[&str]); 4] = [
            (BEAST, "commander", "manual_play", &[]),
            (BEAST, "normal", "blocked", &["no_commander_zone"]),
            (EVERGREEN, "commander", "blocked", &["commander_count"]),
            (EVERGREEN, "normal", "verified", &[]),
        ];
        for (code, format, state, issues) in by_format {
            let at = format!("{code} at {format}, {seats} seats");
            let game = if format == "normal" { &normal } else { &plain };
            let report = verdict(&server, json!({"deck": on(code), "game": game}))
                .await
                .json();
            assert_eq!(codes(&report), issues, "{at}");
            assert_eq!(report["summary"]["state"], state, "{at}");
            assert_eq!(seat_says(code, game.clone()).await, issues, "{at}");
            let table = verdict(
                &server,
                json!({"deck": on(code), "table": {"format": format}}),
            )
            .await;
            assert_eq!(table.json(), report, "{at}: a described table is the same");
        }

        // What no table relaxes still blocks the precon.
        for (code, issue) in [(BEAR, "commander_eligibility"), (GHOST, "unknown_card")] {
            let at = format!("{code} at {seats} seats");
            let report = verdict(&server, json!({"deck": on(code), "game": lenient}))
                .await
                .json();
            assert!(
                codes(&report).iter().any(|code| code == issue),
                "{at}: {report}"
            );
            assert_eq!(report["summary"]["state"], "blocked", "{at}");
            let held = seat_says(code, lenient.clone()).await;
            assert!(held.iter().any(|held| held == issue), "{at}: {held:?}");
        }
    }
}

#[sqlx::test(migrations = false)]
async fn a_request_that_names_no_precon_or_no_table_is_refused_and_a_stored_deck_still_needs_an_account(
    pool: PgPool,
) {
    let server = start(&pool, 10_000).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let game = open(&server, &ann, "commander", 2, &json!({})).await;
    let evergreen = on(EVERGREEN);
    let cases: Vec<(&str, Value, u16, &str)> = vec![
        (
            "unknown code",
            json!({"deck": "precon:nothing-here"}),
            404,
            "not_found",
        ),
        ("empty code", json!({"deck": "precon:"}), 404, "not_found"),
        (
            "malformed code",
            json!({"deck": "precon:Not A Code!"}),
            404,
            "not_found",
        ),
        (
            "very long code",
            json!({"deck": format!("precon:{}", "a".repeat(5_000))}),
            404,
            "not_found",
        ),
        (
            "a list beside the precon",
            json!({"deck": evergreen, "format": "commander", "cards": []}),
            400,
            "invalid_request",
        ),
        (
            "a format beside the precon",
            json!({"deck": evergreen, "format": "commander"}),
            400,
            "invalid_request",
        ),
        (
            "a game and a table",
            json!({"deck": evergreen, "game": game, "table": {"format": "normal"}}),
            400,
            "invalid_request",
        ),
        (
            "a game nobody has",
            json!({"deck": evergreen, "game": "nosuchgame"}),
            404,
            "not_found",
        ),
        (
            "a format the server does not play",
            json!({"deck": evergreen, "table": {"format": "banana"}}),
            400,
            "invalid_request",
        ),
        (
            "a rule out of range",
            json!({"deck": evergreen, "table": {"format": "commander", "house_rules": {"deck": {"deck_size": 5}}}}),
            400,
            "invalid_request",
        ),
    ];
    for (why, body, status, code) in cases {
        let without = anonymous(&server, &body).await;
        assert_eq!(
            (without.status, without.error_code()),
            (status, code.to_owned()),
            "{why}: {}",
            without.text()
        );
        let with = as_account(&server, &ann, &body).await;
        assert_eq!(
            (with.status, with.error_code()),
            (status, code.to_owned()),
            "{why}: {}",
            with.text()
        );
    }

    // Everything but a precon still needs a session, whatever else is wrong with the body.
    let id = "4457ed35-7c10-48c8-9776-456485fdf070";
    let still_private: Vec<(&str, Value)> = vec![
        ("a stored deck", json!({"deck": id})),
        (
            "a stored deck at a table",
            json!({"deck": id, "game": game}),
        ),
        ("a list", json!({"format": "commander", "cards": []})),
        ("a body of nothing", json!({})),
        ("a body of the wrong shape", json!({"cards": 7})),
        (
            "a field nobody knows beside a precon",
            json!({"deck": evergreen, "name": "x"}),
        ),
    ];
    for (why, body) in still_private {
        let reply = anonymous(&server, &body).await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (401, "unauthenticated".to_owned()),
            "{why}"
        );
    }
    let unknown_field = as_account(&server, &ann, &json!({"deck": evergreen, "name": "x"})).await;
    assert_eq!(
        (unknown_field.status, unknown_field.error_code()),
        (400, "invalid_request".to_owned())
    );
    let json_headers = [("Content-Type", "application/json")];
    let text = [("Content-Type", "text/plain")];
    let precon = json!({"deck": evergreen}).to_string();
    for (why, headers, body) in [
        (
            "a body that is no JSON",
            &json_headers,
            "not json".to_owned(),
        ),
        ("a precon in the wrong content type", &text, precon),
    ] {
        let reply = net::request(
            server.addr,
            "POST",
            "/api/decks/check",
            headers,
            body.as_bytes(),
        )
        .await;
        assert_eq!(reply.status, 401, "{why}, without a session");
    }
    let wrong_type = net::request(
        server.addr,
        "POST",
        "/api/decks/check",
        &[("Content-Type", "text/plain"), ("Cookie", &ann.cookie)],
        json!({"deck": evergreen}).to_string().as_bytes(),
    )
    .await;
    assert_eq!(wrong_type.status, 415, "{}", wrong_type.text());
}

#[sqlx::test(migrations = false)]
async fn the_check_of_a_precon_counts_against_the_precon_budget_of_the_address(pool: PgPool) {
    let server = start(&pool, 3).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let body = json!({"deck": on(EVERGREEN)});
    assert_eq!(anonymous(&server, &body).await.status, 200);
    // Unknown codes count too, and so does an account's request.
    assert_eq!(
        anonymous(&server, &json!({"deck": "precon:nothing-here"}))
            .await
            .status,
        404
    );
    assert_eq!(as_account(&server, &ann, &body).await.status, 200);

    let turned_away = anonymous(&server, &body).await;
    assert_eq!(turned_away.status, 429);
    assert_eq!(turned_away.error_code(), "rate_limited");
    assert!(matches!(turned_away.header("retry-after"), Some("1" | "2")));
    assert_eq!(as_account(&server, &ann, &body).await.status, 429);
    // The browser's endpoints share the budget.
    assert_eq!(
        net::get(server.addr, &format!("/api/precons/{EVERGREEN}"))
            .await
            .status,
        429
    );

    // The check of a stored deck has none: it needs a session.
    let stored = json!({"name": "Leader", "format": "commander", "notes": "", "cards": [
        crate::pg_commander::line("commander", "Questing Beast", 1),
        crate::pg_commander::line("main", "Forest", 99),
    ]});
    let id = crate::pg_commander::store(&server, &ann, &stored).await;
    for _ in 0..5 {
        assert_eq!(
            as_account(&server, &ann, &json!({"deck": id})).await.status,
            200
        );
    }

    tokio::time::sleep(Duration::from_millis(2_300)).await;
    assert_eq!(anonymous(&server, &body).await.status, 200);
}

#[tokio::test]
async fn a_server_without_a_database_cannot_check_a_precon() {
    let server = TestServer::start(bot_config()).await;
    let reply = anonymous(&server, &json!({"deck": on(EVERGREEN)})).await;
    assert_eq!(
        (reply.status, reply.error_code()),
        (503, "cards_unavailable".to_owned()),
        "{}",
        reply.text()
    );
    let reply = anonymous(
        &server,
        &json!({"deck": "4457ed35-7c10-48c8-9776-456485fdf070"}),
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code()),
        (503, "decks_unavailable".to_owned())
    );
}
