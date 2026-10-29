//! The warm-up: when a game starts, the pictures of the cards the seats play are fetched in the
//! background, so the first turns do not wait for the CDN.

use std::sync::Arc;

use mtg_db::CardRepo;
use mtg_import::ids::OracleId;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::resolve::{Size, link_of};
use super::{ImageLink, Images};

/// Pictures fetched at once; the upstream spacing applies on top.
const CONCURRENCY: usize = 2;

/// Starts fetching the pictures of a game's cards. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Warmer {
    images: Images,
    repo: CardRepo,
}

impl Warmer {
    /// A warmer that reads links from `repo` and fills `images`.
    pub fn new(images: Images, repo: CardRepo) -> Warmer {
        Warmer { images, repo }
    }

    /// Fetches the `normal` picture of every face of the cards with these oracle ids, in a task
    /// of its own: the caller never waits, and a failure is only logged.
    pub fn warm(&self, oracle_ids: Vec<String>) {
        let warmer = self.clone();
        tokio::spawn(async move { warmer.run(oracle_ids).await });
    }

    async fn run(&self, oracle_ids: Vec<String>) {
        let gate = Arc::new(Semaphore::new(CONCURRENCY));
        let mut fetches = JoinSet::new();
        let mut asked = 0usize;
        for id in oracle_ids {
            let printing = match self.repo.picture_printing(&OracleId(id)).await {
                Ok(Some(printing)) => printing,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%error, "picture warm-up could not read a card");
                    return;
                }
            };
            let faces = printing.images.faces.len().max(1);
            for face in 0..faces {
                let Some(link) = link_of(&printing.images, face, Size::Normal)
                    .and_then(|stored| ImageLink::parse(&stored).ok())
                else {
                    continue;
                };
                asked += 1;
                let (images, gate) = (self.images.clone(), gate.clone());
                fetches.spawn(async move {
                    let Ok(_permit) = gate.acquire_owned().await else {
                        return false;
                    };
                    match images.get(&link).await {
                        Ok(_) => true,
                        Err(error) => {
                            tracing::debug!(%error, "picture warm-up could not fetch a picture");
                            false
                        }
                    }
                });
            }
        }
        let mut failed = 0usize;
        while let Some(done) = fetches.join_next().await {
            if !matches!(done, Ok(true)) {
                failed += 1;
            }
        }
        tracing::info!(pictures = asked, failed, "picture warm-up finished");
    }
}
