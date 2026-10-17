//! Goal: the schema itself rejects invalid rows, cascades deletes the way the importers rely on, and keeps the generated search vector in step with the text.

use sqlx::PgPool;

/// A card insert with every column the checks care about defaulted; `{}` is replaced per case.
const CARD_PREFIX: &str = "INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card, mana_value, faces";

/// SQL that must be rejected, with the SQLSTATE the schema should answer with.
fn violations() -> Vec<(&'static str, String, &'static str)> {
    let card = |columns: &str, values: &str| {
        format!(
            "{CARD_PREFIX}{columns}) VALUES (gen_random_uuid(), 'x', 'x', 'x', 'normal', true, 0, '[{{}}]'{values})"
        )
    };
    let bad_faces = |faces: &str| {
        format!(
            "{CARD_PREFIX}) VALUES (gen_random_uuid(), 'x', 'x', 'x', 'normal', true, 0, '{faces}')"
        )
    };
    vec![
        ("color mask above five bits", card(", colors", ", 32"), "23514"),
        ("negative identity mask", card(", color_identity", ", -1"), "23514"),
        ("produced mana mask above five bits", card(", produced_mana", ", 99"), "23514"),
        ("legalities must be an object", card(", legalities", ", '[]'"), "23514"),
        ("unknown impl status", card(", impl_status", ", 'nearly'"), "22P02"),
        ("faces must not be empty", bad_faces("[]"), "23514"),
        ("faces must be an array", bad_faces("{}"), "23514"),
        (
            "empty name",
            "INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card, mana_value, faces)
             VALUES (gen_random_uuid(), '', 'x', 'x', 'normal', true, 0, '[{}]')"
                .to_owned(),
            "23514",
        ),
        (
            "negative mana value column",
            "INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card, mana_value, faces)
             VALUES (gen_random_uuid(), 'x', 'x', 'x', 'normal', true, -1, '[{}]')"
                .to_owned(),
            "23514",
        ),
        (
            "printing without a card",
            "INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang)
             VALUES (gen_random_uuid(), gen_random_uuid(), 'tst', '1', 'en')"
                .to_owned(),
            "23503",
        ),
        (
            "ruling without a card",
            "INSERT INTO rulings (oracle_id, source, comment) VALUES (gen_random_uuid(), 'wotc', 'text')"
                .to_owned(),
            "23503",
        ),
        (
            "precon card of a missing deck",
            "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count) VALUES (999, 0, 'main', 'x', 1)"
                .to_owned(),
            "23503",
        ),
        (
            "import run finished while running",
            "INSERT INTO import_runs (source, status, finished_at) VALUES ('scryfall_cards', 'running', now())"
                .to_owned(),
            "23514",
        ),
        (
            "failed import run without an error",
            "INSERT INTO import_runs (source, status, finished_at) VALUES ('scryfall_cards', 'failed', now())"
                .to_owned(),
            "23514",
        ),
        (
            "unknown import source",
            "INSERT INTO import_runs (source) VALUES ('carrier_pigeon')".to_owned(),
            "22P02",
        ),
    ]
}

/// Rows the dependent violations can point at: one card, printing, ruling and deck.
const PARENTS: &str = "
    INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card, mana_value, faces)
    VALUES ('00000000-0000-0000-0000-000000000001', 'x', 'x', 'x', 'normal', true, 0, '[{}]');
    INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang)
    VALUES ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-000000000001', 's', '1', 'en');
    INSERT INTO rulings (oracle_id, source, published_at, comment)
    VALUES ('00000000-0000-0000-0000-000000000001', 'wotc', NULL, 'text');
    INSERT INTO precon_decks (code, name) VALUES ('C', 'Deck');
";

/// Violations that need the parent rows to exist first.
const DEPENDENT_VIOLATIONS: &[(&str, &str, &str)] = &[
    (
        "duplicate printing id",
        "INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang)
         VALUES ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-000000000001', 's', '2', 'en')",
        "23505",
    ),
    (
        "unknown rarity",
        "INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang, rarity)
         VALUES (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 's', '2', 'en', 'legendary')",
        "23514",
    ),
    (
        "empty language",
        "INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang)
         VALUES (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 's', '2', '')",
        "23514",
    ),
    (
        "duplicate ruling, even with a missing date",
        "INSERT INTO rulings (oracle_id, source, published_at, comment)
         VALUES ('00000000-0000-0000-0000-000000000001', 'wotc', NULL, 'text')",
        "23505",
    ),
    (
        "unknown ruling source",
        "INSERT INTO rulings (oracle_id, source, comment)
         VALUES ('00000000-0000-0000-0000-000000000001', 'rumor', 'other text')",
        "23514",
    ),
    (
        "empty ruling text",
        "INSERT INTO rulings (oracle_id, source, comment)
         VALUES ('00000000-0000-0000-0000-000000000001', 'wotc', '')",
        "23514",
    ),
    (
        "duplicate deck",
        "INSERT INTO precon_decks (code, name) VALUES ('C', 'Deck')",
        "23505",
    ),
    (
        "zero copies",
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count)
         SELECT id, 0, 'main', 'x', 0 FROM precon_decks",
        "23514",
    ),
    (
        "unknown board",
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count)
         SELECT id, 0, 'maybeboard', 'x', 1 FROM precon_decks",
        "23514",
    ),
    (
        "deck card pointing at a missing card",
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count, oracle_id)
         SELECT id, 0, 'main', 'x', 1, gen_random_uuid() FROM precon_decks",
        "23503",
    ),
];

async fn sqlstate(pool: &PgPool, sql: &str) -> Option<String> {
    match sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
    {
        Ok(_) => None,
        Err(sqlx::Error::Database(error)) => error.code().map(std::borrow::Cow::into_owned),
        Err(other) => panic!("unexpected error: {other}"),
    }
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

#[sqlx::test]
async fn constraints_reject_invalid_rows(pool: PgPool) {
    let mut failures = Vec::new();
    for (label, sql, expected) in violations() {
        let state = sqlstate(&pool, &sql).await;
        if state.as_deref() != Some(expected) {
            failures.push(format!("{label}: expected {expected}, got {state:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[sqlx::test]
async fn constraints_reject_rows_that_clash_with_existing_ones(pool: PgPool) {
    sqlx::raw_sql(PARENTS).execute(&pool).await.unwrap();
    let mut failures = Vec::new();
    for (label, sql, expected) in DEPENDENT_VIOLATIONS {
        let state = sqlstate(&pool, sql).await;
        if state.as_deref() != Some(*expected) {
            failures.push(format!("{label}: expected {expected}, got {state:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[sqlx::test]
async fn deleting_a_card_cascades_and_unlinks_deck_entries(pool: PgPool) {
    sqlx::raw_sql(PARENTS).execute(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count, oracle_id)
         SELECT id, 0, 'main', 'x', 1, '00000000-0000-0000-0000-000000000001' FROM precon_decks",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("DELETE FROM cards")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(count(&pool, "SELECT count(*) FROM printings").await, 0);
    assert_eq!(count(&pool, "SELECT count(*) FROM rulings").await, 0);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM precon_deck_cards WHERE oracle_id IS NULL"
        )
        .await,
        1
    );
}

#[sqlx::test]
async fn deleting_a_deck_removes_its_entries(pool: PgPool) {
    sqlx::raw_sql(PARENTS).execute(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, count)
         SELECT id, 0, 'main', 'x', 1 FROM precon_decks",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("DELETE FROM precon_decks")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM precon_deck_cards").await,
        0
    );
}

#[sqlx::test]
async fn generated_search_vector_follows_the_oracle_text(pool: PgPool) {
    sqlx::raw_sql(
        "INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card, mana_value, oracle_text, faces)
         VALUES (gen_random_uuid(), 'x', 'x', 'x', 'normal', true, 0, 'Destroys creatures', '[{}]')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let matches = |query: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM cards WHERE oracle_tsv @@ websearch_to_tsquery('english', $1)",
            )
            .bind(query)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(matches("destroy creature").await, 1);
    assert_eq!(matches("counter").await, 0);
    sqlx::raw_sql("UPDATE cards SET oracle_text = 'Counter target spell'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(matches("destroy creature").await, 0);
    assert_eq!(matches("counter").await, 1);
}
