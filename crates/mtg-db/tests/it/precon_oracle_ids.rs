//! Goal: `PreconRepo::oracle_ids` returns each stored card the decks name once, on any board, without unmatched entries or cards no deck names, in a stable order.

use crate::common;

use mtg_import::ids::OracleId;
use sqlx::PgPool;

use common::precon::{deck, entry, ingest};
use common::{oracle_of, seeded};

#[sqlx::test]
async fn only_cards_of_the_decks_come_back_once_in_id_order(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.precons();
    assert_eq!(repo.oracle_ids().await.unwrap(), Vec::<OracleId>::new());

    let mut burn = deck(
        "Burn",
        "TST",
        None,
        vec![entry("Shock", 2), entry("No Such Card", 1)],
    );
    burn.side_board = vec![entry("Lightning Bolt", 1)];
    burn.commander = vec![entry("Sol Ring", 1)];
    let again = deck("Again", "TST", None, vec![entry("Shock", 4)]);
    ingest(&database, vec![burn, again]).await;

    let ids = repo.oracle_ids().await.unwrap();
    let mut expected: Vec<OracleId> = ["Shock", "Lightning Bolt", "Sol Ring"]
        .into_iter()
        .map(oracle_of)
        .collect();
    expected.sort();
    assert_eq!(ids, expected);
    assert!(!ids.contains(&oracle_of("Mountain")));
    assert_eq!(repo.oracle_ids().await.unwrap(), ids);
}
