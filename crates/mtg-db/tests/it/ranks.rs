//! Goal: the ranks are replaced whole, keep only cards the database has, and order every read queue by popularity within its pending and approved parts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_db::Verdict;
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use uuid::Uuid;

use common::drafts::{draft, key};
use common::{oracle_of, seeded};

#[sqlx::test]
async fn a_replace_keeps_known_cards_and_drops_the_old_ranks(pool: PgPool) {
    let database = seeded(pool).await;
    let [bolt, ring, shock] = ["Lightning Bolt", "Sol Ring", "Shock"].map(oracle_of);
    let ranks = database.ranks();
    let unknown = OracleId(Uuid::from_u128(7).to_string());
    let stored = ranks
        .replace(&[
            (bolt.clone(), 40),
            (ring.clone(), 1),
            (unknown.clone(), 3),
            (OracleId("no-uuid".into()), 4),
            (shock.clone(), 0),
            (bolt.clone(), 99),
        ])
        .await
        .unwrap();
    assert_eq!(stored, 2);
    let got = ranks
        .of(&[bolt.clone(), ring.clone(), shock.clone(), unknown])
        .await
        .unwrap();
    assert_eq!(
        (got.get(&bolt), got.get(&ring), got.len()),
        (Some(&40), Some(&1), 2)
    );

    assert_eq!(ranks.replace(&[(shock.clone(), 5)]).await.unwrap(), 1);
    let got = ranks.of(&[bolt, ring, shock.clone()]).await.unwrap();
    assert_eq!(got.into_iter().collect::<Vec<_>>(), [(shock, 5)]);
    assert_eq!(ranks.count().await.unwrap(), 1);
}

#[sqlx::test]
async fn the_queue_reads_the_most_played_cards_first_within_each_state(pool: PgPool) {
    let database = seeded(pool).await;
    let [bolt, ring, mountain, island, shock] =
        ["Lightning Bolt", "Sol Ring", "Mountain", "Island", "Shock"].map(oracle_of);
    let cards = [&bolt, &ring, &mountain, &island, &shock];
    let drafts: Vec<_> = cards
        .iter()
        .zip(1..)
        .map(|(card, n)| draft(card, n))
        .collect();
    database.drafts().insert(&drafts).await.unwrap();
    // The island and the mountain are approved by another account; the rest wait for a first read.
    for (card, n) in [(&mountain, 3), (&island, 4)] {
        database
            .reads()
            .append(
                Some(Uuid::from_u128(2)),
                &key(card, n),
                Verdict::Correct,
                None,
            )
            .await
            .unwrap();
    }
    // The shock and the mountain have no rank.
    database
        .ranks()
        .replace(&[(bolt.clone(), 900), (ring.clone(), 1), (island.clone(), 50)])
        .await
        .unwrap();
    let deck = [
        shock.clone(),
        mountain.clone(),
        bolt.clone(),
        island.clone(),
        ring.clone(),
    ];
    let order: Vec<OracleId> = database
        .reads()
        .queue(Uuid::from_u128(1), &deck)
        .await
        .unwrap()
        .into_iter()
        .map(|read| read.oracle_id)
        .collect();
    assert_eq!(order, [ring, bolt, shock, island, mountain]);
}
