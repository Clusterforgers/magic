//! Goal: compiled rules are stored one row per card and read back as written, a row is replaced by writing the card again, rows are removed by id or with their card, the version moves with every change and with nothing else, the counts follow the statuses, and the database refuses a row without a card, with the status `Manual` or with a malformed fingerprint.

use crate::common;

use mtg_db::{CardRulesRepo, DbError, ImplStatus, NewRules, RuleCounts, RuleStamp};
use mtg_import::ids::OracleId;
use serde_json::json;
use sqlx::PgPool;

use common::{oracle_of, seeded};

const FINGERPRINT: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210";

fn rules(oracle_id: &OracleId, status: ImplStatus, fingerprint: &str, version: &str) -> NewRules {
    NewRules {
        oracle_id: oracle_id.clone(),
        rules: json!({"name": "A card", "faces": []}),
        status,
        fingerprint: fingerprint.to_owned(),
        compiler_version: version.to_owned(),
    }
}

async fn repo(pool: PgPool) -> CardRulesRepo {
    seeded(pool).await.card_rules()
}

#[sqlx::test]
async fn a_row_reads_back_exactly_as_it_was_written(pool: PgPool) {
    let repo = repo(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    let row = NewRules {
        rules: json!({"b": [1, 2], "a": {"x": null}}),
        ..rules(&bolt, ImplStatus::Partial, FINGERPRINT, "v1")
    };

    assert_eq!(repo.put(std::slice::from_ref(&row)).await.unwrap(), 1);

    let stored = repo.all().await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].oracle_id, bolt);
    assert_eq!(stored[0].rules, row.rules);
    assert_eq!(stored[0].status, ImplStatus::Partial);
    assert_eq!(stored[0].fingerprint, FINGERPRINT);
    assert_eq!(stored[0].compiler_version, "v1");
    assert_eq!(
        repo.stamps().await.unwrap(),
        [RuleStamp {
            oracle_id: bolt,
            status: ImplStatus::Partial,
            fingerprint: FINGERPRINT.into(),
            compiler_version: "v1".into(),
        }]
    );
}

#[sqlx::test]
async fn writing_a_card_again_replaces_its_row(pool: PgPool) {
    let repo = repo(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    repo.put(&[rules(&bolt, ImplStatus::Partial, FINGERPRINT, "v1")])
        .await
        .unwrap();
    let again = NewRules {
        rules: json!({"changed": true}),
        ..rules(&bolt, ImplStatus::Full, OTHER, "v2")
    };

    assert_eq!(repo.put(std::slice::from_ref(&again)).await.unwrap(), 1);

    let stored = repo.all().await.unwrap();
    assert_eq!(stored.len(), 1, "one row per card");
    assert_eq!(stored[0].rules, json!({"changed": true}));
    assert_eq!(stored[0].status, ImplStatus::Full);
    assert_eq!(
        (
            stored[0].fingerprint.as_str(),
            stored[0].compiler_version.as_str()
        ),
        (OTHER, "v2")
    );
}

#[sqlx::test]
async fn rows_are_removed_by_id_and_with_their_card(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let repo = database.card_rules();
    let (bolt, mountain, island) = (
        oracle_of("Lightning Bolt"),
        oracle_of("Mountain"),
        oracle_of("Island"),
    );
    let rows: Vec<NewRules> = [&bolt, &mountain, &island]
        .into_iter()
        .map(|id| rules(id, ImplStatus::Full, FINGERPRINT, "v1"))
        .collect();
    repo.put(&rows).await.unwrap();

    assert_eq!(repo.remove(std::slice::from_ref(&bolt)).await.unwrap(), 1);
    assert_eq!(
        repo.remove(std::slice::from_ref(&bolt)).await.unwrap(),
        0,
        "once"
    );
    assert_eq!(
        repo.remove(&[OracleId("not an id".into())]).await.unwrap(),
        0
    );
    sqlx::query("DELETE FROM cards WHERE oracle_id = $1::uuid")
        .bind(mountain.as_str())
        .execute(&pool)
        .await
        .unwrap();

    let left: Vec<OracleId> = repo
        .all()
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.oracle_id)
        .collect();
    assert_eq!(left, [island]);
}

#[sqlx::test]
async fn the_version_moves_with_every_change_and_with_nothing_else(pool: PgPool) {
    let repo = repo(pool).await;
    let (bolt, mountain) = (oracle_of("Lightning Bolt"), oracle_of("Mountain"));
    let empty = repo.version().await.unwrap();
    assert_eq!(empty, repo.version().await.unwrap());

    repo.put(&[rules(&bolt, ImplStatus::Full, FINGERPRINT, "v1")])
        .await
        .unwrap();
    let one = repo.version().await.unwrap();
    assert_ne!(one, empty, "a row was added");
    assert_eq!(
        one,
        repo.version().await.unwrap(),
        "reading changes nothing"
    );

    repo.put(&[rules(&bolt, ImplStatus::Full, OTHER, "v1")])
        .await
        .unwrap();
    let replaced = repo.version().await.unwrap();
    assert_ne!(replaced, one, "a row was replaced");

    repo.put(&[rules(&mountain, ImplStatus::Partial, FINGERPRINT, "v1")])
        .await
        .unwrap();
    let two = repo.version().await.unwrap();
    assert_ne!(two, replaced, "another row was added");

    repo.remove(&[mountain]).await.unwrap();
    assert_ne!(repo.version().await.unwrap(), two, "a row was removed");
}

#[sqlx::test]
async fn the_counts_follow_the_statuses(pool: PgPool) {
    let repo = repo(pool).await;
    assert_eq!(repo.counts().await.unwrap(), RuleCounts::default());
    let rows: Vec<NewRules> = [
        ("Lightning Bolt", ImplStatus::Full),
        ("Mountain", ImplStatus::Full),
        ("Island", ImplStatus::Partial),
    ]
    .into_iter()
    .map(|(name, status)| rules(&oracle_of(name), status, FINGERPRINT, "v1"))
    .collect();
    repo.put(&rows).await.unwrap();

    assert_eq!(
        repo.counts().await.unwrap(),
        RuleCounts {
            full: 2,
            partial: 1
        }
    );
}

#[sqlx::test]
async fn a_batch_with_one_bad_row_writes_nothing(pool: PgPool) {
    let repo = repo(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    let rows = [
        rules(&bolt, ImplStatus::Full, FINGERPRINT, "v1"),
        rules(&oracle_of("Mountain"), ImplStatus::Full, "short", "v1"),
    ];

    assert!(matches!(repo.put(&rows).await, Err(DbError::Query(_))));

    assert!(repo.all().await.unwrap().is_empty(), "no rows");
}

#[sqlx::test]
async fn the_database_refuses_what_the_compiler_never_writes(pool: PgPool) {
    let repo = repo(pool).await;
    let bolt = oracle_of("Lightning Bolt");
    let unknown = OracleId(uuid::Uuid::from_u128(77).to_string());
    let bad = [
        rules(&bolt, ImplStatus::Manual, FINGERPRINT, "v1"),
        rules(&bolt, ImplStatus::Full, "XYZ", "v1"),
        rules(&bolt, ImplStatus::Full, FINGERPRINT, ""),
        NewRules {
            rules: json!([1]),
            ..rules(&bolt, ImplStatus::Full, FINGERPRINT, "v1")
        },
        rules(&unknown, ImplStatus::Full, FINGERPRINT, "v1"),
    ];
    for row in bad {
        let outcome = repo.put(std::slice::from_ref(&row)).await;
        assert!(matches!(outcome, Err(DbError::Query(_))), "{row:?}");
    }
    let malformed = rules(
        &OracleId("not an id".into()),
        ImplStatus::Full,
        FINGERPRINT,
        "v1",
    );
    assert!(matches!(
        repo.put(&[malformed]).await,
        Err(DbError::Corrupt { .. })
    ));

    assert!(repo.all().await.unwrap().is_empty(), "no rows");
}
