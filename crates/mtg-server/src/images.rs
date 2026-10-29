//! Card pictures: `GET /api/images/...` answers a card identity's picture from a disk cache that
//! fills from the Scryfall CDN as pictures are asked for (`docs/server.md`, Pictures).
//!
//! A picture is shown as Scryfall serves it: never cropped, re-encoded, resized or marked.

mod api;
mod cache;
mod config;
mod link;
mod resolve;
mod upstream;
mod warm;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::Bytes;
use thiserror::Error;
use tokio::sync::OnceCell;

pub(crate) use api::{by_oracle, by_printing, by_token};
pub use config::{DEFAULT_CACHE_MIB, ImageConfig, MAX_CACHE_MIB, MIN_GAP};
pub use link::{ImageLink, LinkError};
pub use upstream::FetchError;
pub(crate) use warm::Warmer;

use cache::DiskCache;
use upstream::Upstream;

/// Why a picture could not be had.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageError {
    /// The stored link is not one to fetch.
    #[error("the picture's link is refused: {0}")]
    Link(#[from] LinkError),
    /// The CDN did not give the picture.
    #[error("the picture could not be fetched: {0}")]
    Fetch(#[from] FetchError),
}

type Flight = Arc<OnceCell<Result<Bytes, FetchError>>>;

struct Inner {
    cache: DiskCache,
    upstream: Upstream,
    /// The fetches under way by cache key: requests for one picture share one fetch.
    flights: Mutex<HashMap<String, Flight>>,
}

/// The picture cache with its upstream. Cheap to clone.
#[derive(Clone)]
pub struct Images {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Images {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Images").finish_non_exhaustive()
    }
}

impl Images {
    /// A cache in the directory of `config`, read when first used, over an upstream that
    /// connects when first asked.
    pub fn new(config: &ImageConfig) -> Images {
        Images {
            inner: Arc::new(Inner {
                cache: DiskCache::new(config.cache_dir.clone(), config.max_bytes),
                upstream: Upstream::new(config),
                flights: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Whether the picture is in the cache now.
    pub async fn is_cached(&self, link: &ImageLink) -> bool {
        self.inner.cache.contains(link.key()).await
    }

    /// The bytes of the picture: from the cache, else from the CDN, and then kept. Requests that
    /// arrive while the CDN is asked wait for that one answer, failure included.
    pub async fn get(&self, link: &ImageLink) -> Result<Bytes, ImageError> {
        if let Some(bytes) = self.inner.cache.get(link.key()).await {
            return Ok(Bytes::from(bytes));
        }
        let flight = self
            .flights()
            .entry(link.key().to_owned())
            .or_default()
            .clone();
        let outcome = flight
            .get_or_init(|| async {
                // A request that came in just after another one stored the picture needs no fetch.
                if let Some(bytes) = self.inner.cache.get(link.key()).await {
                    return Ok(Bytes::from(bytes));
                }
                let bytes = self.inner.upstream.fetch(link).await?;
                if let Err(error) = self.inner.cache.put(link.key(), &bytes).await {
                    tracing::warn!(%error, key = link.key(), "could not keep a picture in the cache");
                }
                Ok(Bytes::from(bytes))
            })
            .await
            .clone();
        let mut flights = self.flights();
        if flights
            .get(link.key())
            .is_some_and(|current| Arc::ptr_eq(current, &flight))
        {
            flights.remove(link.key());
        }
        Ok(outcome?)
    }

    fn flights(&self) -> std::sync::MutexGuard<'_, HashMap<String, Flight>> {
        // Nothing here can leave the map half-updated, so a poisoned lock is still sound.
        self.inner
            .flights
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
