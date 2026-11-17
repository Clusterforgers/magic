//! Goal: a store double that works like the in-memory store until told to fail or to write slowly.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use mtg_engine::Event;
use mtg_server::{EventStore, GameId, MemoryStore, StoreError};

/// A [`MemoryStore`] whose writes can be switched off and on, or slowed down.
#[derive(Debug, Clone, Default)]
pub struct FlakyStore {
    inner: MemoryStore,
    failing: Arc<AtomicBool>,
    /// How long each append waits first, in milliseconds (a slow `COMMIT`).
    stall_ms: Arc<AtomicU64>,
}

impl FlakyStore {
    pub fn new() -> Self {
        FlakyStore::default()
    }

    /// While on, every append and rewind fails; loads keep working.
    pub fn fail(&self, on: bool) {
        self.failing.store(on, Ordering::SeqCst);
    }

    /// Makes every append wait `stall` before it writes.
    pub fn stall(&self, stall: Duration) {
        let ms = u64::try_from(stall.as_millis()).unwrap_or(u64::MAX);
        self.stall_ms.store(ms, Ordering::SeqCst);
    }

    /// The underlying store, for reading back what was saved.
    pub fn inner(&self) -> &MemoryStore {
        &self.inner
    }
}

impl EventStore for FlakyStore {
    async fn append(
        &self,
        game: GameId,
        from_seq: u64,
        events: &[Event],
    ) -> Result<(), StoreError> {
        let stall = self.stall_ms.load(Ordering::SeqCst);
        if stall > 0 {
            tokio::time::sleep(Duration::from_millis(stall)).await;
        }
        if self.failing.load(Ordering::SeqCst) {
            return Err(StoreError::Unavailable("injected failure".to_owned()));
        }
        self.inner.append(game, from_seq, events).await
    }

    async fn load(&self, game: GameId) -> Result<Vec<Event>, StoreError> {
        self.inner.load(game).await
    }

    async fn rewind(&self, game: GameId, keep: u64, events: &[Event]) -> Result<(), StoreError> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(StoreError::Unavailable("injected failure".to_owned()));
        }
        self.inner.rewind(game, keep, events).await
    }
}
