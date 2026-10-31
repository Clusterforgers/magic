//! Goal: drafts are stored one current row per card and read back as written, a new hash retires the old draft and a returning hash revives it, retiring never deletes, the database refuses malformed rows and a second current draft, drafts go with their card, and the version moves with every change a library could see and with nothing else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_db::{Database, DbError, DraftKey, NewDraft, StoredDraft, Verdict};
use mtg_import::ids::OracleId;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use common::drafts::{draft, hash, key};
use common::{oracle_of, seeded};

/// The hash numbers of the current drafts of `card`.
async fn current_of(database: &Database, card: &OracleId) -> Vec<String> {
    database
        .drafts()
        .current()
        .await
        .unwrap()
        .into_iter()
        .filter(|stored| &stored.oracle_id == card)
        .map(|stored| stored.read_hash)
        .collect()
}

async fn stored_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM card_drafts")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn sqlstate(error: &sqlx::Error) -> String {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_default()
}

#[sqlx::test]
async fn a_draft_reads_back_exactly_as_it_was_written(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    let row = NewDraft {
        rules: json!({"b": [1, 2], "a": {"x": null}}),
        ..draft(&bolt, 1)
    };
    assert_eq!(
        database
            .drafts()
            .insert(std::slice::from_ref(&row))
            .await
            .unwrap(),
        1
    );
    let expected = StoredDraft {
        oracle_id: bolt,
        read_hash: hash(1),
        rules: row.rules,
        retired: false,
    };
    assert_eq!(
        database.drafts().current().await.unwrap(),
        std::slice::from_ref(&expected)
    );
    assert_eq!(
        database.drafts().get(&[expected.key()]).await.unwrap(),
        [expected]
    );
}

#[sqlx::test]
async fn inserting_a_stored_current_draft_again_changes_nothing(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    let repo = database.drafts();
    repo.insert(&[draft(&bolt, 1)]).await.unwrap();
    let again = NewDraft {
        rules: json!({"changed": true}),
        ..draft(&bolt, 1)
    };
    assert_eq!(repo.insert(&[again]).await.unwrap(), 0);
    assert_eq!(stored_rows(&pool).await, 1);
    assert_eq!(
        repo.current().await.unwrap()[0].rules,
        draft(&bolt, 1).rules,
        "the hash covers the rules, so a stored draft is not rewritten"
    );
    assert_eq!(repo.insert(&[]).await.unwrap(), 0);
}

#[sqlx::test]
async fn a_new_hash_retires_the_cards_old_draft_and_keeps_its_row(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    let repo = database.drafts();
    repo.insert(&[draft(&bolt, 1), draft(&ring, 2)])
        .await
        .unwrap();
    assert_eq!(repo.insert(&[draft(&bolt, 3)]).await.unwrap(), 1);

    assert_eq!(current_of(&database, &bolt).await, [hash(3)]);
    assert_eq!(
        current_of(&database, &ring).await,
        [hash(2)],
        "other cards are not touched"
    );
    assert_eq!(stored_rows(&pool).await, 3);
    let old = repo.get(&[key(&bolt, 1)]).await.unwrap();
    assert_eq!(old.len(), 1);
    assert!(old[0].retired);
    assert_eq!(old[0].rules, draft(&bolt, 1).rules);
}

#[sqlx::test]
async fn a_retired_draft_the_build_holds_again_becomes_current_and_retires_the_other(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    let repo = database.drafts();
    repo.insert(&[draft(&bolt, 1)]).await.unwrap();
    repo.insert(&[draft(&bolt, 2)]).await.unwrap();
    assert_eq!(repo.insert(&[draft(&bolt, 1)]).await.unwrap(), 1);
    assert_eq!(current_of(&database, &bolt).await, [hash(1)]);
    let both = repo.get(&[key(&bolt, 1), key(&bolt, 2)]).await.unwrap();
    assert_eq!(
        both.iter().map(|d| d.retired).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(stored_rows(&pool).await, 2, "no row was added");
}

#[sqlx::test]
async fn a_card_listed_twice_in_one_call_keeps_the_last_draft(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    database
        .drafts()
        .insert(&[draft(&bolt, 1), draft(&bolt, 2)])
        .await
        .unwrap();
    assert_eq!(current_of(&database, &bolt).await, [hash(2)]);
}

#[sqlx::test]
async fn retiring_marks_the_named_current_drafts_and_deletes_nothing(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring, mountain) = (
        oracle_of("Lightning Bolt"),
        oracle_of("Sol Ring"),
        oracle_of("Mountain"),
    );
    let repo = database.drafts();
    repo.insert(&[draft(&bolt, 1), draft(&ring, 2), draft(&mountain, 3)])
        .await
        .unwrap();
    let unknown = key(&oracle_of("Island"), 9);
    let wrong_hash = key(&ring, 8);
    let no_uuid = DraftKey {
        oracle_id: OracleId("not-an-id".into()),
        read_hash: hash(1),
    };
    assert_eq!(
        repo.retire(&[key(&bolt, 1), unknown, wrong_hash, no_uuid])
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo.retire(&[key(&bolt, 1)]).await.unwrap(),
        0,
        "already retired"
    );
    assert_eq!(repo.retire(&[]).await.unwrap(), 0);
    let current: Vec<_> = repo
        .current()
        .await
        .unwrap()
        .into_iter()
        .map(|d| d.oracle_id)
        .collect();
    assert!(!current.contains(&bolt));
    assert_eq!(current.len(), 2);
    assert_eq!(stored_rows(&pool).await, 3);
}

#[sqlx::test]
async fn get_returns_retired_and_current_drafts_and_leaves_out_unknown_keys(pool: PgPool) {
    let database = seeded(pool).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    let repo = database.drafts();
    repo.insert(&[draft(&bolt, 1), draft(&ring, 5)])
        .await
        .unwrap();
    repo.insert(&[draft(&bolt, 2)]).await.unwrap();
    let no_uuid = DraftKey {
        oracle_id: OracleId("x".into()),
        read_hash: hash(1),
    };
    let got = repo
        .get(&[
            key(&ring, 5),
            key(&bolt, 1),
            key(&bolt, 2),
            key(&bolt, 7),
            no_uuid,
        ])
        .await
        .unwrap();
    let seen: Vec<(String, bool)> = got
        .iter()
        .map(|d| (d.read_hash.clone(), d.retired))
        .collect();
    let mut expected = vec![(hash(1), true), (hash(2), false), (hash(5), false)];
    let mut seen_sorted = seen.clone();
    seen_sorted.sort();
    expected.sort();
    assert_eq!(seen_sorted, expected, "{seen:?}");
    assert_eq!(repo.get(&[]).await.unwrap(), vec![]);
}

#[sqlx::test]
async fn a_batch_with_a_bad_draft_writes_nothing(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    let repo = database.drafts();
    let no_uuid = NewDraft {
        oracle_id: OracleId("not-an-id".into()),
        ..draft(&bolt, 2)
    };
    let refused = repo.insert(&[draft(&bolt, 1), no_uuid]).await;
    assert!(
        matches!(refused, Err(DbError::Corrupt { .. })),
        "{refused:?}"
    );
    let unknown_card = NewDraft {
        oracle_id: OracleId("00000000-0000-4000-8000-000000000000".into()),
        ..draft(&bolt, 3)
    };
    let refused = repo.insert(&[draft(&bolt, 1), unknown_card]).await;
    assert!(matches!(refused, Err(DbError::Query(_))), "{refused:?}");
    assert_eq!(stored_rows(&pool).await, 0);
}

#[sqlx::test]
async fn the_database_refuses_malformed_rows_and_a_second_current_draft(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    let cases: [(&str, String, &str); 6] = [
        ("upper-case hash", hash(255).to_uppercase(), "23514"),
        ("short hash", "abc".into(), "23514"),
        ("empty hash", String::new(), "23514"),
        ("hash with a non-hex character", "g".repeat(32), "23514"),
        ("second current draft", hash(2), "23505"),
        ("same key twice", hash(1), "23505"),
    ];
    for (name, read_hash, state) in cases {
        let refused = sqlx::query(
            "INSERT INTO card_drafts (oracle_id, read_hash, rules) VALUES ($1::uuid, $2, '{}')",
        )
        .bind(bolt.as_str())
        .bind(read_hash)
        .execute(&pool)
        .await
        .unwrap_err();
        assert_eq!(sqlstate(&refused), state, "{name}");
    }
    for (name, rules) in [("array", "[]"), ("scalar", "1"), ("null", "null")] {
        let refused = sqlx::query(
            "INSERT INTO card_drafts (oracle_id, read_hash, rules, retired)
             VALUES ($1::uuid, $2, $3::jsonb, true)",
        )
        .bind(bolt.as_str())
        .bind(hash(9))
        .bind(rules)
        .execute(&pool)
        .await
        .unwrap_err();
        assert_eq!(sqlstate(&refused), "23514", "rules that are a(n) {name}");
    }
    let retired_twice = sqlx::query(
        "INSERT INTO card_drafts (oracle_id, read_hash, rules, retired)
         VALUES ($1::uuid, $2, '{}', true), ($1::uuid, $3, '{}', true)",
    )
    .bind(bolt.as_str())
    .bind(hash(7))
    .bind(hash(8))
    .execute(&pool)
    .await;
    assert!(retired_twice.is_ok(), "any number of retired rows per card");
}

#[sqlx::test]
async fn deleting_a_card_removes_its_drafts_and_their_verdicts(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    database
        .drafts()
        .insert(&[draft(&bolt, 1), draft(&ring, 2)])
        .await
        .unwrap();
    for card in [&bolt, &ring] {
        let n = if card == &bolt { 1 } else { 2 };
        database
            .reads()
            .append(
                Some(Uuid::from_u128(1)),
                &key(card, n),
                Verdict::Correct,
                None,
            )
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM cards WHERE oracle_id = $1::uuid")
        .bind(bolt.as_str())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(stored_rows(&pool).await, 1);
    let reads = database.reads().list(false).await.unwrap();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].oracle_id, ring);
}

#[sqlx::test]
async fn the_version_moves_with_the_current_drafts_and_the_verdicts_and_with_nothing_else(
    pool: PgPool,
) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    let repo = database.drafts();
    let user = Uuid::from_u128(1);
    let empty = repo.version().await.unwrap();
    assert_eq!(repo.version().await.unwrap(), empty);

    repo.insert(&[draft(&bolt, 1)]).await.unwrap();
    let one = repo.version().await.unwrap();
    assert_ne!(one, empty, "a draft was added");
    repo.insert(&[draft(&bolt, 1)]).await.unwrap();
    repo.current().await.unwrap();
    database.reads().state().await.unwrap();
    assert_eq!(
        repo.version().await.unwrap(),
        one,
        "reads of the data and a no-op insert change nothing"
    );

    repo.insert(&[draft(&ring, 2)]).await.unwrap();
    let two = repo.version().await.unwrap();
    assert_ne!(two, one, "a card gained a draft");

    repo.insert(&[draft(&bolt, 3)]).await.unwrap();
    let replaced = repo.version().await.unwrap();
    assert_ne!(replaced, two, "a draft was replaced");
    repo.insert(&[draft(&bolt, 1)]).await.unwrap();
    let back = repo.version().await.unwrap();
    assert_ne!(back, replaced, "a retired draft came back");
    assert_eq!(
        back, two,
        "the same current drafts and verdicts are the same version"
    );

    database
        .reads()
        .append(Some(user), &key(&bolt, 1), Verdict::Correct, None)
        .await
        .unwrap();
    let judged = repo.version().await.unwrap();
    assert_ne!(judged, back, "a verdict was added");
    database
        .reads()
        .append(Some(user), &key(&bolt, 1), Verdict::Correct, None)
        .await
        .unwrap();
    let again = repo.version().await.unwrap();
    assert_ne!(again, judged, "a repeated verdict is another row");

    let mut tx = pool.begin().await.unwrap();
    mtg_db::ReadRepo::delete_all_of(&mut tx, user)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let deleted = repo.version().await.unwrap();
    assert_ne!(deleted, again, "an account's verdicts were deleted");

    repo.retire(&[key(&ring, 2)]).await.unwrap();
    assert_ne!(
        repo.version().await.unwrap(),
        deleted,
        "a draft was retired"
    );
}
