//! The stored cards a run is restricted to (`--stored`), and what the compiler and the corpus already make of each.

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::builtin;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::{Library, slug};
use mtg_cards::support::{Caps, PrintedCard, support};
use mtg_db::Database;

use crate::audit::stored_cards;
use crate::compile::names_of;
use crate::error::CliError;

/// The stored cards by slug, with what is needed to tell what the rules of the corpus and the compiler already give.
pub struct Stored {
    cards: BTreeMap<String, PrintedCard>,
    library: &'static Library,
    names: BTreeSet<String>,
    caps: Caps,
}

impl Stored {
    /// Reads the stored cards.
    pub async fn load(db: &Database) -> Result<Stored, CliError> {
        let library = builtin::library().map_err(|errors| {
            CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
        })?;
        let printed = stored_cards(db).await?;
        let names = names_of(&printed);
        let cards = printed
            .into_iter()
            .map(|card| (slug(&card.name), card))
            .collect();
        Ok(Stored {
            cards,
            library,
            names,
            caps: mtg_engine::caps(),
        })
    }

    /// Whether the database stores a card with this slug.
    pub fn has(&self, slug: &str) -> bool {
        self.cards.contains_key(slug)
    }

    /// The oracle id of the stored card with this slug.
    pub fn oracle_id(&self, slug: &str) -> Option<&str> {
        self.cards.get(slug).map(|card| card.oracle_id.as_str())
    }

    /// What the corpus or the compiler makes of the card today: `corpus-verified`, `compiled-verified`, `partial`,
    /// `manual` and the like, the words of the support states.
    pub fn today(&self, slug: &str) -> String {
        let Some(printed) = self.cards.get(slug) else {
            return "unknown".to_owned();
        };
        if let Some(card) = self.library.card(&printed.oracle_id) {
            let state = support(Some(card), printed, &self.caps).support;
            return format!("corpus-{state:?}").to_lowercase();
        }
        match generate(printed, self.library, &self.names) {
            Generation::Rules(card) => {
                let state = support(Some(&card), printed, &self.caps).support;
                format!("compiled-{state:?}").to_lowercase()
            }
            Generation::Nothing => "none".to_owned(),
            Generation::Invalid(_) => "invalid".to_owned(),
        }
    }
}
