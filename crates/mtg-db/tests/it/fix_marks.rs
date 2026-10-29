//! Goal: fix marks are one row per account and card (marking again only replaces the note), each account sees and removes only its own, the ranking counts accounts and ties break the same way every time, the newest note wins, an unknown card cannot be marked, the database refuses a note over 280 characters, and marks go with their card and their account.

use crate::common;

use mtg_db::{DbError, FixMarkRepo, MAX_NOTE_CHARS, RankedMark};
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use uuid::Uuid;

use common::{oracle_of, seeded};

fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The marks of the ranking as `(name, count)`.
fn counts(ranked: &[RankedMark]) -> Vec<(&str, u32)> {
    ranked
        .iter()
        .map(|mark| (mark.name.as_str(), mark.marks))
        .collect()
}

#[sqlx::test]
async fn a_mark_reads_back_with_the_card_name_and_a_utc_time(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    let made = repo
        .mark(user(1), &oracle_of("Lightning Bolt"), Some("  deals 2  "))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(made.name, "Lightning Bolt");
    assert_eq!(made.note.as_deref(), Some("deals 2"));
    assert_eq!(made.created_at.len(), 20, "{}", made.created_at);
    assert!(made.created_at.ends_with('Z'));
    assert_eq!(repo.list(user(1)).await.unwrap(), vec![made]);
}

#[sqlx::test]
async fn marking_again_replaces_the_note_and_keeps_one_row_and_its_time(pool: PgPool) {
    let repo = seeded(pool.clone()).await.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    let first = repo
        .mark(user(1), &bolt, Some("first"))
        .await
        .unwrap()
        .unwrap();
    let second = repo.mark(user(1), &bolt, None).await.unwrap().unwrap();
    assert_eq!(second.note, None);
    assert_eq!(second.created_at, first.created_at);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM card_fix_marks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    let blank = repo
        .mark(user(1), &bolt, Some("   "))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(blank.note, None, "an empty note is none");
}

#[sqlx::test]
async fn an_account_lists_and_removes_only_its_own_marks(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    repo.mark(user(1), &bolt, None).await.unwrap();
    repo.mark(user(2), &bolt, Some("bo")).await.unwrap();
    assert!(!repo.unmark(user(3), &bolt).await.unwrap());
    assert!(repo.unmark(user(1), &bolt).await.unwrap());
    assert!(!repo.unmark(user(1), &bolt).await.unwrap(), "already gone");
    assert_eq!(repo.list(user(1)).await.unwrap(), vec![]);
    let kept = repo.list(user(2)).await.unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].note.as_deref(), Some("bo"));
}

#[sqlx::test]
async fn a_list_is_the_newest_mark_first(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    for name in ["Lightning Bolt", "Sol Ring", "Mountain"] {
        repo.mark(user(1), &oracle_of(name), None).await.unwrap();
    }
    let names: Vec<String> = repo
        .list(user(1))
        .await
        .unwrap()
        .into_iter()
        .map(|mark| mark.name)
        .collect();
    assert_eq!(names, ["Mountain", "Sol Ring", "Lightning Bolt"]);
}

#[sqlx::test]
async fn a_card_the_database_lacks_cannot_be_marked(pool: PgPool) {
    let repo = seeded(pool.clone()).await.fix_marks();
    for id in ["00000000-0000-4000-8000-000000000000", "not-an-id", ""] {
        let made = repo.mark(user(1), &OracleId(id.to_owned()), None).await;
        assert_eq!(made.unwrap(), None, "{id:?}");
        assert!(
            !repo
                .unmark(user(1), &OracleId(id.to_owned()))
                .await
                .unwrap()
        );
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM card_fix_marks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test]
async fn the_ranking_counts_accounts_and_breaks_ties_by_the_newest_mark_then_name(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    let ring = oracle_of("Sol Ring");
    let mountain = oracle_of("Mountain");
    // Bolt has three marks, Sol Ring and Mountain two each; Mountain's newest mark is the later one.
    for who in 1..=3 {
        repo.mark(user(who), &bolt, None).await.unwrap();
    }
    repo.mark(user(1), &ring, None).await.unwrap();
    repo.mark(user(2), &ring, None).await.unwrap();
    repo.mark(user(1), &mountain, None).await.unwrap();
    repo.mark(user(2), &mountain, None).await.unwrap();
    // The same account marking again does not count twice.
    repo.mark(user(1), &mountain, Some("again")).await.unwrap();

    let all = repo.ranked(None).await.unwrap();
    assert_eq!(
        counts(&all),
        [("Lightning Bolt", 3), ("Mountain", 2), ("Sol Ring", 2)]
    );
    let top = repo.ranked(Some(2)).await.unwrap();
    assert_eq!(counts(&top), [("Lightning Bolt", 3), ("Mountain", 2)]);
    assert_eq!(repo.ranked(Some(0)).await.unwrap(), vec![]);
}

#[sqlx::test]
async fn the_newest_note_of_a_card_is_the_newest_mark_that_has_one(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    repo.mark(user(1), &bolt, Some("old")).await.unwrap();
    repo.mark(user(2), &bolt, Some("new")).await.unwrap();
    repo.mark(user(3), &bolt, None).await.unwrap();
    let ranked = repo.ranked(None).await.unwrap();
    assert_eq!(ranked[0].marks, 3);
    assert_eq!(ranked[0].newest_note.as_deref(), Some("new"));
    repo.unmark(user(2), &bolt).await.unwrap();
    let ranked = repo.ranked(None).await.unwrap();
    assert_eq!(ranked[0].newest_note.as_deref(), Some("old"));
    repo.mark(user(1), &bolt, None).await.unwrap();
    let ranked = repo.ranked(None).await.unwrap();
    assert_eq!(ranked[0].newest_note, None, "no mark has a note any more");
}

#[sqlx::test]
async fn the_database_refuses_a_note_over_the_limit_and_takes_one_at_it(pool: PgPool) {
    let repo = seeded(pool).await.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    let at_limit = "é".repeat(MAX_NOTE_CHARS);
    let made = repo
        .mark(user(1), &bolt, Some(&at_limit))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(made.note.as_deref(), Some(at_limit.as_str()));
    let over = "x".repeat(MAX_NOTE_CHARS + 1);
    let refused = repo.mark(user(1), &bolt, Some(&over)).await;
    assert!(matches!(refused, Err(DbError::Query(_))), "{refused:?}");
}

#[sqlx::test]
async fn deleting_an_account_removes_its_marks_only(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let repo = database.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    repo.mark(user(1), &bolt, None).await.unwrap();
    repo.mark(user(1), &oracle_of("Sol Ring"), None)
        .await
        .unwrap();
    repo.mark(user(2), &bolt, None).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let gone = FixMarkRepo::delete_all_of(&mut tx, user(1)).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(gone, 2);
    assert_eq!(repo.list(user(1)).await.unwrap(), vec![]);
    assert_eq!(repo.list(user(2)).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn deleting_a_card_removes_its_marks(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let repo = database.fix_marks();
    let bolt = oracle_of("Lightning Bolt");
    repo.mark(user(1), &bolt, None).await.unwrap();
    sqlx::query("DELETE FROM cards WHERE oracle_id = $1::uuid")
        .bind(bolt.as_str())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(repo.list(user(1)).await.unwrap(), vec![]);
}
