//! Goal: what only the Postgres store promises: real game logs survive the jsonb round trip and a new connection pool, reads can start anywhere, a batch that fails midway leaves no rows, and a damaged or unreachable table is reported instead of guessed at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{game_id, reference, table_setup};
use mtg_core::ids::PlayerId;
use mtg_engine::{Event, replay};
use mtg_server::{EventStore, PgEventStore, StoreError, migrate};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

async fn store(pool: PgPool) -> PgEventStore {
    migrate(&pool).await.unwrap();
    PgEventStore::new(pool)
}

fn numbered(n: u64) -> Vec<Event> {
    (0..n)
        .map(|i| Event::FirstPlayerChosen {
            player: PlayerId(i),
        })
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_whole_game_survives_the_round_trip_in_odd_sized_batches(pool: PgPool) {
    let store = store(pool).await;
    for (n, (players, seed)) in [(2, 1), (3, 2), (4, 3)].into_iter().enumerate() {
        let id = game_id(n as u128);
        let played = reference(players, seed);

        let mut at = 0;
        for size in [1, 7, 2, 40].into_iter().cycle() {
            if at == played.events.len() {
                break;
            }
            let end = (at + size).min(played.events.len());
            store
                .append(id, at as u64, &played.events[at..end])
                .await
                .unwrap();
            at = end;
        }

        let loaded = store.load(id).await.unwrap();
        assert_eq!(loaded, played.events, "{players} players, seed {seed}");
        let replayed = replay(&table_setup(players, seed), &loaded).unwrap();
        assert_eq!(replayed.state_hash(), played.final_hash);
    }
}

#[sqlx::test(migrations = false)]
async fn reading_from_a_position_returns_the_rest_of_the_log(pool: PgPool) {
    let store = store(pool).await;
    let log = numbered(10);
    store.append(game_id(1), 0, &log).await.unwrap();
    store.append(game_id(2), 0, &log[..3]).await.unwrap();

    // (from_seq, index of the first event expected back)
    for (from_seq, first) in [
        (0, 0),
        (4, 4),
        (9, 9),
        (10, 10),
        (1_000, 10),
        (u64::MAX, 10),
    ] {
        assert_eq!(
            store.read_from(game_id(1), from_seq).await.unwrap(),
            log[first..],
            "from {from_seq}"
        );
        assert_eq!(store.read_from(game_id(9), from_seq).await.unwrap(), []);
    }
    assert_eq!(store.read_from(game_id(2), 2).await.unwrap(), log[2..3]);
}

#[sqlx::test(migrations = false)]
async fn a_log_outlives_the_pool_that_wrote_it(
    pool_options: PgPoolOptions,
    options: PgConnectOptions,
) {
    let pool = pool_options.connect_with(options.clone()).await.unwrap();
    let played = reference(2, 5);
    let writer = store(pool.clone()).await;
    writer.append(game_id(1), 0, &played.events).await.unwrap();
    drop(writer);
    pool.close().await;

    // A restarted server opens its own pool on the same database.
    let reborn = PgPoolOptions::new().connect_with(options).await.unwrap();
    let reader = store(reborn).await;
    assert_eq!(reader.load(game_id(1)).await.unwrap(), played.events);
    reader
        .append(game_id(1), played.events.len() as u64, &played.events[..2])
        .await
        .unwrap();
    assert_eq!(
        reader.load(game_id(1)).await.unwrap().len(),
        played.events.len() + 2
    );
}

#[sqlx::test(migrations = false)]
async fn a_batch_that_fails_midway_leaves_no_rows(pool: PgPool) {
    let store = store(pool.clone()).await;
    let log = reference(2, 1).events;
    store.append(game_id(1), 0, &log[..3]).await.unwrap();

    // The database refuses the fourth row of the batch, after three were already written.
    sqlx::query(
        "CREATE FUNCTION refuse_seq_six() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'refused row %', NEW.seq; END $$",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER refuse_six BEFORE INSERT ON game_events
         FOR EACH ROW WHEN (NEW.seq = 6) EXECUTE FUNCTION refuse_seq_six()",
    )
    .execute(&pool)
    .await
    .unwrap();

    let failed = store.append(game_id(1), 3, &log[3..10]).await;
    match failed {
        Err(StoreError::Unavailable(message)) => assert!(message.contains("refused row 6")),
        other => panic!("expected the database error, got {other:?}"),
    }
    assert_eq!(store.load(game_id(1)).await.unwrap(), log[..3]);

    // Once the database cooperates the same batch goes in whole.
    sqlx::query("DROP TRIGGER refuse_six ON game_events")
        .execute(&pool)
        .await
        .unwrap();
    store.append(game_id(1), 3, &log[3..10]).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap(), log[..10]);
}

async fn insert_raw(pool: &PgPool, game: u128, seq: i64, event: serde_json::Value) {
    sqlx::query("INSERT INTO game_events (game_id, seq, event) VALUES ($1, $2, $3)")
        .bind(game_id(game).uuid())
        .bind(seq)
        .bind(event)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_damaged_log_is_reported_as_corrupt(pool: PgPool) {
    let store = store(pool.clone()).await;
    let good = serde_json::to_value(Event::MulliganPhaseBegan).unwrap();

    // Game 1 lost its second event.
    insert_raw(&pool, 1, 0, good.clone()).await;
    insert_raw(&pool, 1, 2, good.clone()).await;
    // Game 2 holds something that is no event.
    insert_raw(&pool, 2, 0, good.clone()).await;
    insert_raw(&pool, 2, 1, serde_json::json!({"NoSuchEvent": {"x": 1}})).await;
    // Game 3 does not start at its first event.
    insert_raw(&pool, 3, 1, good).await;

    for (game, from_seq) in [(1, 0), (2, 0), (3, 0)] {
        let result = store.read_from(game_id(game), from_seq).await;
        let Err(StoreError::Corrupt(message)) = result else {
            panic!("game {game} from {from_seq}: expected Corrupt, got {result:?}");
        };
        assert!(message.contains(&game_id(game).to_string()), "{message}");
    }
    let gap = store.read_from(game_id(1), 0).await.unwrap_err();
    assert!(
        gap.to_string().contains("expected event 1, found 2"),
        "{gap}"
    );
    // The intact part of a damaged log can still be read.
    assert_eq!(store.read_from(game_id(1), 2).await.unwrap().len(), 1);
    assert_eq!(store.read_from(game_id(3), 1).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = false)]
async fn an_unreachable_database_is_reported_as_unavailable(pool: PgPool) {
    let store = store(pool.clone()).await;
    store
        .append(game_id(1), 0, &[Event::MulliganPhaseBegan])
        .await
        .unwrap();
    pool.close().await;

    let append = store
        .append(game_id(1), 1, &[Event::MulliganPhaseBegan])
        .await;
    assert!(
        matches!(append, Err(StoreError::Unavailable(_))),
        "{append:?}"
    );
    let load = store.load(game_id(1)).await;
    assert!(matches!(load, Err(StoreError::Unavailable(_))), "{load:?}");
}

#[sqlx::test(migrations = false)]
async fn a_store_before_migration_is_reported_as_unavailable(pool: PgPool) {
    let store = PgEventStore::new(pool);
    let load = store.load(game_id(1)).await;
    assert!(matches!(load, Err(StoreError::Unavailable(_))), "{load:?}");
}
