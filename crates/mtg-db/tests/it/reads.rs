//! Goal: verdicts are appended and never changed, an account's newest verdict on a hash is the one that counts, one wrong wins whatever the accounts and the order, the server's wrong has no account and stays, a verdict on another hash counts for nothing, the queue is what an account has left to read, and deleting an account removes its verdicts but not the drafts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_db::{DbError, ReadRepo, ReadState, Verdict};
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use uuid::Uuid;

use common::drafts::{draft, key};
use common::{oracle_of, seeded};

const CORRECT: Verdict = Verdict::Correct;
const WRONG: Verdict = Verdict::Wrong;

fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Verdicts by account (none is the server) in the order they are given.
type Sequence = &'static [(Option<u128>, Verdict)];

/// An append to try: what it is called, who gives it, the verdict, the note, and whether the database takes it.
type Attempt<'a> = (&'a str, Option<Uuid>, Verdict, Option<&'a str>, bool);

/// Appends the verdicts on the draft of `card` with hash number `n`; a `wrong` carries a note.
async fn give(database: &mtg_db::Database, card: &OracleId, n: u32, verdicts: Sequence) {
    for (who, verdict) in verdicts {
        let note = (*verdict == WRONG).then_some("it does not work");
        let made = database
            .reads()
            .append(who.map(user), &key(card, n), *verdict, note)
            .await
            .unwrap();
        assert!(made.is_some(), "the draft exists");
    }
}

async fn state_of(database: &mtg_db::Database, card: &OracleId) -> (ReadState, u32, u32) {
    let all = database
        .reads()
        .state_of(std::slice::from_ref(card))
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
    (all[0].state, all[0].correct, all[0].wrong)
}

/// Every verdict sequence with the state, the accounts that judged it correct and the wrong ones it must give.
const STATES: &[(&str, Sequence, ReadState, u32, u32)] = &[
    ("no verdict", &[], ReadState::Pending, 0, 0),
    (
        "one correct",
        &[(Some(1), CORRECT)],
        ReadState::Approved,
        1,
        0,
    ),
    (
        "two accounts correct",
        &[(Some(1), CORRECT), (Some(2), CORRECT)],
        ReadState::Approved,
        2,
        0,
    ),
    ("one wrong", &[(Some(1), WRONG)], ReadState::Wrong, 0, 1),
    (
        "wrong after correct",
        &[(Some(1), CORRECT), (Some(2), WRONG)],
        ReadState::Wrong,
        1,
        1,
    ),
    (
        "wrong before correct",
        &[(Some(2), WRONG), (Some(1), CORRECT)],
        ReadState::Wrong,
        1,
        1,
    ),
    (
        "an account's later correct replaces its wrong",
        &[(Some(1), WRONG), (Some(1), CORRECT)],
        ReadState::Approved,
        1,
        0,
    ),
    (
        "an account's later wrong replaces its correct",
        &[(Some(1), CORRECT), (Some(1), WRONG)],
        ReadState::Wrong,
        0,
        1,
    ),
    (
        "the same verdict again counts once",
        &[(Some(1), CORRECT), (Some(1), CORRECT), (Some(1), CORRECT)],
        ReadState::Approved,
        1,
        0,
    ),
    (
        "another account's correct does not undo a wrong",
        &[(Some(1), WRONG), (Some(2), CORRECT), (Some(2), CORRECT)],
        ReadState::Wrong,
        1,
        1,
    ),
    (
        "a wrong undone by its account while another still holds one",
        &[(Some(1), WRONG), (Some(2), WRONG), (Some(1), CORRECT)],
        ReadState::Wrong,
        1,
        1,
    ),
    (
        "every wrong undone",
        &[
            (Some(1), WRONG),
            (Some(2), WRONG),
            (Some(1), CORRECT),
            (Some(2), CORRECT),
        ],
        ReadState::Approved,
        2,
        0,
    ),
    (
        "the server's wrong",
        &[(None, WRONG)],
        ReadState::Wrong,
        0,
        1,
    ),
    (
        "the server's wrong beats accounts' correct",
        &[(Some(1), CORRECT), (None, WRONG), (Some(2), CORRECT)],
        ReadState::Wrong,
        2,
        1,
    ),
    (
        "no account replaces the server's wrong",
        &[(None, WRONG), (Some(1), CORRECT), (Some(1), CORRECT)],
        ReadState::Wrong,
        1,
        1,
    ),
    (
        "each server wrong counts",
        &[(None, WRONG), (None, WRONG)],
        ReadState::Wrong,
        0,
        2,
    ),
];

#[sqlx::test]
async fn the_read_state_follows_the_counting_verdicts(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    for (name, verdicts, state, correct, wrong) in STATES {
        sqlx::query("DELETE FROM card_reads")
            .execute(&pool)
            .await
            .unwrap();
        give(&database, &bolt, 1, verdicts).await;
        assert_eq!(
            state_of(&database, &bolt).await,
            (*state, *correct, *wrong),
            "{name}"
        );
    }
}

#[sqlx::test]
async fn the_order_of_different_accounts_never_changes_the_state(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    let verdicts = [(1, CORRECT), (2, WRONG), (3, CORRECT)];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        sqlx::query("DELETE FROM card_reads")
            .execute(&pool)
            .await
            .unwrap();
        for at in order {
            let (who, verdict) = verdicts[at];
            let note = (verdict == WRONG).then_some("no");
            database
                .reads()
                .append(Some(user(who)), &key(&bolt, 1), verdict, note)
                .await
                .unwrap();
        }
        assert_eq!(
            state_of(&database, &bolt).await,
            (ReadState::Wrong, 2, 1),
            "{order:?}"
        );
    }
}

#[sqlx::test]
async fn state_lists_each_current_draft_with_its_own_verdicts(pool: PgPool) {
    let database = seeded(pool).await;
    let (bolt, ring, mountain) = (
        oracle_of("Lightning Bolt"),
        oracle_of("Sol Ring"),
        oracle_of("Mountain"),
    );
    database
        .drafts()
        .insert(&[draft(&bolt, 1), draft(&ring, 2), draft(&mountain, 3)])
        .await
        .unwrap();
    give(&database, &bolt, 1, &[(Some(1), CORRECT)]).await;
    give(&database, &ring, 2, &[(Some(1), WRONG)]).await;
    database
        .drafts()
        .retire(&[key(&mountain, 3)])
        .await
        .unwrap();

    let all = database.reads().state().await.unwrap();
    let seen: Vec<_> = all
        .iter()
        .map(|read| (read.oracle_id.clone(), read.state, read.read_hash.clone()))
        .collect();
    let mut expected = vec![
        (bolt.clone(), ReadState::Approved, common::drafts::hash(1)),
        (ring.clone(), ReadState::Wrong, common::drafts::hash(2)),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen, expected,
        "by oracle id, and a retired draft has no state"
    );

    let only_ring = database
        .reads()
        .state_of(std::slice::from_ref(&ring))
        .await
        .unwrap();
    assert_eq!(only_ring.len(), 1);
    assert_eq!(only_ring[0].oracle_id, ring);
    let none = [
        OracleId("not-an-id".into()),
        OracleId("00000000-0000-4000-8000-000000000000".into()),
        mountain,
    ];
    assert_eq!(database.reads().state_of(&none).await.unwrap(), vec![]);
    assert_eq!(database.reads().state_of(&[]).await.unwrap(), vec![]);
}

#[sqlx::test]
async fn a_verdict_on_another_hash_counts_for_nothing_and_is_kept(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    let drafts = database.drafts();
    drafts.insert(&[draft(&bolt, 1)]).await.unwrap();
    give(&database, &bolt, 1, &[(Some(1), CORRECT), (Some(2), WRONG)]).await;
    assert_eq!(state_of(&database, &bolt).await.0, ReadState::Wrong);

    drafts.insert(&[draft(&bolt, 2)]).await.unwrap();
    assert_eq!(
        state_of(&database, &bolt).await,
        (ReadState::Pending, 0, 0),
        "read again"
    );
    give(&database, &bolt, 2, &[(Some(1), CORRECT)]).await;
    assert_eq!(
        state_of(&database, &bolt).await,
        (ReadState::Approved, 1, 0)
    );

    drafts.insert(&[draft(&bolt, 1)]).await.unwrap();
    assert_eq!(
        state_of(&database, &bolt).await,
        (ReadState::Wrong, 1, 1),
        "the verdicts on the first hash were kept"
    );
    assert_eq!(database.reads().list(false).await.unwrap().len(), 3);
}

#[sqlx::test]
async fn a_verdict_reads_back_with_the_card_name_a_trimmed_note_and_a_time(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    let before = chrono::Utc::now();
    let made = database
        .reads()
        .append(Some(user(7)), &key(&bolt, 1), WRONG, Some("  deals 2  "))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(made.name, "Lightning Bolt");
    assert_eq!(made.oracle_id, bolt);
    assert_eq!(made.read_hash, common::drafts::hash(1));
    assert_eq!((made.user_id, made.verdict), (Some(user(7)), WRONG));
    assert_eq!(made.note.as_deref(), Some("deals 2"));
    assert!(made.created_at >= before - chrono::TimeDelta::seconds(5));
    assert_eq!(
        database.reads().list(false).await.unwrap(),
        std::slice::from_ref(&made)
    );

    let correct = database
        .reads()
        .append(Some(user(7)), &key(&bolt, 1), CORRECT, Some("   "))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(correct.note, None, "an empty note is none");
    assert!(correct.id > made.id);
}

#[sqlx::test]
async fn a_verdict_needs_a_draft_but_not_a_current_one(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    let reads = database.reads();
    let no_uuid = common::drafts::key(&OracleId("not-an-id".into()), 1);
    for missing in [key(&bolt, 2), key(&ring, 1), no_uuid] {
        assert_eq!(
            reads
                .append(Some(user(1)), &missing, CORRECT, None)
                .await
                .unwrap(),
            None,
            "{missing:?}"
        );
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM card_reads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    database.drafts().insert(&[draft(&bolt, 2)]).await.unwrap();
    let late = reads
        .append(None, &key(&bolt, 1), WRONG, Some("failed in a game"))
        .await
        .unwrap();
    assert!(
        late.is_some(),
        "a game can still be playing the retired draft"
    );
    assert_eq!(state_of(&database, &bolt).await, (ReadState::Pending, 0, 0));
}

#[sqlx::test]
async fn the_database_refuses_a_wrong_without_a_note_a_server_correct_and_a_long_note(
    pool: PgPool,
) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    let reads = database.reads();
    let long = "é".repeat(280);
    let too_long = "x".repeat(281);
    let cases: [Attempt; 9] = [
        ("wrong with no note", Some(user(1)), WRONG, None, false),
        (
            "wrong with a blank note",
            Some(user(1)),
            WRONG,
            Some(" \n "),
            false,
        ),
        ("server wrong with no note", None, WRONG, None, false),
        ("server correct", None, CORRECT, Some("fine"), false),
        (
            "note over the limit",
            Some(user(1)),
            WRONG,
            Some(too_long.as_str()),
            false,
        ),
        (
            "note at the limit",
            Some(user(1)),
            WRONG,
            Some(long.as_str()),
            true,
        ),
        ("correct with no note", Some(user(1)), CORRECT, None, true),
        (
            "correct with a note",
            Some(user(1)),
            CORRECT,
            Some("matches"),
            true,
        ),
        (
            "server wrong with a note",
            None,
            WRONG,
            Some("failed"),
            true,
        ),
    ];
    for (name, by, verdict, note, accepted) in cases {
        let result = reads.append(by, &key(&bolt, 1), verdict, note).await;
        match (accepted, result) {
            (true, Ok(Some(_))) | (false, Err(DbError::Query(_))) => {}
            (_, other) => panic!("{name}: {other:?}"),
        }
    }
}

#[sqlx::test]
async fn verdicts_are_append_only_a_later_one_never_changes_an_earlier_row(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    give(&database, &bolt, 1, &[(Some(1), WRONG)]).await;
    let first = database.reads().list(false).await.unwrap();
    give(&database, &bolt, 1, &[(Some(1), CORRECT), (Some(1), WRONG)]).await;

    let all = database.reads().list(false).await.unwrap();
    assert_eq!(all.len(), 3, "each verdict is a row");
    assert_eq!(all[0], first[0], "the first row is as it was");
    assert!(all.windows(2).all(|pair| pair[0].id < pair[1].id));

    // The database refuses to change a row, whoever asks.
    for update in [
        "UPDATE card_reads SET verdict = 'correct'",
        "UPDATE card_reads SET note = 'edited'",
        "UPDATE card_reads SET user_id = NULL",
        "UPDATE card_reads SET created_at = now()",
    ] {
        let refused = sqlx::query(update).execute(&pool).await.unwrap_err();
        assert_eq!(
            refused
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref(),
            Some("23000"),
            "{update}"
        );
    }
    assert_eq!(database.reads().list(false).await.unwrap(), all);
}

#[sqlx::test]
async fn retiring_and_replacing_drafts_never_removes_a_verdict(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    judge_three_drafts(&database, &bolt).await;
    database.drafts().retire(&[key(&bolt, 3)]).await.unwrap();
    assert_eq!(database.reads().list(false).await.unwrap().len(), 3);
}

async fn judge_three_drafts(database: &mtg_db::Database, bolt: &OracleId) {
    for n in 1..=3 {
        database.drafts().insert(&[draft(bolt, n)]).await.unwrap();
        give(database, bolt, n, &[(Some(1), CORRECT)]).await;
    }
}

#[sqlx::test]
async fn deleting_an_account_removes_its_verdicts_only_and_keeps_the_drafts(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    database
        .drafts()
        .insert(&[draft(&bolt, 1), draft(&ring, 2)])
        .await
        .unwrap();
    give(
        &database,
        &bolt,
        1,
        &[(Some(1), CORRECT), (Some(2), CORRECT)],
    )
    .await;
    give(
        &database,
        &ring,
        2,
        &[(Some(1), CORRECT), (Some(1), CORRECT), (None, WRONG)],
    )
    .await;
    let version = database.drafts().version().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let gone = ReadRepo::delete_all_of(&mut tx, user(1)).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(gone, 3, "every row of the account, superseded ones too");
    assert_ne!(database.drafts().version().await.unwrap(), version);
    assert_eq!(
        database.drafts().current().await.unwrap().len(),
        2,
        "the drafts stay"
    );
    assert_eq!(
        state_of(&database, &bolt).await,
        (ReadState::Approved, 1, 0),
        "another account still approves it"
    );
    assert_eq!(
        state_of(&database, &ring).await,
        (ReadState::Wrong, 0, 1),
        "the server's verdict is not an account's"
    );
    let left = database.reads().list(false).await.unwrap();
    assert_eq!(left.len(), 2);
    assert!(left.iter().all(|read| read.user_id != Some(user(1))));

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(ReadRepo::delete_all_of(&mut tx, user(1)).await.unwrap(), 0);
    assert_eq!(ReadRepo::delete_all_of(&mut tx, user(99)).await.unwrap(), 0);
    tx.commit().await.unwrap();
}

#[sqlx::test]
async fn a_card_only_the_deleted_account_approved_is_pending_again(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    give(&database, &bolt, 1, &[(Some(1), CORRECT)]).await;
    assert_eq!(state_of(&database, &bolt).await.0, ReadState::Approved);
    let mut tx = pool.begin().await.unwrap();
    ReadRepo::delete_all_of(&mut tx, user(1)).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(state_of(&database, &bolt).await, (ReadState::Pending, 0, 0));
}

#[sqlx::test]
async fn a_card_only_the_deleted_account_called_wrong_loses_that_verdict(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    give(&database, &bolt, 1, &[(Some(2), CORRECT), (Some(1), WRONG)]).await;
    let mut tx = pool.begin().await.unwrap();
    ReadRepo::delete_all_of(&mut tx, user(1)).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        state_of(&database, &bolt).await,
        (ReadState::Approved, 1, 0)
    );
}

#[sqlx::test]
async fn deleting_an_account_before_the_read_tables_exist_deletes_nothing(pool: PgPool) {
    sqlx::query("DROP TABLE card_reads")
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(ReadRepo::delete_all_of(&mut tx, user(1)).await.unwrap(), 0);
    tx.commit().await.unwrap();
}

#[sqlx::test]
async fn the_account_verdict_prefers_the_current_hash_and_falls_back_to_the_newest(pool: PgPool) {
    let database = seeded(pool).await;
    let (bolt, ring, mountain) = (
        oracle_of("Lightning Bolt"),
        oracle_of("Sol Ring"),
        oracle_of("Mountain"),
    );
    let drafts = database.drafts();
    drafts
        .insert(&[draft(&bolt, 1), draft(&ring, 5), draft(&mountain, 9)])
        .await
        .unwrap();
    give(&database, &bolt, 1, &[(Some(1), CORRECT), (Some(2), WRONG)]).await;
    drafts.insert(&[draft(&bolt, 2)]).await.unwrap();
    give(&database, &ring, 5, &[(Some(1), WRONG), (Some(1), CORRECT)]).await;
    give(&database, &bolt, 2, &[(Some(3), CORRECT)]).await;

    let keys = [key(&bolt, 2), key(&ring, 5), key(&mountain, 9)];
    let mine = database.reads().of_account(user(1), &keys).await.unwrap();
    let seen: Vec<_> = mine
        .iter()
        .map(|read| {
            (
                read.oracle_id.clone(),
                read.read_hash.clone(),
                read.verdict,
                read.current,
            )
        })
        .collect();
    let mut expected = vec![
        (bolt.clone(), common::drafts::hash(1), CORRECT, false),
        (ring.clone(), common::drafts::hash(5), CORRECT, true),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen, expected,
        "the mountain was never judged by this account"
    );
    assert_eq!(
        mine.iter()
            .find(|read| read.oracle_id == ring)
            .map(|read| read.note.clone()),
        Some(None)
    );

    // A revived hash: the account's verdict on it is the current one again, though a newer verdict exists on the other hash.
    give(&database, &bolt, 2, &[(Some(1), WRONG)]).await;
    drafts.insert(&[draft(&bolt, 1)]).await.unwrap();
    let mine = database
        .reads()
        .of_account(user(1), &[key(&bolt, 1)])
        .await
        .unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(
        (mine[0].read_hash.clone(), mine[0].verdict, mine[0].current),
        (common::drafts::hash(1), CORRECT, true)
    );
    assert_eq!(
        database.reads().of_account(user(1), &[]).await.unwrap(),
        vec![]
    );
    assert_eq!(
        database.reads().of_account(user(4), &keys).await.unwrap(),
        vec![]
    );
}

#[sqlx::test]
async fn the_queue_lists_pending_drafts_then_approved_ones_the_account_has_not_judged(
    pool: PgPool,
) {
    let database = seeded(pool).await;
    let [bolt, ring, mountain, island, shock] =
        ["Lightning Bolt", "Sol Ring", "Mountain", "Island", "Shock"].map(oracle_of);
    database
        .drafts()
        .insert(&[
            draft(&bolt, 1),
            draft(&ring, 2),
            draft(&mountain, 3),
            draft(&island, 4),
        ])
        .await
        .unwrap();
    give(&database, &ring, 2, &[(Some(2), CORRECT)]).await;
    give(&database, &mountain, 3, &[(Some(1), CORRECT)]).await;
    give(&database, &island, 4, &[(Some(3), WRONG)]).await;
    // The shock has no draft; the deck lists the bolt twice.
    let deck = [
        mountain.clone(),
        ring.clone(),
        shock,
        island.clone(),
        bolt.clone(),
        mountain.clone(),
        bolt.clone(),
    ];
    let order = |reads: Vec<mtg_db::DraftRead>| -> Vec<OracleId> {
        reads.into_iter().map(|read| read.oracle_id).collect()
    };

    // User 1 judged the mountain: the pending bolt, then the approved ring.
    let queue = database.reads().queue(user(1), &deck).await.unwrap();
    assert_eq!(order(queue), [bolt.clone(), ring.clone()]);
    // User 2 judged the ring: the pending bolt, then the approved mountain.
    let queue = database.reads().queue(user(2), &deck).await.unwrap();
    assert_eq!(order(queue), [bolt.clone(), mountain.clone()]);
    // A new account reads the pending bolt first, then the approved mountain and ring in deck order.
    let queue = database.reads().queue(user(9), &deck).await.unwrap();
    assert_eq!(order(queue), [bolt.clone(), mountain, ring]);
    assert_eq!(database.reads().queue(user(9), &[]).await.unwrap(), vec![]);

    // The queue carries the state of each entry.
    let queue = database.reads().queue(user(9), &[bolt]).await.unwrap();
    assert_eq!((queue[0].state, queue[0].correct), (ReadState::Pending, 0));
}

#[sqlx::test]
async fn a_card_read_again_after_a_verdict_of_this_account_is_in_the_queue_again(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    database.drafts().insert(&[draft(&bolt, 1)]).await.unwrap();
    give(
        &database,
        &bolt,
        1,
        &[(Some(1), CORRECT), (Some(2), CORRECT)],
    )
    .await;
    assert_eq!(
        database
            .reads()
            .queue(user(1), std::slice::from_ref(&bolt))
            .await
            .unwrap(),
        vec![]
    );
    database.drafts().insert(&[draft(&bolt, 2)]).await.unwrap();
    let queue = database.reads().queue(user(1), &[bolt]).await.unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].state, ReadState::Pending);
}

#[sqlx::test]
async fn the_list_is_by_card_name_then_account_with_the_server_first_then_time(pool: PgPool) {
    let database = seeded(pool).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    database
        .drafts()
        .insert(&[draft(&bolt, 1), draft(&ring, 2)])
        .await
        .unwrap();
    give(&database, &ring, 2, &[(Some(1), CORRECT)]).await;
    give(
        &database,
        &bolt,
        1,
        &[
            (Some(2), WRONG),
            (Some(1), CORRECT),
            (None, WRONG),
            (Some(1), WRONG),
        ],
    )
    .await;

    let all = database.reads().list(false).await.unwrap();
    let seen: Vec<_> = all
        .iter()
        .map(|read| (read.name.as_str(), read.user_id, read.verdict))
        .collect();
    assert_eq!(
        seen,
        [
            ("Lightning Bolt", None, WRONG),
            ("Lightning Bolt", Some(user(1)), CORRECT),
            ("Lightning Bolt", Some(user(1)), WRONG),
            ("Lightning Bolt", Some(user(2)), WRONG),
            ("Sol Ring", Some(user(1)), CORRECT),
        ]
    );
    let wrong = database.reads().list(true).await.unwrap();
    assert_eq!(wrong.len(), 3);
    assert!(wrong.iter().all(|read| read.verdict == WRONG));
}
