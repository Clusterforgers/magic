//! Where a game's event log lives, so an actor can be rebuilt after a restart.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mtg_engine::Event;
use thiserror::Error;

use crate::game_id::GameId;

/// Why a store operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    /// The append does not begin where the stored log ends.
    #[error("append starts at event {from_seq} but the stored log holds {stored}")]
    OutOfOrder {
        /// Position the caller wanted to append at.
        from_seq: u64,
        /// Events the store already holds.
        stored: u64,
    },
    /// The backing store could not be reached or refused the write.
    #[error("store unavailable: {0}")]
    Unavailable(String),
    /// An event could not be encoded, or the stored log has a gap or an event that does not decode.
    #[error("stored log is unusable: {0}")]
    Corrupt(String),
}

/// Append-only storage of each game's event log.
pub trait EventStore: Send + Sync + 'static {
    /// Appends `events` to the game's log; `from_seq` must equal the number of events already stored.
    fn append(
        &self,
        game: GameId,
        from_seq: u64,
        events: &[Event],
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Every stored event of the game in order; empty when the game has none.
    fn load(&self, game: GameId) -> impl Future<Output = Result<Vec<Event>, StoreError>> + Send;

    /// Cuts the game's log back to its first `keep` events and appends `events` after them, as one write; `keep` must
    /// not lie beyond the stored log.
    fn rewind(
        &self,
        game: GameId,
        keep: u64,
        events: &[Event],
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// A store that keeps every log in memory; clones share the same logs.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    logs: Arc<Mutex<HashMap<GameId, Vec<Event>>>>,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        MemoryStore::default()
    }

    fn logs(&self) -> MutexGuard<'_, HashMap<GameId, Vec<Event>>> {
        // A panic elsewhere cannot leave a log half-written, so a poisoned lock is still safe to use.
        self.logs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn append_now(&self, game: GameId, from_seq: u64, events: &[Event]) -> Result<(), StoreError> {
        let mut logs = self.logs();
        let stored = u64::try_from(logs.get(&game).map_or(0, Vec::len)).unwrap_or(u64::MAX);
        if stored != from_seq {
            return Err(StoreError::OutOfOrder { from_seq, stored });
        }
        if !events.is_empty() {
            logs.entry(game).or_default().extend_from_slice(events);
        }
        Ok(())
    }

    fn rewind_now(&self, game: GameId, keep: u64, events: &[Event]) -> Result<(), StoreError> {
        let mut logs = self.logs();
        let log = logs.entry(game).or_default();
        let stored = u64::try_from(log.len()).unwrap_or(u64::MAX);
        let kept = usize::try_from(keep)
            .ok()
            .filter(|_| keep <= stored)
            .ok_or(StoreError::OutOfOrder {
                from_seq: keep,
                stored,
            })?;
        log.truncate(kept);
        log.extend_from_slice(events);
        Ok(())
    }
}

// Memory never waits, so each call finishes before it returns and the futures are already complete.
impl EventStore for MemoryStore {
    fn append(
        &self,
        game: GameId,
        from_seq: u64,
        events: &[Event],
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.append_now(game, from_seq, events))
    }

    fn load(&self, game: GameId) -> impl Future<Output = Result<Vec<Event>, StoreError>> + Send {
        std::future::ready(Ok(self.logs().get(&game).cloned().unwrap_or_default()))
    }

    fn rewind(
        &self,
        game: GameId,
        keep: u64,
        events: &[Event],
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.rewind_now(game, keep, events))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the in-memory store keeps one gapless, append-only log per game and refuses writes that do not continue it.
    use super::*;
    use mtg_core::ids::PlayerId;
    use mtg_engine::Event;
    use rstest::rstest;
    use uuid::Uuid;

    fn game(n: u128) -> GameId {
        GameId(Uuid::from_u128(n))
    }

    fn events(n: u64) -> Vec<Event> {
        (0..n)
            .map(|i| Event::FirstPlayerChosen {
                player: PlayerId(i),
            })
            .collect()
    }

    #[rstest]
    #[case::to_the_start(0, true)]
    #[case::inside(3, true)]
    #[case::at_the_end(5, true)]
    #[case::beyond_the_end(6, false)]
    #[tokio::test]
    async fn a_rewind_cuts_the_log_and_continues_it(#[case] keep: u64, #[case] allowed: bool) {
        let store = MemoryStore::new();
        let all = events(5);
        store.append(game(1), 0, &all).await.unwrap();
        let more = events(7)[5..].to_vec();
        let result = store.rewind(game(1), keep, &more).await;
        assert_eq!(result.is_ok(), allowed);
        let mut expected = all.clone();
        if allowed {
            expected.truncate(usize::try_from(keep).unwrap());
            expected.extend(more);
        }
        assert_eq!(store.load(game(1)).await.unwrap(), expected);
    }

    #[tokio::test]
    async fn appends_accumulate_in_order() {
        let store = MemoryStore::new();
        let all = events(5);
        store.append(game(1), 0, &all[..2]).await.unwrap();
        store.append(game(1), 2, &all[2..]).await.unwrap();
        assert_eq!(store.load(game(1)).await.unwrap(), all);
    }

    #[tokio::test]
    async fn games_do_not_share_logs() {
        let store = MemoryStore::new();
        store.append(game(1), 0, &events(3)).await.unwrap();
        store.append(game(2), 0, &events(1)).await.unwrap();
        assert_eq!(store.load(game(1)).await.unwrap().len(), 3);
        assert_eq!(store.load(game(2)).await.unwrap().len(), 1);
        assert_eq!(store.load(game(3)).await.unwrap(), Vec::<Event>::new());
    }

    #[rstest]
    #[case::gap(5, 3)]
    #[case::overlap(1, 3)]
    #[case::rewrite(0, 3)]
    #[tokio::test]
    async fn an_append_that_does_not_continue_the_log_is_refused(
        #[case] from_seq: u64,
        #[case] stored: u64,
    ) {
        let store = MemoryStore::new();
        store.append(game(1), 0, &events(stored)).await.unwrap();
        let refused = store.append(game(1), from_seq, &events(2)).await;
        assert_eq!(refused, Err(StoreError::OutOfOrder { from_seq, stored }));
        assert_eq!(store.load(game(1)).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn clones_see_the_same_logs() {
        let store = MemoryStore::new();
        let other = store.clone();
        store.append(game(1), 0, &events(2)).await.unwrap();
        assert_eq!(other.load(game(1)).await.unwrap().len(), 2);
    }
}
