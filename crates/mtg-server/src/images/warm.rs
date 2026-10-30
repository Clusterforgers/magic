//! The warm-up: when a game starts, the pictures of the cards the seats play are fetched in the
//! background, so the first turns do not wait for the CDN.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_db::{CardRepo, DbError, PrintingImages};
use mtg_import::ids::{OracleId, PrintingId};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::resolve::{Size, link_of};
use super::{ImageLink, Images};
use crate::recipe::CardArt;

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

    /// Fetches the `normal` picture of every face of these cards, in the printing a deck line
    /// chose or else the newest paper one, in a task of its own: the caller never waits, and a
    /// failure is only logged.
    pub fn warm(&self, art: Vec<CardArt>) {
        let warmer = self.clone();
        tokio::spawn(async move { warmer.run(art).await });
    }

    /// The pictures of the printings that lines chose, by printing id; a printing the database
    /// does not know is left out.
    async fn chosen(&self, art: &[CardArt]) -> Result<BTreeMap<String, PrintingImages>, DbError> {
        let named: Vec<PrintingId> = art
            .iter()
            .filter_map(|card| card.printing.clone().map(PrintingId))
            .collect();
        let found = self.repo.printings(&named).await?;
        Ok(found
            .into_iter()
            .map(|printing| (printing.printing_id.to_string(), printing.images))
            .collect())
    }

    /// The pictures that stand for a card: its chosen printing's when the database has them, else
    /// the card's newest printing's.
    async fn pictures(
        &self,
        card: &CardArt,
        chosen: &BTreeMap<String, PrintingImages>,
    ) -> Result<Option<PrintingImages>, DbError> {
        let named = card.printing.as_ref().and_then(|id| chosen.get(id));
        if let Some(images) = named.filter(|images| link_of(images, 0, Size::Normal).is_some()) {
            return Ok(Some(images.clone()));
        }
        let newest = self
            .repo
            .picture_printing(&OracleId(card.oracle_id.clone()))
            .await?;
        Ok(newest.map(|printing| printing.images))
    }

    async fn run(&self, art: Vec<CardArt>) {
        let chosen = match self.chosen(&art).await {
            Ok(chosen) => chosen,
            Err(error) => {
                tracing::warn!(%error, "picture warm-up could not read the printings");
                return;
            }
        };
        let gate = Arc::new(Semaphore::new(CONCURRENCY));
        let mut fetches = JoinSet::new();
        let mut asked = 0usize;
        for card in &art {
            let printing = match self.pictures(card, &chosen).await {
                Ok(Some(printing)) => printing,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%error, "picture warm-up could not read a card");
                    return;
                }
            };
            let faces = printing.faces.len().max(1);
            for face in 0..faces {
                let Some(link) = link_of(&printing, face, Size::Normal)
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
