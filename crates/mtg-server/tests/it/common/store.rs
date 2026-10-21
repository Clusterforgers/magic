//! Goal: a store double that works like the in-memory store until told to fail.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mtg_engine::Event;
use mtg_server::{EventStore, GameId, MemoryStore, StoreError};

/// A [`MemoryStore`] whose writes can be switched off and on.
#[derive(Debug, Clone, Default)]
pub struct FlakyStore {
    inner: MemoryStore,
    failing: Arc<AtomicBool>,
}

impl FlakyStore {
    pub fn new() -> Self {
        FlakyStore::default()
    }

    /// While on, every append fails; loads keep working.
    pub fn fail(&self, on: bool) {
        self.failing.store(on, Ordering::SeqCst);
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
        if self.failing.load(Ordering::SeqCst) {
            return Err(StoreError::Unavailable("injected failure".to_owned()));
        }
        self.inner.append(game, from_seq, events).await
    }

    async fn load(&self, game: GameId) -> Result<Vec<Event>, StoreError> {
        self.inner.load(game).await
    }
}
