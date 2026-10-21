//! Goal: every predicate shape the repository issues can be served by an index, so search stays fast as the card table grows to tens of thousands of rows.

use sqlx::{PgPool, Row};

/// Query shapes the repository issues, each with the index that must be able to serve it.
///
/// With sequential scans disabled the planner uses an index whenever one applies, so these fail
/// if a predicate is written in a form (or on a column) that no index covers.
const INDEXED_QUERIES: &[(&str, &str, &str)] = &[
    (
        "fuzzy name",
        "SELECT oracle_id FROM cards WHERE name_normalized % 'lightning'",
        "cards_name_trgm_idx",
    ),
    (
        "partial name",
        "SELECT oracle_id FROM cards WHERE 'bolt' <% name_normalized",
        "cards_name_trgm_idx",
    ),
    (
        "name contains",
        "SELECT oracle_id FROM cards WHERE name_normalized LIKE '%bolt%'",
        "cards_name_trgm_idx",
    ),
    (
        "type line contains",
        "SELECT oracle_id FROM cards WHERE type_line ILIKE '%creature%'",
        "cards_type_line_trgm_idx",
    ),
    (
        "oracle text",
        "SELECT oracle_id FROM cards WHERE oracle_tsv @@ websearch_to_tsquery('english', 'aaaa')",
        "cards_oracle_tsv_idx",
    ),
    (
        "legality",
        "SELECT oracle_id FROM cards WHERE legalities @> '{\"commander\": \"legal\"}'::jsonb",
        "cards_legalities_idx",
    ),
    (
        "mana value range",
        "SELECT oracle_id FROM cards WHERE mana_value >= 2 AND mana_value <= 2",
        "cards_mana_value_idx",
    ),
    (
        "exact colors",
        "SELECT oracle_id FROM cards WHERE colors = 9",
        "cards_colors_idx",
    ),
    (
        "exact color identity",
        "SELECT oracle_id FROM cards WHERE color_identity = 9",
        "cards_color_identity_idx",
    ),
    (
        "exact name",
        "SELECT oracle_id FROM cards WHERE name_normalized = 'lightning bolt'",
        "cards_name_order_idx",
    ),
    (
        "front face name",
        "SELECT oracle_id FROM cards WHERE front_name_normalized = 'fire'",
        "cards_front_name_idx",
    ),
    (
        "default ordering",
        "SELECT oracle_id FROM cards ORDER BY name_normalized, oracle_id LIMIT 10",
        "cards_name_order_idx",
    ),
    (
        "printings of a card",
        "SELECT printing_id FROM printings WHERE oracle_id = '00000000-0000-0000-0000-000000000001' ORDER BY released_at DESC NULLS LAST, printing_id",
        "printings_oracle_idx",
    ),
    (
        "rulings of a card",
        "SELECT id FROM rulings WHERE oracle_id = '00000000-0000-0000-0000-000000000001' ORDER BY published_at NULLS LAST, id",
        "rulings_oracle_idx",
    ),
    (
        "latest import run",
        "SELECT id FROM import_runs WHERE source = 'scryfall_cards' ORDER BY started_at DESC, id DESC LIMIT 1",
        "import_runs_source_started_idx",
    ),
];

/// Enough varied rows that the planner's cost estimates reflect a real card table.
const POPULATE: &str = "
    INSERT INTO cards (oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card,
                       mana_value, type_line, oracle_text, colors, color_identity, legalities, faces)
    SELECT ('00000000-0000-0000-0000-' || lpad(g::text, 12, '0'))::uuid, 'n' || g,
           md5(g::text) || ' ' || md5((g * 7)::text), md5((g * 3)::text), 'normal', true,
           g % 10, (ARRAY['Creature', 'Instant', 'Sorcery', 'Artifact', 'Land'])[g % 5 + 1]
               || ' ' || md5((g * 3)::text),
           md5((g * 5)::text) || ' ' || md5((g * 11)::text), g % 32, g % 32,
           CASE WHEN g % 50 = 0 THEN '{\"commander\": \"legal\"}'::jsonb ELSE '{}'::jsonb END,
           '[{}]'
    FROM generate_series(1, 4000) g;
    INSERT INTO printings (printing_id, oracle_id, set_code, collector_number, lang)
    SELECT gen_random_uuid(), oracle_id, 'tst', '1', 'en' FROM cards;
    INSERT INTO rulings (oracle_id, source, comment)
    SELECT oracle_id, 'wotc', 'ruling ' || oracle_id FROM cards;
    INSERT INTO import_runs (source) SELECT 'scryfall_cards' FROM generate_series(1, 200);
";

#[sqlx::test]
async fn indexes_can_serve_the_repository_queries(pool: PgPool) {
    sqlx::raw_sql(POPULATE).execute(&pool).await.unwrap();
    // VACUUM flushes GIN pending lists, which otherwise inflate the planner's GIN cost estimates.
    sqlx::raw_sql("VACUUM ANALYZE")
        .execute(&pool)
        .await
        .unwrap();

    let mut connection = pool.acquire().await.unwrap();
    sqlx::raw_sql("SET enable_seqscan = off")
        .execute(&mut *connection)
        .await
        .unwrap();
    let mut failures = Vec::new();
    for (label, sql, index) in INDEXED_QUERIES {
        let rows = sqlx::raw_sql(sqlx::AssertSqlSafe(format!("EXPLAIN {sql}")))
            .fetch_all(&mut *connection)
            .await
            .unwrap();
        let plan: Vec<String> = rows.iter().map(|row| row.get::<String, _>(0)).collect();
        let plan = plan.join("\n");
        if !plan.contains(index) {
            failures.push(format!("{label}: expected {index} in\n{plan}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
