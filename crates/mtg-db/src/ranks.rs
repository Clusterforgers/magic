//! The popularity of cards: Scryfall's EDHREC rank of each card that has one (1 is the most played). `mtg-cli ingest`
//! replaces the table from the Oracle Cards file; the read queues list cards by it.

use std::collections::HashMap;

use mtg_import::ids::OracleId;
use sqlx::PgPool;

use crate::drafts::uuid_of;
use crate::error::DbError;

/// Reads and replaces the ranks.
#[derive(Debug, Clone)]
pub struct RankRepo {
    pool: PgPool,
}

impl RankRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Replaces every rank with `ranks` in one transaction and returns how many were stored: a card the database
    /// lacks, an id that is no uuid and a rank of 0 are left out, and a card named twice keeps its first rank.
    pub async fn replace(&self, ranks: &[(OracleId, u32)]) -> Result<u64, DbError> {
        let mut ids = Vec::with_capacity(ranks.len());
        let mut values = Vec::with_capacity(ranks.len());
        for (id, rank) in ranks {
            let (Some(uuid), Ok(rank)) = (uuid_of(id), i32::try_from(*rank)) else {
                continue;
            };
            if rank > 0 {
                ids.push(uuid);
                values.push(rank);
            }
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query!("DELETE FROM card_ranks")
            .execute(&mut *tx)
            .await?;
        let stored = sqlx::query!(
            "INSERT INTO card_ranks (oracle_id, edhrec_rank)
             SELECT r.oracle_id, r.edhrec_rank
             FROM unnest($1::uuid[], $2::int4[]) WITH ORDINALITY AS r(oracle_id, edhrec_rank, at)
             JOIN cards c ON c.oracle_id = r.oracle_id
             ORDER BY r.at
             ON CONFLICT (oracle_id) DO NOTHING",
            &ids,
            &values,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(stored)
    }

    /// The rank of each of `cards` that has one.
    pub async fn of(&self, cards: &[OracleId]) -> Result<HashMap<OracleId, u32>, DbError> {
        let ids: Vec<_> = cards.iter().filter_map(uuid_of).collect();
        let rows = sqlx::query!(
            "SELECT oracle_id, edhrec_rank FROM card_ranks WHERE oracle_id = ANY($1)",
            &ids,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let rank = u32::try_from(row.edhrec_rank).ok()?;
                Some((OracleId(row.oracle_id.to_string()), rank))
            })
            .collect())
    }

    /// How many cards have a rank.
    pub async fn count(&self) -> Result<u64, DbError> {
        let count = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM card_ranks"#)
            .fetch_one(&self.pool)
            .await?;
        Ok(u64::try_from(count).unwrap_or(0))
    }
}

/// Orders `cards` by popularity, most played first, keeping the given order among cards of one rank and after the
/// ranked ones for the cards without a rank.
pub fn by_rank<T>(cards: &mut [T], rank: impl Fn(&T) -> Option<u32>) {
    cards.sort_by_key(|card| rank(card).map_or((1, 0), |rank| (0, rank)));
}

#[cfg(test)]
mod tests {
    //! Goal: popularity order puts ranked cards first by rank, then the unranked ones in their given order.
    use super::*;

    #[test]
    fn ranked_cards_come_first_by_rank_and_the_rest_keep_their_order() {
        let mut cards = [
            ("a", None),
            ("b", Some(30)),
            ("c", Some(2)),
            ("d", None),
            ("e", Some(30)),
        ];
        by_rank(&mut cards, |card| card.1);
        let names: Vec<_> = cards.iter().map(|card| card.0).collect();
        assert_eq!(names, ["c", "b", "e", "a", "d"]);
    }
}
