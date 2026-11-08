//! Goal: a trusted Forge draft whose clause reads back as other text than the card plays by hand, and the server names the reason `readback:Differs` in the card's support and on its card page; a draft that reads back plays automatically.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::support::{READBACK_DIFFERS, Support, read_hash};
use mtg_db::{Database, NewDraft, SOURCE_FORGE};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate, printed_card};
use net::{TestServer, bot_config, get};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A sorcery the corpus writes by hand; the test keeps it out of the rules files and reads it as a draft instead.
const CARD: &str = "Explore";

/// The draw effect of the draft, the support the card then has and whether the reason is named.
const CASES: &[(&str, Support, bool)] = &[
    (r#"{"Draw": {}}"#, Support::Forge, false),
    (r#"{"Draw": {"count": 2}}"#, Support::Manual, true),
    (r#"{"Draw": {"who": "EachPlayer"}}"#, Support::Manual, true),
];

fn id_of(name: &str) -> String {
    crate::common::oracle_id_of(name)
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
    let scryfall = mtg_testkit::corpus::scryfall_array().unwrap();
    db.importer()
        .import_cards(cards(scryfall.as_bytes()))
        .await
        .unwrap();
    db
}

/// Stores a trusted Forge draft of [`CARD`] whose spell draws as `draw` says.
async fn draft(db: &Database, draw: Value) {
    let corpus = builtin::library().unwrap();
    let mut rules = serde_json::to_value(corpus.card_by_name(CARD).unwrap()).unwrap();
    rules["faces"][0]["abilities"][0]["Spell"]["effects"][1] = draw;
    let card: Card = serde_json::from_value(rules.clone()).unwrap();
    let stored = db
        .cards()
        .get(&OracleId(id_of(CARD)))
        .await
        .unwrap()
        .unwrap();
    db.drafts()
        .insert(&[NewDraft {
            oracle_id: OracleId(id_of(CARD)),
            read_hash: read_hash(&printed_card(&stored), &card),
            rules,
            source: SOURCE_FORGE.to_owned(),
            oracle_matches: true,
            differs: false,
            proof: None,
        }])
        .await
        .unwrap();
}

fn names_why(text: &str) -> bool {
    text.starts_with(READBACK_DIFFERS)
}

#[sqlx::test(migrations = false)]
async fn a_draft_that_reads_back_as_other_text_plays_by_hand_and_names_why(pool: PgPool) {
    let db = seeded(&pool).await;
    let source = CardSource::new(pool.clone()).with_rules(without_card());
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source.clone()).await;
    for &(draw, expected, named) in CASES {
        draft(&db, serde_json::from_str(draw).unwrap()).await;
        let assembled = source.assemble(&[id_of(CARD)]).await.unwrap();
        let record = &assembled[&id_of(CARD)].support;
        assert_eq!(record.support, expected, "{draw}: {record:?}");
        let found = record
            .clauses
            .iter()
            .any(|clause| names_why(&clause.reason));
        assert_eq!(found, named, "{draw}: {record:?}");

        let page = get(server.addr, &format!("/api/cards/{}", id_of(CARD)))
            .await
            .json();
        let shown = page["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|reason| reason["text"].as_str())
            .any(names_why);
        assert_eq!(shown, named, "{draw}: {page}");
        let state = json!(format!("{expected:?}").to_lowercase());
        assert_eq!(page["card"]["support"], state, "{draw}");
        sqlx::query("DELETE FROM card_drafts")
            .execute(&pool)
            .await
            .unwrap();
    }
}
