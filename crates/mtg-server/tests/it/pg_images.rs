//! Goal: the picture endpoints answer over real HTTP on Postgres from a fake CDN: a card, printing, face, size and token pick the stored link and the picture arrives byte for byte with its type, week-long caching rule and `ETag`; a repeat is served from the cache and a matching `If-None-Match` is a 304 that needs neither cache nor CDN; requests for one picture make one CDN request, failure included; only https links under scryfall.io are fetched; a failing, lying or busy CDN leaves the cache clean and is waited out as the politeness rules say; the cache stays under its limit; and bad paths and queries are 400s and missing pictures the typed 404.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::cdn::{Behavior, Cdn, body_for};
use crate::net;

use std::path::Path;
use std::time::Duration;

use futures_util::future::join_all;
use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, ImageConfig, ImageLink, migrate};
use net::{TestServer, bot_config, get, request};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

fn id(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

const ALPHA: u128 = 1;
const BETA: u128 = 2;
const GAMMA: u128 = 3;
const DELTA: u128 = 4;
const EPSILON: u128 = 5;
const HOSTILE: u128 = 6;
const PLAIN: u128 = 7;
const ODD: u128 = 8;
const GOBLIN_RED: u128 = 9;
const GOBLIN_BLACK: u128 = 10;
const TREASURE: u128 = 11;

/// Printings of Alpha: the first two are paper, the digital one is newer, the last has no picture.
const ALPHA_2020: u128 = 11;
const ALPHA_2022: u128 = 12;
const ALPHA_DIGITAL: u128 = 13;
const ALPHA_BLANK: u128 = 14;

/// The links of the sizes named, as Scryfall writes them for the picture called `tag`.
fn links(tag: &str, side: &str, sizes: &[&str]) -> Value {
    let mut uris = serde_json::Map::new();
    for size in sizes {
        let extension = if *size == "png" { "png" } else { "jpg" };
        uris.insert(
            (*size).to_owned(),
            json!(format!(
                "https://cards.scryfall.io/{size}/{side}/t/s/{tag}.{extension}?1"
            )),
        );
    }
    Value::Object(uris)
}

const ALL_SIZES: [&str; 5] = ["small", "normal", "large", "art_crop", "png"];

/// A Scryfall object for one printing; `extra` adds or replaces members.
fn printing(oracle: u128, printing: u128, name: &str, extra: &Value) -> Value {
    let mut object = json!({
        "object": "card", "id": id(printing), "oracle_id": id(oracle), "name": name,
        "lang": "en", "released_at": "2020-01-01", "layout": "normal", "cmc": 1.0,
        "type_line": "Instant", "colors": ["R"], "color_identity": ["R"],
        "legalities": {}, "set": "tst", "collector_number": printing.to_string(), "rarity": "common",
    });
    let members = object.as_object_mut().unwrap();
    for (key, value) in extra.as_object().unwrap() {
        members.insert(key.clone(), value.clone());
    }
    object
}

fn face(name: &str, tag: &str, side: &str) -> Value {
    json!({
        "object": "card_face", "name": name, "type_line": "Creature", "oracle_text": "",
        "colors": ["U"], "image_uris": links(tag, side, &["small", "normal"]),
    })
}

fn bare_face(name: &str) -> Value {
    json!({"object": "card_face", "name": name, "type_line": "Instant", "oracle_text": "", "colors": ["R"]})
}

fn token(
    oracle: u128,
    name: &str,
    type_line: &str,
    colors: &Value,
    pt: (&str, &str),
    tag: &str,
) -> Value {
    printing(
        oracle,
        oracle + 100,
        name,
        &json!({
            "layout": "token", "type_line": type_line, "colors": colors, "color_identity": colors,
            "power": pt.0, "toughness": pt.1, "image_uris": links(tag, "front", &["normal"]),
        }),
    )
}

/// Every card of the tests as the one JSON array the importer streams.
fn fixture() -> String {
    let all = vec![
        printing(
            ALPHA,
            ALPHA_2020,
            "Alpha Bolt",
            &json!({"image_uris": links("alpha-2020", "front", &ALL_SIZES)}),
        ),
        printing(
            ALPHA,
            ALPHA_2022,
            "Alpha Bolt",
            &json!({"released_at": "2022-06-01", "image_uris": links("alpha-2022", "front", &ALL_SIZES)}),
        ),
        printing(
            ALPHA,
            ALPHA_DIGITAL,
            "Alpha Bolt",
            &json!({"released_at": "2024-01-01", "digital": true, "image_uris": links("alpha-digital", "front", &ALL_SIZES)}),
        ),
        printing(
            ALPHA,
            ALPHA_BLANK,
            "Alpha Bolt",
            &json!({"released_at": "2023-01-01"}),
        ),
        printing(
            BETA,
            21,
            "Beta Shock",
            &json!({"image_uris": links("beta", "front", &["small", "normal"])}),
        ),
        printing(
            GAMMA,
            31,
            "Gamma Delver // Gamma Insect",
            &json!({"layout": "transform", "card_faces": [face("Gamma Delver", "gamma", "front"), face("Gamma Insect", "gamma", "back")]}),
        ),
        printing(
            DELTA,
            41,
            "Delta Fire // Delta Ice",
            &json!({"layout": "split", "card_faces": [bare_face("Delta Fire"), bare_face("Delta Ice")], "image_uris": links("delta", "front", &["normal"])}),
        ),
        printing(EPSILON, 51, "Epsilon Blank", &json!({})),
        printing(
            HOSTILE,
            61,
            "Zeta Hostile",
            &json!({"image_uris": {"normal": "https://evil.example/normal/front/h/o/hostile.jpg?1"}}),
        ),
        printing(
            PLAIN,
            71,
            "Eta Plain",
            &json!({"image_uris": {"normal": "http://cards.scryfall.io/normal/front/p/l/plain.jpg?1"}}),
        ),
        printing(
            ODD,
            81,
            "Theta Odd",
            &json!({"image_uris": {"normal": "https://cards.scryfall.io/normal/front/o/d/odd.html?1"}}),
        ),
        token(
            GOBLIN_RED,
            "Goblin",
            "Token Creature \u{2014} Goblin",
            &json!(["R"]),
            ("1", "1"),
            "goblin-red",
        ),
        token(
            GOBLIN_BLACK,
            "Goblin",
            "Token Creature \u{2014} Goblin",
            &json!(["B"]),
            ("2", "2"),
            "goblin-black",
        ),
        token(
            TREASURE,
            "Treasure",
            "Token Artifact \u{2014} Treasure",
            &json!([]),
            ("", ""),
            "treasure",
        ),
    ];
    serde_json::to_string(&all).unwrap()
}

/// What the test server is told about its cache and its tries.
struct Setup {
    max_bytes: u64,
    backoff: Duration,
}

impl Default for Setup {
    fn default() -> Setup {
        Setup {
            max_bytes: 1 << 30,
            backoff: Duration::from_millis(5),
        }
    }
}

fn image_config(cdn: &Cdn, dir: &Path, setup: &Setup) -> ImageConfig {
    ImageConfig {
        cache_dir: dir.join("cache"),
        max_bytes: setup.max_bytes,
        upstream_base: Some(cdn.base()),
        min_gap: Duration::ZERO,
        attempts: 3,
        backoff: setup.backoff,
        timeout: Duration::from_secs(5),
    }
}

async fn import(pool: &PgPool) {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
}

/// A server over the imported cards whose pictures come from `cdn`.
async fn serve(pool: &PgPool, cdn: &Cdn, dir: &Path, setup: &Setup) -> TestServer {
    let mut config = bot_config();
    config.images = image_config(cdn, dir, setup);
    TestServer::start_with_cards(pool.clone(), config, CardSource::new(pool.clone())).await
}

/// Both schemas migrated, the cards imported and a server whose pictures come from `cdn`.
async fn start(pool: &PgPool, cdn: &Cdn, dir: &Path, setup: &Setup) -> TestServer {
    import(pool).await;
    serve(pool, cdn, dir, setup).await
}

fn cached_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir.join("cache")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn oracle_path(card: u128, face: &str, size: &str) -> String {
    format!("/api/images/oracle/{}/{face}/{size}", id(card))
}

fn printing_path(printing: u128, face: &str, size: &str) -> String {
    format!("/api/images/printing/{}/{face}/{size}", id(printing))
}

/// What a request must give: the path the CDN is asked for, or the status and error code.
enum Expect {
    Picture(&'static str),
    Refused(u16, &'static str),
}

use Expect::{Picture, Refused};

fn check(path: &str, reply: &net::Reply, expect: &Expect) {
    match expect {
        Picture(upstream) => {
            assert_eq!(reply.status, 200, "{path}: {}", reply.text());
            assert_eq!(
                reply.body,
                body_for(upstream),
                "{path}: the picture is the CDN's, unaltered"
            );
            let kind = if upstream.contains(".png") {
                "image/png"
            } else {
                "image/jpeg"
            };
            assert_eq!(reply.header("content-type"), Some(kind), "{path}");
            assert_eq!(
                reply.header("cache-control"),
                Some("public, max-age=604800"),
                "{path}"
            );
            assert!(
                reply.header("etag").is_some_and(|tag| tag.starts_with('"')),
                "{path}"
            );
        }
        Refused(status, code) => {
            assert_eq!(
                (reply.status, reply.error_code()),
                (*status, (*code).to_owned()),
                "{path}: {}",
                reply.text()
            );
            assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
        }
    }
}

#[sqlx::test(migrations = false)]
async fn identity_face_and_size_pick_the_picture(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let cases: Vec<(String, Expect)> = vec![
        // The newest paper printing with a picture stands for the card.
        (
            oracle_path(ALPHA, "0", "normal"),
            Picture("/normal/front/t/s/alpha-2022.jpg?1"),
        ),
        (
            oracle_path(ALPHA, "0", "small"),
            Picture("/small/front/t/s/alpha-2022.jpg?1"),
        ),
        (
            oracle_path(ALPHA, "0", "large"),
            Picture("/large/front/t/s/alpha-2022.jpg?1"),
        ),
        (
            oracle_path(ALPHA, "0", "art_crop"),
            Picture("/art_crop/front/t/s/alpha-2022.jpg?1"),
        ),
        (
            oracle_path(ALPHA, "0", "png"),
            Picture("/png/front/t/s/alpha-2022.png?1"),
        ),
        (oracle_path(ALPHA, "1", "normal"), Refused(404, "no_image")),
        (
            oracle_path(BETA, "0", "normal"),
            Picture("/normal/front/t/s/beta.jpg?1"),
        ),
        (oracle_path(BETA, "0", "png"), Refused(404, "no_image")),
        (oracle_path(BETA, "0", "large"), Refused(404, "no_image")),
        // A card with a picture per face.
        (
            oracle_path(GAMMA, "0", "normal"),
            Picture("/normal/front/t/s/gamma.jpg?1"),
        ),
        (
            oracle_path(GAMMA, "1", "normal"),
            Picture("/normal/back/t/s/gamma.jpg?1"),
        ),
        (
            oracle_path(GAMMA, "1", "small"),
            Picture("/small/back/t/s/gamma.jpg?1"),
        ),
        (oracle_path(GAMMA, "2", "normal"), Refused(404, "no_image")),
        (oracle_path(GAMMA, "0", "large"), Refused(404, "no_image")),
        // A card whose one picture shows both halves.
        (
            oracle_path(DELTA, "0", "normal"),
            Picture("/normal/front/t/s/delta.jpg?1"),
        ),
        (
            oracle_path(DELTA, "1", "normal"),
            Picture("/normal/front/t/s/delta.jpg?1"),
        ),
        (oracle_path(DELTA, "2", "normal"), Refused(404, "no_image")),
        // No picture, an unknown card and a malformed id look alike.
        (
            oracle_path(EPSILON, "0", "normal"),
            Refused(404, "no_image"),
        ),
        (oracle_path(999, "0", "normal"), Refused(404, "no_image")),
        (
            "/api/images/oracle/not-an-id/0/normal".to_owned(),
            Refused(404, "no_image"),
        ),
        // One printing, whatever its age or kind.
        (
            printing_path(ALPHA_2020, "0", "normal"),
            Picture("/normal/front/t/s/alpha-2020.jpg?1"),
        ),
        (
            printing_path(ALPHA_DIGITAL, "0", "normal"),
            Picture("/normal/front/t/s/alpha-digital.jpg?1"),
        ),
        (
            printing_path(ALPHA_BLANK, "0", "normal"),
            Refused(404, "no_image"),
        ),
        (printing_path(999, "0", "normal"), Refused(404, "no_image")),
        (
            "/api/images/printing/nope/0/normal".to_owned(),
            Refused(404, "no_image"),
        ),
        // A bad face or size is told apart from a missing picture.
        (
            oracle_path(ALPHA, "0", "border_crop"),
            Refused(400, "invalid_request"),
        ),
        (
            oracle_path(ALPHA, "0", "NORMAL"),
            Refused(400, "invalid_request"),
        ),
        (
            oracle_path(ALPHA, "0", "huge"),
            Refused(400, "invalid_request"),
        ),
        (
            oracle_path(ALPHA, "x", "normal"),
            Refused(400, "invalid_request"),
        ),
        (
            oracle_path(ALPHA, "-1", "normal"),
            Refused(400, "invalid_request"),
        ),
        (
            oracle_path(ALPHA, "99999999999999999999999", "normal"),
            Refused(400, "invalid_request"),
        ),
    ];
    for (path, expect) in cases {
        let reply = get(server.addr, &path).await;
        check(&path, &reply, &expect);
    }
}

#[sqlx::test(migrations = false)]
async fn tokens_are_found_by_name_and_every_fact_given(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let dash = "%E2%80%94";
    let red = Picture("/normal/front/t/s/goblin-red.jpg?1");
    let black = Picture("/normal/front/t/s/goblin-black.jpg?1");
    let treasure = Picture("/normal/front/t/s/treasure.jpg?1");
    let cases: Vec<(String, Expect)> = vec![
        ("name=Goblin&colors=R&power=1&toughness=1".to_owned(), red),
        ("name=goblin&colors=b&power=2&toughness=2".to_owned(), black),
        (
            format!("name=Goblin&type=Creature%20{dash}%20Goblin&power=2"),
            Picture("/normal/front/t/s/goblin-black.jpg?1"),
        ),
        (
            format!("name=Goblin&type=Token%20Creature%20{dash}%20Goblin&colors=R"),
            Picture("/normal/front/t/s/goblin-red.jpg?1"),
        ),
        ("name=Treasure&colors=C".to_owned(), treasure),
        (
            "name=Treasure".to_owned(),
            Picture("/normal/front/t/s/treasure.jpg?1"),
        ),
        ("name=Goblin&colors=G".to_owned(), Refused(404, "no_image")),
        (
            "name=Goblin&colors=R&power=3".to_owned(),
            Refused(404, "no_image"),
        ),
        (
            "name=Goblin&colors=R&toughness=9".to_owned(),
            Refused(404, "no_image"),
        ),
        (
            "name=Goblin&type=Creature%20-%20Zombie".to_owned(),
            Refused(404, "no_image"),
        ),
        (
            "name=Treasure&colors=R".to_owned(),
            Refused(404, "no_image"),
        ),
        ("name=Nothing".to_owned(), Refused(404, "no_image")),
        // A card of that name that is not a token is not a token.
        ("name=Alpha%20Bolt".to_owned(), Refused(404, "no_image")),
        (String::new(), Refused(400, "invalid_request")),
        ("name=%20".to_owned(), Refused(400, "invalid_request")),
        (
            "name=Goblin&colors=RX".to_owned(),
            Refused(400, "invalid_request"),
        ),
        (
            "name=Goblin&size=big".to_owned(),
            Refused(400, "invalid_request"),
        ),
    ];
    for (query, expect) in cases {
        let path = format!("/api/images/token/0/normal?{query}");
        let reply = get(server.addr, &path).await;
        match expect {
            Picture(upstream) => {
                assert_eq!(reply.status, 200, "{path}: {}", reply.text());
                assert_eq!(reply.body, body_for(upstream), "{path}");
            }
            Refused(status, code) => assert_eq!(
                (reply.status, reply.error_code()),
                (status, code.to_owned()),
                "{path}"
            ),
        }
    }
    let bad_face = get(server.addr, "/api/images/token/x/normal?name=Goblin").await;
    assert_eq!(bad_face.status, 400);
}

#[sqlx::test(migrations = false)]
async fn a_picture_is_fetched_once_kept_and_validated_by_its_tag(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let path = oracle_path(BETA, "0", "normal");

    let first = get(server.addr, &path).await;
    assert_eq!(first.status, 200);
    let tag = first.header("etag").unwrap().to_owned();
    let link = ImageLink::parse("https://cards.scryfall.io/normal/front/t/s/beta.jpg?1").unwrap();
    assert_eq!(tag, link.etag(), "the tag follows the stored link");
    assert_eq!(cached_files(dir.path()), ["normal_front_t_s_beta.jpg@1"]);

    let again = get(server.addr, &path).await;
    assert_eq!((again.status, &again.body), (200, &first.body));
    assert_eq!(cdn.count(), 1, "the second answer came from the cache");

    let sent = [
        tag.clone(),
        format!("W/{tag}"),
        format!("\"other\", {tag}"),
        "*".to_owned(),
    ];
    for sent in sent {
        let reply = request(server.addr, "GET", &path, &[("If-None-Match", &sent)], b"").await;
        assert_eq!(reply.status, 304, "{sent}");
        assert_eq!(reply.body, Vec::<u8>::new());
        assert_eq!(reply.header("etag"), Some(tag.as_str()));
        assert_eq!(
            reply.header("cache-control"),
            Some("public, max-age=604800")
        );
    }
    let stale = request(
        server.addr,
        "GET",
        &path,
        &[("If-None-Match", "\"stale\"")],
        b"",
    )
    .await;
    assert_eq!((stale.status, stale.body), (200, first.body.clone()));

    let head = request(server.addr, "HEAD", &path, &[], b"").await;
    assert_eq!(
        (head.status, head.header("etag")),
        (200, Some(tag.as_str()))
    );
    assert_eq!(head.body, Vec::<u8>::new());
    assert_eq!(cdn.count(), 1);
}

#[sqlx::test(migrations = false)]
async fn a_matching_tag_needs_neither_cache_nor_cdn(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let tag = ImageLink::parse("https://cards.scryfall.io/normal/front/t/s/beta.jpg?1")
        .unwrap()
        .etag();
    let reply = request(
        server.addr,
        "GET",
        &oracle_path(BETA, "0", "normal"),
        &[("If-None-Match", &tag)],
        b"",
    )
    .await;
    assert_eq!(reply.status, 304);
    assert_eq!(cdn.count(), 0);
    assert_eq!(cached_files(dir.path()), Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn requests_for_one_picture_make_one_cdn_request(pool: PgPool) {
    let cdn = Cdn::start().await;
    cdn.set(Behavior::Delay(Duration::from_millis(300)));
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let path = oracle_path(ALPHA, "0", "normal");
    let replies = join_all((0..10).map(|_| get(server.addr, &path))).await;
    for reply in &replies {
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, body_for("/normal/front/t/s/alpha-2022.jpg?1"));
    }
    assert_eq!(cdn.count(), 1, "ten requests, one trip to the CDN");
}

#[sqlx::test(migrations = false)]
async fn a_failure_is_shared_by_the_requests_that_waited_for_it(pool: PgPool) {
    let cdn = Cdn::start().await;
    cdn.set(Behavior::Fail(500));
    let dir = tempfile::tempdir().unwrap();
    // The waits between the tries keep the one fetch under way while every request arrives.
    let setup = Setup {
        backoff: Duration::from_millis(150),
        ..Setup::default()
    };
    let server = start(&pool, &cdn, dir.path(), &setup).await;
    let path = oracle_path(BETA, "0", "normal");
    let replies = join_all((0..8).map(|_| get(server.addr, &path))).await;
    assert!(replies.iter().all(|reply| reply.status == 502));
    assert_eq!(
        cdn.count(),
        3,
        "eight requests shared the three tries of one"
    );
}

#[sqlx::test(migrations = false)]
async fn only_https_links_under_scryfall_io_are_fetched(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    for card in [HOSTILE, PLAIN, ODD] {
        let reply = get(server.addr, &oracle_path(card, "0", "normal")).await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (404, "no_image".to_owned()),
            "{card}"
        );
    }
    assert_eq!(cdn.count(), 0, "no request left the server");
    assert_eq!(cached_files(dir.path()), Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn a_failing_or_lying_cdn_leaves_the_cache_clean(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let path = oracle_path(BETA, "0", "normal");
    // (what the CDN does, the answer, requests it saw)
    let cases = [
        (Behavior::Fail(500), 502, "image_unavailable", 3),
        (Behavior::Fail(503), 502, "image_unavailable", 3),
        (Behavior::Fail(403), 502, "image_unavailable", 1),
        (Behavior::Fail(404), 404, "no_image", 1),
        (Behavior::Fail(410), 404, "no_image", 1),
        (Behavior::Page, 502, "image_unavailable", 1),
    ];
    for (behavior, status, code, requests) in cases {
        let before = cdn.count();
        cdn.set(behavior.clone());
        let reply = get(server.addr, &path).await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (status, code.to_owned()),
            "{behavior:?}"
        );
        assert_eq!(cdn.count() - before, requests, "{behavior:?}");
        assert!(
            cached_files(dir.path()).is_empty(),
            "{behavior:?} left a file"
        );
    }
    cdn.set(Behavior::Serve);
    let reply = get(server.addr, &path).await;
    assert_eq!(
        reply.status, 200,
        "the next try after a failure fetches again"
    );
    assert_eq!(cached_files(dir.path()), ["normal_front_t_s_beta.jpg@1"]);
}

#[sqlx::test(migrations = false)]
async fn a_busy_cdn_is_waited_out_and_asked_again(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let busy = |remaining, status, retry_after| Behavior::FailFirst {
        remaining,
        status,
        retry_after,
    };
    let cases = [
        (BETA, busy(2, 429, None), 200, 3),
        (ALPHA, busy(2, 503, Some(0)), 200, 3),
        (GAMMA, busy(1, 408, None), 200, 2),
        (DELTA, busy(3, 429, None), 502, 3),
    ];
    for (card, behavior, status, requests) in cases {
        let before = cdn.count();
        cdn.set(behavior.clone());
        let reply = get(server.addr, &oracle_path(card, "0", "normal")).await;
        assert_eq!(reply.status, status, "{behavior:?}");
        assert_eq!(cdn.count() - before, requests, "{behavior:?}");
    }
}

#[sqlx::test(migrations = false)]
async fn requests_to_the_cdn_say_who_asks(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let reply = get(server.addr, &oracle_path(BETA, "0", "normal")).await;
    assert_eq!(reply.status, 200);
    let hits = cdn.hits();
    assert_eq!(hits.len(), 1);
    let agent = hits[0].user_agent.as_deref().unwrap();
    assert!(agent.starts_with("mtg-cli/"), "{agent}");
    assert!(
        hits[0]
            .accept
            .as_deref()
            .is_some_and(|accept| accept.contains("image/"))
    );
}

#[sqlx::test(migrations = false)]
async fn the_cache_drops_the_least_recently_used_pictures_at_its_limit(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = [
        "/normal/front/t/s/alpha-2022.jpg?1",
        "/normal/front/t/s/beta.jpg?1",
        "/normal/front/t/s/gamma.jpg?1",
    ];
    let mut sizes: Vec<u64> = paths.iter().map(|p| body_for(p).len() as u64).collect();
    sizes.sort_unstable();
    // Room for the two biggest: the third picture in pushes the first out.
    let setup = Setup {
        max_bytes: sizes[1] + sizes[2],
        ..Setup::default()
    };
    let server = start(&pool, &cdn, dir.path(), &setup).await;
    for card in [ALPHA, BETA, GAMMA] {
        let reply = get(server.addr, &oracle_path(card, "0", "normal")).await;
        assert_eq!(reply.status, 200);
    }
    assert_eq!(cdn.count(), 3);
    assert_eq!(
        cached_files(dir.path()).len(),
        2,
        "{:?}",
        cached_files(dir.path())
    );
    let newest = get(server.addr, &oracle_path(GAMMA, "0", "normal")).await;
    assert_eq!(
        (newest.status, cdn.count()),
        (200, 3),
        "the newest is still there"
    );
    let oldest = get(server.addr, &oracle_path(ALPHA, "0", "normal")).await;
    assert_eq!(
        (oldest.status, cdn.count()),
        (200, 4),
        "the oldest was dropped and is fetched again"
    );
}

#[sqlx::test(migrations = false)]
async fn a_restarted_server_serves_the_pictures_already_on_disk(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let path = oracle_path(BETA, "0", "normal");
    let first = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    assert_eq!(get(first.addr, &path).await.status, 200);
    drop(first);
    let second = serve(&pool, &cdn, dir.path(), &Setup::default()).await;
    assert_eq!(get(second.addr, &path).await.status, 200);
    assert_eq!(
        cdn.count(),
        1,
        "the second server read the file the first one wrote"
    );
}

#[tokio::test]
async fn a_server_without_a_database_has_no_pictures() {
    let server = TestServer::start(bot_config()).await;
    let reply = get(server.addr, &oracle_path(ALPHA, "0", "normal")).await;
    assert_eq!(
        (reply.status, reply.error_code()),
        (503, "cards_unavailable".to_owned())
    );
}

#[sqlx::test(migrations = false)]
async fn other_methods_and_paths_are_refused(pool: PgPool) {
    let cdn = Cdn::start().await;
    let dir = tempfile::tempdir().unwrap();
    let server = start(&pool, &cdn, dir.path(), &Setup::default()).await;
    let post = request(
        server.addr,
        "POST",
        &oracle_path(BETA, "0", "normal"),
        &[],
        b"{}",
    )
    .await;
    assert_eq!(post.status, 405);
    let short = get(server.addr, &format!("/api/images/oracle/{}", id(BETA))).await;
    assert_eq!(short.status, 404);
    assert_eq!(cdn.count(), 0);
}
