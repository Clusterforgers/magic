//! Goal: the server loads the current drafts with their read state into the library of a game: a trusted Forge draft (source `forge`, Oracle text matched) is played automatically as `forge`, a draft a player approved as `read`, any other draft and any draft with a `wrong` verdict by hand; a game pins the drafts it plays in its recipe, so a verdict never changes a game that runs, its resume builds the same cards, and a pinned draft that is gone fails the game; seats that chose under different draft states play one definition per card, so the game resumes; a draft load that failed is not kept as current; the next game after a `wrong` verdict is played by hand; an ability the game leaves to the table records the server's own `wrong`; and a fix mark on a card that runs on a read records the account's `wrong`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use common::{Bot, drive, roomy};
use mtg_cards::card::Card;
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::support::{Support, read_hash};
use mtg_cards::{PrintedCard, builtin};
use mtg_db::{Database, DraftKey, NewDraft, SOURCE_FORGE, SOURCE_SWEEP, Verdict};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::cards;
use mtg_protocol::ClientMessage;
use mtg_server::{
    CardSource, DeckRef, GameId, GameRecipe, GameRecords, RecordStatus, Registry, ServerConfig,
    migrate, printed_card,
};
use net::lobby;
use net::{PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// A green sorcery with no target that the corpus writes by hand; the tests keep it out of the rules files and read
/// it as a draft instead.
const CARD: &str = "Explore";

fn scryfall() -> String {
    mtg_testkit::corpus::scryfall_array().unwrap()
}

fn id_of(name: &str) -> String {
    let record = cards(scryfall().as_bytes())
        .map(Result::unwrap)
        .find(|record| record.name == name);
    record.unwrap().oracle_id.to_string()
}

/// The embedded rules without the file of [`CARD`].
fn without_card() -> Library {
    let skipped = format!("cards/{}.json", slug(CARD));
    let sources: Vec<Source> = builtin::sources()
        .into_iter()
        .filter(|source| source.path != skipped)
        .collect();
    Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"))
}

async fn seeded(pool: &PgPool) -> Database {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(scryfall().as_bytes()))
        .await
        .unwrap();
    db
}

fn source(pool: &PgPool) -> CardSource {
    CardSource::new(pool.clone()).with_rules(without_card())
}

async fn printed(db: &Database) -> PrintedCard {
    let stored = db.cards().get(&OracleId(id_of(CARD))).await.unwrap();
    printed_card(&stored.unwrap())
}

/// The card's rules as a draft whose spell, when `faulty`, fails when it resolves: it changes a power too large for a game.
fn rules(faulty: bool) -> Card {
    let mut card = builtin::library()
        .unwrap()
        .card_by_name(CARD)
        .unwrap()
        .clone();
    if faulty {
        let failing = json!({"Continuous": {
            "affected": {"Each": [{"Type": "Creature"}]},
            "mods": [{"ModifyPT": {"power": {"Sum": [i32::MAX, i32::MAX]}, "toughness": 0}}],
            "duration": "UntilEndOfTurn",
        }});
        let mut json = serde_json::to_value(&card).unwrap();
        json["faces"][0]["abilities"][0]["Spell"]["effects"] = json!([failing]);
        card = serde_json::from_value(json).unwrap();
    }
    card
}

/// Stores the draft of [`CARD`]; returns its key.
async fn draft(db: &Database, source: &str, matches: bool, faulty: bool) -> DraftKey {
    let card = rules(faulty);
    let key = DraftKey {
        oracle_id: OracleId(id_of(CARD)),
        read_hash: read_hash(&printed(db).await, &card),
    };
    db.drafts()
        .insert(&[NewDraft {
            oracle_id: key.oracle_id.clone(),
            read_hash: key.read_hash.clone(),
            rules: serde_json::to_value(&card).unwrap(),
            source: source.to_owned(),
            oracle_matches: matches,
        }])
        .await
        .unwrap();
    key
}

async fn verdict(db: &Database, key: &DraftKey, verdict: Verdict, by: Option<u128>) {
    let note = (verdict == Verdict::Wrong).then_some("it does not work");
    let by = by.map(Uuid::from_u128);
    let made = db.reads().append(by, key, verdict, note).await.unwrap();
    assert!(made.is_some());
}

async fn support(source: &CardSource) -> Support {
    let assembled = source.assemble(&[id_of(CARD)]).await.unwrap();
    assembled[&id_of(CARD)].support.support
}

/// A draft's source, whether its text matched, the verdicts given on it and the support the card then has.
type Case = (
    &'static str,
    &'static str,
    bool,
    &'static [Verdict],
    Support,
);

const CASES: &[Case] = &[
    (
        "a sweep draft nobody read",
        SOURCE_SWEEP,
        false,
        &[],
        Support::Manual,
    ),
    (
        "a sweep draft a player approved",
        SOURCE_SWEEP,
        false,
        &[Verdict::Correct],
        Support::Read,
    ),
    (
        "a trusted Forge draft",
        SOURCE_FORGE,
        true,
        &[],
        Support::Forge,
    ),
    (
        "a Forge draft a player also approved",
        SOURCE_FORGE,
        true,
        &[Verdict::Correct],
        Support::Forge,
    ),
    (
        "a trusted Forge draft a player found wrong",
        SOURCE_FORGE,
        true,
        &[Verdict::Wrong],
        Support::Manual,
    ),
    (
        "a Forge draft whose text did not match",
        SOURCE_FORGE,
        false,
        &[],
        Support::Manual,
    ),
    (
        "a Forge draft whose text did not match, approved",
        SOURCE_FORGE,
        false,
        &[Verdict::Correct],
        Support::Read,
    ),
];

#[sqlx::test(migrations = false)]
async fn the_support_of_a_card_follows_the_source_of_its_draft_and_the_verdicts(pool: PgPool) {
    for (name, origin, matches, verdicts, expected) in CASES {
        let db = seeded(&pool).await;
        let source = source(&pool);
        assert_eq!(support(&source).await, Support::Manual, "{name}: no draft");
        let key = draft(&db, origin, *matches, false).await;
        for given in *verdicts {
            verdict(&db, &key, *given, Some(1)).await;
        }
        assert_eq!(support(&source).await, *expected, "{name}");
        sqlx::query("DELETE FROM card_drafts")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(support(&source).await, Support::Manual, "{name}: removed");
    }
}

#[sqlx::test(migrations = false)]
async fn a_verdict_moves_the_library_of_a_running_source_at_once(pool: PgPool) {
    let db = seeded(&pool).await;
    let source = source(&pool);
    let key = draft(&db, SOURCE_FORGE, true, false).await;
    assert_eq!(support(&source).await, Support::Forge);

    verdict(&db, &key, Verdict::Wrong, Some(2)).await;
    assert_eq!(support(&source).await, Support::Manual);
    let deck = source.deck(&[id_of(CARD)]).await.unwrap();
    assert!(!deck[0].read && deck[0].abilities.is_empty() && deck[0].rules.is_empty());
}

// ---- games ----

/// Forty Forests, four copies of four other cards (two creatures, so there is one to be changed) and four copies of the card.
fn ids() -> Vec<String> {
    let mut ids = vec![id_of("Forest"); 40];
    for name in ["Grizzly Bears", "Llanowar Elves", "Sol Ring", "Fireball"] {
        ids.extend(vec![id_of(name); 4]);
    }
    ids.extend(vec![id_of(CARD); 4]);
    ids
}

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn registry(pool: &PgPool, config: &ServerConfig) -> Registry {
    Registry::with_card_source(config, pool.clone(), source(pool))
}

/// A recorded two-seat game with the card in both decks: the bots are seated, ready, and have given `answers` answers.
async fn started(registry: &Registry, seed: u64, answers: usize) -> (GameId, Vec<Bot>) {
    let recipe = GameRecipe::normal(seed, [DeckRef::oracle(ids()), DeckRef::oracle(ids())]);
    let names = vec!["Bot 0".to_owned(), "Bot 1".to_owned()];
    let created = registry
        .create_from_recipe(recipe, names, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut bots = vec![
        Bot::join(&handle, 0, seed).await,
        Bot::join(&handle, 1, seed).await,
    ];
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers).await, None);
    (created.id, bots)
}

async fn recipe_of(pool: &PgPool, game: GameId) -> Value {
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    record.recipe
}

#[sqlx::test(migrations = false)]
async fn a_game_pins_the_draft_it_plays_and_the_next_game_after_a_wrong_verdict_plays_by_hand(
    pool: PgPool,
) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_FORGE, true, false).await;
    let config = config(61);
    let registry = registry(&pool, &config);

    let (first, _bots) = started(&registry, 61, 10).await;
    let pinned = recipe_of(&pool, first).await;
    assert_eq!(pinned["reads"], json!({ id_of(CARD): key.read_hash }));

    verdict(&db, &key, Verdict::Wrong, Some(3)).await;
    let (second, _others) = started(&registry, 62, 10).await;

    assert!(
        recipe_of(&pool, second).await.get("reads").is_none(),
        "played by hand: nothing to pin"
    );
    assert_eq!(
        recipe_of(&pool, first).await["reads"],
        pinned["reads"],
        "the first game keeps its pins"
    );
}

#[sqlx::test(migrations = false)]
async fn a_game_resumes_with_its_pinned_cards_whatever_the_verdicts_are_now(pool: PgPool) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_FORGE, true, false).await;
    let config = config(63);
    let first = registry(&pool, &config);
    let (game, _bots) = started(&first, 63, 60).await;
    let before = first.handle(game).unwrap().status().await.unwrap();
    assert!(before.started && before.events > 10);
    drop(first);

    verdict(&db, &key, Verdict::Wrong, Some(4)).await;
    let second = registry(&pool, &config);
    let report = second.resume().await.unwrap();

    assert_eq!((report.resumed, report.failed.len()), (1, 0), "{report:?}");
    let after = second.handle(game).unwrap().status().await.unwrap();
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );
    let recipe: GameRecipe = serde_json::from_value(recipe_of(&pool, game).await).unwrap();
    let catalog = source(&pool).catalog_for(&recipe).await.unwrap();
    assert!(catalog.get(&id_of(CARD)).unwrap().read, "built as it was");
}

#[sqlx::test(migrations = false)]
async fn a_game_whose_pinned_draft_is_gone_fails_at_boot_like_a_lost_card(pool: PgPool) {
    let db = seeded(&pool).await;
    draft(&db, SOURCE_FORGE, true, false).await;
    let config = config(64);
    let first = registry(&pool, &config);
    let (game, _bots) = started(&first, 64, 20).await;
    drop(first);

    sqlx::query("DELETE FROM card_drafts")
        .execute(&pool)
        .await
        .unwrap();
    let report = registry(&pool, &config).resume().await.unwrap();

    assert_eq!(report.failed, [game], "{report:?}");
    let record = GameRecords::new(pool).get(game).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Failed);
}

#[sqlx::test(migrations = false)]
async fn an_ability_left_to_the_table_records_the_servers_wrong_and_the_next_game_plays_by_hand(
    pool: PgPool,
) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_FORGE, true, true).await;
    let config = ServerConfig {
        fixed_seed: None,
        ..config(65)
    };
    let registry = registry(&pool, &config);
    assert_eq!(support(&source(&pool)).await, Support::Forge);

    let wrong = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM card_reads WHERE user_id IS NULL AND verdict = 'wrong'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    // Random bots cast a four-of rarely: play seeds until one does.
    for seed in 0..12 {
        if wrong().await > 0 {
            break;
        }
        let (game, mut bots) = started(&registry, seed, 0).await;
        let handle = registry.handle(game).unwrap();
        for bot in &mut bots {
            bot.rejoin(&handle).await;
        }
        drive(&mut bots, &handle, usize::MAX).await;
    }

    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_events WHERE event::text LIKE '%AbilityLeftToTable%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        left > 0,
        "the log holds the ability the game left to the table"
    );
    assert!(wrong().await > 0, "the bots cast the card and it failed");
    let note: String =
        sqlx::query_scalar("SELECT note FROM card_reads WHERE user_id IS NULL LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!note.is_empty() && note.chars().count() <= 280, "{note}");
    let state = db.reads().state().await.unwrap();
    assert_eq!(state[0].read_hash, key.read_hash);
    assert_eq!(state[0].state, mtg_db::ReadState::Wrong);
    assert_eq!(support(&source(&pool)).await, Support::Manual);
}

// ---- the fix mark ----

fn mark_body(note: Option<&str>) -> String {
    json!({ "note": note }).to_string()
}

#[sqlx::test(migrations = false)]
async fn a_fix_mark_on_a_card_that_runs_on_a_read_records_the_accounts_wrong(pool: PgPool) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_FORGE, true, false).await;
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(&pool)).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let path = format!("/api/fix-marks/{}", id_of(CARD));
    let card = || async {
        net::get(server.addr, &format!("/api/cards/{}", id_of(CARD)))
            .await
            .json()["card"]["support"]
            .clone()
    };
    assert_eq!(card().await, "forge");

    let marked = ann
        .put(server.addr, &path, &mark_body(Some("it draws twice")))
        .await;
    assert_eq!(marked.status, 200, "{}", marked.text());

    assert_eq!(card().await, "manual", "played by hand from the next game");
    let state = db.reads().state().await.unwrap();
    assert_eq!(
        (state[0].read_hash.as_str(), state[0].wrong),
        (key.read_hash.as_str(), 1)
    );
    let note: String = sqlx::query_scalar("SELECT note FROM card_reads WHERE verdict = 'wrong'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(note, "it draws twice");
}

#[sqlx::test(migrations = false)]
async fn a_fix_mark_on_a_card_played_by_hand_records_no_verdict(pool: PgPool) {
    let db = seeded(&pool).await;
    draft(&db, SOURCE_SWEEP, false, false).await;
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(&pool)).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let path = format!("/api/fix-marks/{}", id_of(CARD));

    let marked = ann.put(server.addr, &path, &mark_body(None)).await;

    assert_eq!(marked.status, 200, "{}", marked.text());
    let verdicts: i64 = sqlx::query_scalar("SELECT count(*) FROM card_reads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(verdicts, 0);
}

// ---- seats that chose under different draft states ----

/// What happens to the draft of [`CARD`] between the two seats' choices.
#[derive(Clone, Copy, Debug)]
enum Between {
    /// The first seat chose while the draft was unread (by hand); a player approves it before the second seat chooses.
    Approved,
    /// The first seat chose on an approved draft (pinned); a player finds it wrong before the second seat chooses.
    FoundWrong,
}

/// A modern deck of Forests and four copies of [`CARD`].
fn explorers() -> Value {
    let line = |name: &str, count: u32| json!({"zone": "main", "oracle_id": id_of(name), "printing_id": null, "count": count});
    json!({"name": "Explorers", "format": "freeform", "notes": "",
           "cards": [line("Forest", 56), line(CARD, 4)]})
}

async fn lobby_server(pool: &PgPool) -> TestServer {
    TestServer::start_with_cards(pool.clone(), bot_config(), source(pool)).await
}

async fn store(server: &TestServer, who: &net::Browser) -> String {
    let reply = who
        .post(server.addr, "/api/decks", &explorers().to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

/// Two accounts each play a stored deck holding the card, choosing before and after `between` happens to its draft.
/// The game starts, its recipe builds, and a restarted server resumes it with the very state it had.
async fn seats_choose_under_different_drafts(pool: PgPool, between: Between) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_SWEEP, false, false).await;
    if matches!(between, Between::FoundWrong) {
        verdict(&db, &key, Verdict::Correct, Some(1)).await;
    }
    let mut server = lobby_server(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let ann_deck = store(&server, &ann).await;
    let bo_deck = store(&server, &bo).await;
    let body = json!({"format": "normal", "visibility": "public", "held": 1,
                      "seats": [{"name": "ann", "deck": "mono-green"}, {"deck": "mono-red"}]});
    let game = ann.create_game(server.addr, &body).await.0;
    let chosen = ann
        .put(
            server.addr,
            &format!("/api/games/{game}/seats/0/deck"),
            &json!({ "deck": ann_deck }).to_string(),
        )
        .await;
    assert_eq!(chosen.status, 200, "{}", chosen.text());

    match between {
        Between::Approved => verdict(&db, &key, Verdict::Correct, Some(1)).await,
        Between::FoundWrong => verdict(&db, &key, Verdict::Wrong, Some(2)).await,
    }
    let joined = bo
        .post(
            server.addr,
            &format!("/api/games/{game}/join"),
            &json!({ "deck": bo_deck }).to_string(),
        )
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());

    lobby::start(&server.registry, &game, 2).await;
    let id: GameId = game.parse().unwrap();
    let record = GameRecords::new(pool.clone())
        .get(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        record.recipe["reads"],
        json!({ id_of(CARD): key.read_hash }),
        "one draft for the card in the whole game ({between:?})"
    );
    // Every seat's fingerprint matches the one definition per card.
    let recipe: GameRecipe = serde_json::from_value(record.recipe.clone()).unwrap();
    let catalog = source(&pool).catalog_for(&recipe).await.unwrap();
    recipe.build_with(&catalog).unwrap();

    let handle = server.registry.handle(id).unwrap();
    let before = handle.status().await.unwrap();
    server.shutdown();
    server.finished().await.unwrap();
    drop(handle);
    drop(server);

    let restarted = lobby_server(&pool).await;
    let handle = restarted.registry.handle(id).expect("the game came back");
    let after = handle.status().await.unwrap();
    assert_eq!(after.state_hash, before.state_hash, "{between:?}");
    let record = GameRecords::new(pool).get(id).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Running, "{between:?}");
}

#[sqlx::test(migrations = false)]
async fn a_card_approved_between_two_seats_choices_is_one_definition_for_both_and_the_game_resumes(
    pool: PgPool,
) {
    seats_choose_under_different_drafts(pool, Between::Approved).await;
}

#[sqlx::test(migrations = false)]
async fn a_card_found_wrong_between_two_seats_choices_is_one_definition_for_both_and_the_game_resumes(
    pool: PgPool,
) {
    seats_choose_under_different_drafts(pool, Between::FoundWrong).await;
}

#[sqlx::test(migrations = false)]
async fn a_draft_load_that_failed_is_not_kept_as_current(pool: PgPool) {
    let db = seeded(&pool).await;
    let key = draft(&db, SOURCE_SWEEP, false, false).await;
    verdict(&db, &key, Verdict::Correct, Some(1)).await;
    let source = source(&pool);
    assert_eq!(support(&source).await, Support::Read);

    // The version still reads while the drafts cannot: the library of that moment has no draft.
    let broken = [
        "ALTER TABLE card_drafts RENAME COLUMN rules TO rules_away",
        "ALTER TABLE card_drafts RENAME COLUMN rules_away TO rules",
    ];
    sqlx::query(broken[0]).execute(&pool).await.unwrap();
    verdict(&db, &key, Verdict::Correct, Some(5)).await;
    assert_eq!(support(&source).await, Support::Manual);

    // Nothing else changes, and the next read of the drafts succeeds: the card is a read again.
    sqlx::query(broken[1]).execute(&pool).await.unwrap();
    assert_eq!(
        support(&source).await,
        Support::Read,
        "not cached as current"
    );
}
