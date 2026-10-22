//! Goal: on Postgres the deck rules a table relaxed (singleton off, a copy limit, colour identity off, a deck size, two commanders per seat) let a stored deck and a precon in at creation, at join and on a deck change that the same deck is refused for without the rule, a rule a table did not relax and the rules no table relaxes (a commander that cannot lead, an unknown card) still refuse, and the lobby's deck check answers per table, by game or by described rules, with the issues a seat would be refused with.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net::{Browser, PASSWORD, TestServer, bot_config, register};
use crate::pg_commander::{
    BEAR, GHOST, SHORT, create_body, deck, fixture, join, led_by, line, on, precon_deck, source,
    store,
};
use crate::pg_commander::{bear_pack, beast_pack, choose, ghost_pack, red_pack, short_pack};

use mtg_db::Database;
use mtg_import::mtgjson::PreconDeck;
use mtg_import::scryfall::cards;
use mtg_server::migrate;
use serde_json::{Value, json};
use sqlx::PgPool;

const TWICE: &str = "cmd-twice-pack";
const PAIR: &str = "cmd-pair-pack";
const RED: &str = "cmd-red-pack";

/// A second copy of a card under a commander.
fn twice_pack() -> PreconDeck {
    precon_deck(
        "Twice Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Gemrazer", 2), ("Forest", 97)],
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

async fn start(pool: &PgPool) -> TestServer {
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
    ];
    database
        .importer()
        .import_precons(packs.into_iter().map(Ok))
        .await
        .unwrap();
    TestServer::start_with_cards(pool.clone(), bot_config(), source(pool)).await
}

/// A game of two seats under `rules`; seat 0 is the creator's and has no deck yet.
fn table(rules: &Value) -> Value {
    let mut body = create_body(&[json!({"name": "Ann"}), json!({})], 1);
    body["house_rules"] = rules.clone();
    body
}

async fn open(server: &TestServer, ann: &Browser, rules: &Value) -> String {
    ann.create_game(server.addr, &table(rules)).await.0
}

/// The code of the first issue of a refusal.
fn refused(reply: &crate::net::Reply, why: &str) -> String {
    assert_eq!(reply.status, 422, "{why}: {}", reply.text());
    assert_eq!(reply.error_code(), "deck_blocked", "{why}");
    reply.json()["issues"][0]["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A Commander deck of `commanders`, `copies` Gemrazer and Forests to a hundred.
fn with_courser(commanders: &[&str], copies: u32) -> Value {
    let mut lines: Vec<Value> = commanders
        .iter()
        .map(|name| line("commander", name, 1))
        .collect();
    lines.push(line("main", "Gemrazer", copies));
    let others = 100 - u32::try_from(commanders.len()).unwrap() - copies;
    lines.push(line("main", "Forest", others));
    deck("Courser", "commander", &lines)
}

struct Relaxed {
    why: &'static str,
    deck: Value,
    rules: Value,
    /// What the deck is refused for at a table without the rule.
    code: &'static str,
}

fn relaxed() -> Vec<Relaxed> {
    vec![
        Relaxed {
            why: "singleton off",
            deck: with_courser(&["Questing Beast"], 2),
            rules: json!({"deck": {"singleton": false}}),
            code: "copies",
        },
        Relaxed {
            why: "a copy limit",
            deck: with_courser(&["Questing Beast"], 3),
            rules: json!({"deck": {"singleton": false, "max_copies": 3}}),
            code: "copies",
        },
        Relaxed {
            why: "colour identity off",
            deck: deck(
                "Red",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Lightning Bolt", 1),
                    line("main", "Forest", 98),
                ],
            ),
            rules: json!({"deck": {"colour_identity": false}}),
            code: "color_identity",
        },
        Relaxed {
            why: "a deck size",
            deck: deck(
                "Short",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Forest", 50),
                ],
            ),
            rules: json!({"deck": {"deck_size": 51}}),
            code: "deck_size",
        },
        Relaxed {
            why: "two commanders",
            deck: deck(
                "Pair",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("commander", "Rosheen Meanderer", 1),
                    line("main", "Forest", 98),
                ],
            ),
            rules: json!({"commanders_per_seat": 2}),
            code: "commander_pair",
        },
    ]
}

#[sqlx::test(migrations = false)]
async fn a_relaxed_rule_lets_in_the_deck_the_format_refuses_at_creation_join_and_deck_change(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let none = json!({});
    for case in relaxed() {
        let why = case.why;
        let mine = store(&server, &ann, &case.deck).await;
        let theirs = store(&server, &bob, &case.deck).await;

        // At creation: the table with the rule takes it, the one without refuses it.
        let seats = [json!({"name": "Ann", "deck": mine}), json!({})];
        let mut body = create_body(&seats, 1);
        body["house_rules"] = case.rules.clone();
        let made = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(made.status, 201, "{why}: {}", made.text());
        body.as_object_mut().unwrap().remove("house_rules");
        let plain = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(refused(&plain, why), case.code, "{why}");

        // At join; nobody is seated by a refusal.
        let with = open(&server, &ann, &case.rules).await;
        let without = open(&server, &ann, &none).await;
        let joined = join(&server, &bob, &with, &theirs).await;
        assert_eq!(joined.status, 200, "{why}: {}", joined.text());
        let late = join(&server, &bob, &without, &theirs).await;
        assert_eq!(refused(&late, why), case.code, "{why}");

        // On a deck change.
        let with = open(&server, &ann, &case.rules).await;
        let changed = choose(&server, &ann, &with, 0, &mine).await;
        assert_eq!(changed.status, 200, "{why}: {}", changed.text());
        let changed = choose(&server, &ann, &without, 0, &mine).await;
        assert_eq!(refused(&changed, why), case.code, "{why}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_relaxed_table_still_refuses_what_it_did_not_relax(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let singleton_off = json!({"deck": {"singleton": false}});
    let limit_three = json!({"deck": {"singleton": false, "max_copies": 3}});
    let size = json!({"deck": {"deck_size": 51}});
    let identity_off = json!({"deck": {"colour_identity": false}});
    let one = json!({"commanders_per_seat": 1});
    let all = json!({"deck": {"singleton": false, "colour_identity": false, "deck_size": 100},
        "commanders_per_seat": 2});
    let red = relaxed().remove(2).deck;
    let short = relaxed().remove(3).deck;
    let pair = relaxed().remove(4).deck;
    let bears = deck(
        "Bears",
        "commander",
        &[
            line("commander", "Grizzly Bears", 1),
            line("main", "Forest", 99),
        ],
    );
    let cases = [
        (
            "the size is not the table's",
            led_by("Questing Beast"),
            &size,
            "deck_size",
        ),
        (
            "a short deck at a table without a size",
            short,
            &singleton_off,
            "deck_size",
        ),
        (
            "a red card under a green commander",
            red,
            &singleton_off,
            "color_identity",
        ),
        (
            "a fourth copy over the limit",
            with_courser(&["Questing Beast"], 4),
            &limit_three,
            "copies",
        ),
        (
            "a second copy where identity is off",
            with_courser(&["Questing Beast"], 2),
            &identity_off,
            "copies",
        ),
        (
            "a pair at a table of one commander",
            pair,
            &one,
            "commander_pair",
        ),
        (
            "a commander that cannot lead",
            bears,
            &all,
            "commander_eligibility",
        ),
    ];
    for (why, stored, rules, code) in cases {
        let id = store(&server, &ann, &stored).await;
        let seats = [json!({"name": "Ann", "deck": id}), json!({})];
        let mut body = create_body(&seats, 1);
        body["house_rules"] = rules.clone();
        let made = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(refused(&made, why), code, "{why}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_precon_the_check_refuses_only_for_a_relaxed_rule_is_taken_at_that_table(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let none = json!({});
    let packs = [
        (TWICE, json!({"deck": {"singleton": false}}), "copies"),
        (
            RED,
            json!({"deck": {"colour_identity": false}}),
            "color_identity",
        ),
        (SHORT, json!({"deck": {"deck_size": 51}}), "deck_size"),
        (PAIR, json!({"commanders_per_seat": 2}), "commander_pair"),
    ];
    for (code, rules, issue) in packs {
        let precon = on(code);

        // At creation, in one request.
        let seats = [json!({"name": "Ann", "deck": precon}), json!({})];
        let mut body = create_body(&seats, 1);
        body["house_rules"] = rules.clone();
        let made = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(made.status, 201, "{code}: {}", made.text());
        body.as_object_mut().unwrap().remove("house_rules");
        let plain = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(refused(&plain, code), issue, "{code}");

        // At join and on a deck change.
        let with = open(&server, &ann, &rules).await;
        let without = open(&server, &ann, &none).await;
        let joined = join(&server, &bob, &with, &precon).await;
        assert_eq!(joined.status, 200, "{code}: {}", joined.text());
        assert_eq!(
            refused(&join(&server, &bob, &without, &precon).await, code),
            issue
        );
        let with = open(&server, &ann, &rules).await;
        let changed = choose(&server, &ann, &with, 0, &precon).await;
        assert_eq!(changed.status, 200, "{code}: {}", changed.text());
        let changed = choose(&server, &ann, &without, 0, &precon).await;
        assert_eq!(refused(&changed, code), issue, "{code}");
    }

    // What no table relaxes still refuses the precon, wherever it is brought.
    let everything =
        json!({"deck": {"singleton": false, "colour_identity": false}, "commanders_per_seat": 2});
    for (code, issue) in [(BEAR, "commander_eligibility"), (GHOST, "unknown_card")] {
        let game = open(&server, &ann, &everything).await;
        let choice = choose(&server, &ann, &game, 0, &on(code)).await;
        refused(&choice, code);
        let issues = choice.json()["issues"].to_string();
        assert!(issues.contains(issue), "{code}: {issues}");
    }
}

/// `POST /api/decks/check` for `body`, which must succeed.
async fn check(server: &TestServer, who: &Browser, body: &Value) -> Value {
    let reply = who
        .post(server.addr, "/api/decks/check", &body.to_string())
        .await;
    assert_eq!(reply.status, 200, "{body}: {}", reply.text());
    reply.json()
}

fn codes(report: &Value) -> Vec<String> {
    report["legality"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = false)]
async fn the_lobby_check_answers_for_the_table_of_a_game_or_of_described_rules(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let twice = store(&server, &ann, &with_courser(&["Questing Beast"], 2)).await;
    let good = store(&server, &ann, &led_by("Questing Beast")).await;
    let singleton_off = json!({"deck": {"singleton": false}});
    let relaxed_game = open(&server, &ann, &singleton_off).await;
    let plain_game = open(&server, &ann, &json!({})).await;

    // The same deck, two tables.
    let report = check(&server, &ann, &json!({"deck": twice, "game": relaxed_game})).await;
    assert!(codes(&report).is_empty(), "{report}");
    assert_ne!(report["summary"]["state"], "blocked");
    assert_eq!(report["summary"]["issues"], 0);
    let report = check(&server, &ann, &json!({"deck": twice, "game": plain_game})).await;
    assert_eq!(codes(&report), ["copies"]);
    assert_eq!(report["summary"]["state"], "blocked");
    assert_eq!(report["summary"]["issues"], 1);

    // Rules that no game holds yet.
    let described = |rules: Value| json!({"format": "commander", "house_rules": rules});
    let report = check(
        &server,
        &ann,
        &json!({"deck": twice, "table": described(singleton_off.clone())}),
    )
    .await;
    assert!(codes(&report).is_empty(), "{report}");
    let report = check(
        &server,
        &ann,
        &json!({"deck": twice, "table": {"format": "commander"}}),
    )
    .await;
    assert_eq!(codes(&report), ["copies"]);

    // The engine's check under the table's rules judges a deck the format accepts.
    let size = json!({"table": described(json!({"deck": {"deck_size": 51}})), "deck": good});
    let report = check(&server, &ann, &size).await;
    assert_eq!(codes(&report), ["deck_size"]);
    assert!(
        report["legality"][0]["message"]
            .as_str()
            .unwrap()
            .contains("51")
    );
    let normal = json!({"table": {"format": "normal"}, "deck": good});
    assert_eq!(
        codes(&check(&server, &ann, &normal).await),
        ["no_commander_zone"]
    );
    let report = check(&server, &ann, &json!({"deck": good})).await;
    assert!(
        codes(&report).is_empty(),
        "a check without a table is the deck's own"
    );

    // An unsaved list is judged the same way.
    let list = json!({"format": "commander", "table": described(singleton_off.clone()), "cards": [
        {"zone": "commander", "oracle_id": line("commander", "Questing Beast", 1)["oracle_id"], "count": 1},
        {"zone": "main", "oracle_id": line("main", "Gemrazer", 2)["oracle_id"], "count": 2},
        {"zone": "main", "oracle_id": line("main", "Forest", 97)["oracle_id"], "count": 97},
    ]});
    let report = check(&server, &ann, &list).await;
    assert!(codes(&report).is_empty(), "{report}");

    // A request that names no table the server knows.
    let refused_with = |body: Value, status: u16, code: &'static str| {
        let ann = &ann;
        let server = &server;
        async move {
            let reply = ann
                .post(server.addr, "/api/decks/check", &body.to_string())
                .await;
            assert_eq!(
                (reply.status, reply.error_code()),
                (status, code.to_owned()),
                "{body}"
            );
            reply
        }
    };
    let both = json!({"deck": twice, "game": plain_game, "table": {"format": "commander"}});
    refused_with(both, 400, "invalid_request").await;
    let ghost = json!({"deck": twice, "game": "00000000-0000-4000-8000-000000000001"});
    refused_with(ghost, 404, "not_found").await;
    refused_with(json!({"deck": twice, "game": "nope"}), 404, "not_found").await;
    refused_with(
        json!({"deck": twice, "table": {"format": "modern"}}),
        400,
        "invalid_request",
    )
    .await;
    let range = json!({"deck": twice, "table": described(json!({"deck": {"deck_size": 5}}))});
    let reply = refused_with(range, 400, "invalid_request").await;
    assert_eq!(reply.json()["field"], "deck.deck_size");
}
