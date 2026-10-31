//! Read-tier verdicts: what accounts (and the server, for a resolution that failed in a game) said of a
//! draft's read hash. The table is append-only: an account's newest verdict on a hash is the one that
//! counts, one `wrong` anywhere on the hash wins, and only the deletion of an account removes rows.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use mtg_import::ids::OracleId;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::drafts::{DraftKey, unzip_keys, uuid_of};
use crate::error::DbError;
use crate::fix_marks::clean;

/// What a verdict says of a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "read_verdict", rename_all = "lowercase")]
pub enum Verdict {
    /// The draft does what the card says.
    Correct,
    /// The draft does not do what the card says; a note says what.
    Wrong,
}

/// Where a draft stands, from the verdicts on its read hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadState {
    /// No counting verdict yet.
    Pending,
    /// A counting verdict is `correct` and none is `wrong`.
    Approved,
    /// A counting verdict is `wrong`.
    Wrong,
}

impl ReadState {
    /// The state of a hash with `wrong` and `correct` counting verdicts: one wrong wins.
    pub fn of_counts(wrong: u32, correct: u32) -> Self {
        if wrong > 0 {
            Self::Wrong
        } else if correct > 0 {
            Self::Approved
        } else {
            Self::Pending
        }
    }
}

/// A stored verdict with the name of its card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRead {
    /// Position in the table: later verdicts have larger ids.
    pub id: i64,
    /// The account; none for the server's own verdict.
    pub user_id: Option<Uuid>,
    /// The card.
    pub oracle_id: OracleId,
    /// The card's name.
    pub name: String,
    /// The read hash the verdict is on.
    pub read_hash: String,
    /// What the verdict says.
    pub verdict: Verdict,
    /// Why, trimmed; always there for `wrong`.
    pub note: Option<String>,
    /// When the verdict was given.
    pub created_at: DateTime<Utc>,
}

/// The read state of a card's current draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRead {
    /// The card.
    pub oracle_id: OracleId,
    /// The read hash of its current draft.
    pub read_hash: String,
    /// Where the draft stands.
    pub state: ReadState,
    /// How many counting verdicts are `correct`: the accounts that judged it correct.
    pub correct: u32,
    /// How many counting verdicts are `wrong`: accounts whose newest verdict is `wrong`, and the server's.
    pub wrong: u32,
}

/// An account's verdict on a card: on the current hash when it has one there, else its newest on an older hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRead {
    /// The card.
    pub oracle_id: OracleId,
    /// The read hash the verdict is on.
    pub read_hash: String,
    /// What the verdict says.
    pub verdict: Verdict,
    /// Why.
    pub note: Option<String>,
    /// When the verdict was given.
    pub created_at: DateTime<Utc>,
    /// Whether the verdict is on the current hash; false means the card was read again since.
    pub current: bool,
}

/// Reads and writes verdicts.
#[derive(Debug, Clone)]
pub struct ReadRepo {
    pool: PgPool,
}

/// The count of a `count(*)` as the unsigned number it is.
fn count_of(count: i64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// What a deck's reader has left to read, in deck order: the pending drafts first, then the approved
/// ones whose current hash this account has not judged. A card with a `wrong` draft or none is left out,
/// and a card listed twice once.
fn queue_order(
    cards: &[OracleId],
    states: Vec<DraftRead>,
    judged: &HashSet<OracleId>,
) -> Vec<DraftRead> {
    let mut by_card: HashMap<OracleId, DraftRead> = states
        .into_iter()
        .map(|state| (state.oracle_id.clone(), state))
        .collect();
    let (mut pending, mut approved) = (Vec::new(), Vec::new());
    for card in cards {
        let Some(state) = by_card.remove(card) else {
            continue;
        };
        match state.state {
            ReadState::Pending => pending.push(state),
            ReadState::Approved if !judged.contains(card) => approved.push(state),
            ReadState::Approved | ReadState::Wrong => {}
        }
    }
    pending.extend(approved);
    pending
}

/// The ids as the uuid array a query takes; one that is no uuid names no card.
fn uuids_of(cards: &[OracleId]) -> Vec<Uuid> {
    cards.iter().filter_map(uuid_of).collect()
}

impl ReadRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Appends a verdict on `draft` and returns it; `by` is the account, none for the server's own
    /// verdict. The note is trimmed and an empty one is none. `None` if no stored draft has the key
    /// (a retired draft takes verdicts too: a game can still be playing it).
    ///
    /// The database refuses a `wrong` without a note, a server verdict that is not `wrong` and a note
    /// over [`crate::MAX_NOTE_CHARS`] as errors, and never changes or removes a row it has.
    pub async fn append(
        &self,
        by: Option<Uuid>,
        draft: &DraftKey,
        verdict: Verdict,
        note: Option<&str>,
    ) -> Result<Option<StoredRead>, DbError> {
        let Some(oracle) = uuid_of(&draft.oracle_id) else {
            return Ok(None);
        };
        let row = sqlx::query!(
            r#"WITH written AS (
                   INSERT INTO card_reads (user_id, oracle_id, read_hash, verdict, note)
                   SELECT $1, d.oracle_id, d.read_hash, $4::read_verdict, $5
                   FROM card_drafts d WHERE d.oracle_id = $2 AND d.read_hash = $3
                   RETURNING id, user_id, oracle_id, read_hash, verdict, note, created_at
               )
               SELECT w.id AS "id!", w.user_id, w.oracle_id AS "oracle_id!", c.name AS "name!",
                      w.read_hash AS "read_hash!", w.verdict AS "verdict!: Verdict", w.note,
                      w.created_at AS "created_at!"
               FROM written w JOIN cards c ON c.oracle_id = w.oracle_id"#,
            by,
            oracle,
            draft.read_hash,
            verdict as Verdict,
            clean(note)
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| StoredRead {
            id: row.id,
            user_id: row.user_id,
            oracle_id: OracleId(row.oracle_id.to_string()),
            name: row.name,
            read_hash: row.read_hash,
            verdict: row.verdict,
            note: row.note,
            created_at: row.created_at,
        }))
    }

    /// The read state of every card's current draft, by oracle id, in one query. A verdict on another
    /// hash counts for nothing.
    pub async fn state(&self) -> Result<Vec<DraftRead>, DbError> {
        self.states(None).await
    }

    /// The read state of the current drafts of these cards, by oracle id; a card with none is left out.
    pub async fn state_of(&self, cards: &[OracleId]) -> Result<Vec<DraftRead>, DbError> {
        self.states(Some(&uuids_of(cards))).await
    }

    async fn states(&self, cards: Option<&[Uuid]>) -> Result<Vec<DraftRead>, DbError> {
        let rows = sqlx::query!(
            r#"WITH counting AS (
                   SELECT r.oracle_id, r.read_hash, r.verdict FROM card_reads r
                   WHERE r.user_id IS NULL
                      OR r.id = (SELECT max(n.id) FROM card_reads n
                                 WHERE n.user_id = r.user_id AND n.oracle_id = r.oracle_id
                                   AND n.read_hash = r.read_hash)
               )
               SELECT d.oracle_id, d.read_hash,
                      count(c.verdict) FILTER (WHERE c.verdict = 'wrong') AS "wrong!",
                      count(c.verdict) FILTER (WHERE c.verdict = 'correct') AS "correct!"
               FROM card_drafts d
               LEFT JOIN counting c ON c.oracle_id = d.oracle_id AND c.read_hash = d.read_hash
               WHERE NOT d.retired AND ($1::uuid[] IS NULL OR d.oracle_id = ANY($1))
               GROUP BY d.oracle_id, d.read_hash
               ORDER BY d.oracle_id"#,
            cards
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let (wrong, correct) = (count_of(row.wrong), count_of(row.correct));
                DraftRead {
                    oracle_id: OracleId(row.oracle_id.to_string()),
                    read_hash: row.read_hash,
                    state: ReadState::of_counts(wrong, correct),
                    correct,
                    wrong,
                }
            })
            .collect())
    }

    /// The verdict `user` gave on each card, by oracle id: the newest on the key's hash, else the
    /// newest on any other hash of the card (`current` is false then). A card the account never judged
    /// is left out. `drafts` holds one key per card, normally its current draft's.
    pub async fn of_account(
        &self,
        user: Uuid,
        drafts: &[DraftKey],
    ) -> Result<Vec<AccountRead>, DbError> {
        let (ids, hashes) = unzip_keys(drafts);
        let rows = sqlx::query!(
            r#"SELECT DISTINCT ON (r.oracle_id)
                      r.oracle_id, r.read_hash, r.verdict AS "verdict: Verdict", r.note, r.created_at,
                      (r.read_hash = k.read_hash) AS "current!"
               FROM card_reads r
               JOIN unnest($2::uuid[], $3::text[]) AS k(oracle_id, read_hash)
                 ON r.oracle_id = k.oracle_id
               WHERE r.user_id = $1
               ORDER BY r.oracle_id, (r.read_hash = k.read_hash) DESC, r.id DESC"#,
            user,
            &ids,
            &hashes
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| AccountRead {
                oracle_id: OracleId(row.oracle_id.to_string()),
                read_hash: row.read_hash,
                verdict: row.verdict,
                note: row.note,
                created_at: row.created_at,
                current: row.current,
            })
            .collect())
    }

    /// What `user` has left to read among these cards (a deck's, in deck order): the cards whose
    /// current draft is pending, then those approved that this account has not judged on the current
    /// hash. A card that is `wrong` or has no current draft, and one `user` judged, is not in it.
    pub async fn queue(&self, user: Uuid, cards: &[OracleId]) -> Result<Vec<DraftRead>, DbError> {
        let states = self.state_of(cards).await?;
        let keys: Vec<DraftKey> = states
            .iter()
            .map(|state| DraftKey {
                oracle_id: state.oracle_id.clone(),
                read_hash: state.read_hash.clone(),
            })
            .collect();
        let judged = self
            .of_account(user, &keys)
            .await?
            .into_iter()
            .filter(|read| read.current)
            .map(|read| read.oracle_id)
            .collect();
        Ok(queue_order(cards, states, &judged))
    }

    /// Every verdict ever given, by card name, account (the server's first) and time; with
    /// `only_wrong`, the `wrong` ones.
    pub async fn list(&self, only_wrong: bool) -> Result<Vec<StoredRead>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT r.id, r.user_id, r.oracle_id, c.name, r.read_hash,
                      r.verdict AS "verdict: Verdict", r.note, r.created_at
               FROM card_reads r JOIN cards c ON c.oracle_id = r.oracle_id
               WHERE (NOT $1 OR r.verdict = 'wrong')
               ORDER BY c.name, r.oracle_id, r.user_id NULLS FIRST, r.id"#,
            only_wrong
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StoredRead {
                id: row.id,
                user_id: row.user_id,
                oracle_id: OracleId(row.oracle_id.to_string()),
                name: row.name,
                read_hash: row.read_hash,
                verdict: row.verdict,
                note: row.note,
                created_at: row.created_at,
            })
            .collect())
    }

    /// Deletes every verdict of `user`, inside the caller's transaction `tx`, and returns how many went;
    /// the drafts stay, and a card that account alone approved is pending again. A database whose read
    /// tables are not migrated yet (the server's accounts can run without card data) holds none: 0.
    pub async fn delete_all_of(tx: &mut PgConnection, user: Uuid) -> Result<u64, DbError> {
        let migrated =
            sqlx::query_scalar!(r#"SELECT to_regclass('card_reads') IS NOT NULL AS "migrated!""#)
                .fetch_one(&mut *tx)
                .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM card_reads WHERE user_id = $1", user)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: one wrong verdict wins over any number of correct ones, and a deck's queue lists the pending drafts first and then the approved ones its reader has not judged, in deck order and once.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0, 0, ReadState::Pending)]
    #[case(0, 1, ReadState::Approved)]
    #[case(0, 7, ReadState::Approved)]
    #[case(1, 0, ReadState::Wrong)]
    #[case(1, 5, ReadState::Wrong)]
    #[case(3, 3, ReadState::Wrong)]
    fn one_wrong_wins(#[case] wrong: u32, #[case] correct: u32, #[case] state: ReadState) {
        assert_eq!(ReadState::of_counts(wrong, correct), state);
    }

    #[test]
    fn counts_saturate() {
        assert_eq!(count_of(3), 3);
        assert_eq!(count_of(-1), u32::MAX);
    }

    fn draft(id: &str, state: ReadState) -> DraftRead {
        DraftRead {
            oracle_id: OracleId(id.into()),
            read_hash: "0".repeat(32),
            state,
            correct: 0,
            wrong: 0,
        }
    }

    fn ids(queue: &[DraftRead]) -> Vec<&str> {
        queue.iter().map(|read| read.oracle_id.as_str()).collect()
    }

    #[test]
    fn the_queue_is_pending_first_then_unjudged_approved_in_deck_order() {
        let states = vec![
            draft("a", ReadState::Approved),
            draft("b", ReadState::Pending),
            draft("c", ReadState::Wrong),
            draft("d", ReadState::Approved),
            draft("e", ReadState::Pending),
            draft("f", ReadState::Approved),
        ];
        let judged: HashSet<OracleId> = [OracleId("d".into())].into();
        let deck: Vec<OracleId> = ["f", "e", "z", "d", "c", "b", "a", "e", "f"]
            .into_iter()
            .map(|id| OracleId(id.into()))
            .collect();
        let queue = queue_order(&deck, states, &judged);
        assert_eq!(ids(&queue), ["e", "b", "f", "a"]);
    }

    #[test]
    fn an_empty_deck_has_an_empty_queue() {
        let queue = queue_order(&[], vec![draft("a", ReadState::Pending)], &HashSet::new());
        assert_eq!(queue, vec![]);
    }
}
