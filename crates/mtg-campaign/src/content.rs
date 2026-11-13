//! The campaign's content: custom cards (relics, boss cards and phases, the Spirit's haunt) and the themes with their
//! bosses, events and reward pool, read from the JSON files of `content/`.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use serde::{Deserialize, Serialize};

use crate::CampaignError;

/// What a custom card is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CardKind {
    /// A boon a hero keeps for the run, in their command zone at every fight.
    Relic,
    /// A boss's card, in its command zone from the start.
    Boss,
    /// A boss phase, which begins at a life threshold.
    Phase,
    /// The emblem a hero gets on becoming a Spirit.
    Haunt,
}

/// A card that is not on Scryfall: its printed side and the abilities it has as an emblem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomCard {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it is for.
    pub kind: CardKind,
    /// Its type line.
    pub type_line: String,
    /// Its rules text.
    pub text: String,
    /// The key of its art placeholder.
    #[serde(default)]
    pub art: String,
    /// The team role a relic favours: Healer, Removal, Damage, Support, or Risky for one that helps the boss too.
    #[serde(default)]
    pub role: String,
    /// A relic's price at the shop; 0 for one the shop never sells.
    #[serde(default)]
    pub price: u32,
    /// Life its holder starts each fight with in addition.
    #[serde(default)]
    pub life: i32,
    /// Cards its holder's opening hand has in addition.
    #[serde(default)]
    pub hand: u32,
    /// Its abilities, in the card DSL.
    pub abilities: Vec<Ability>,
}

impl CustomCard {
    /// The card as an emblem the engine plays.
    pub fn definition(&self) -> Arc<CardDefinition> {
        Arc::new(mtg_engine::w6b_emblems::emblem_definition(
            &self.name,
            &self.abilities,
        ))
    }
}

/// How a boss seat is played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Brain {
    /// Random legal answers.
    Easy,
    /// The heuristic bot.
    #[default]
    Normal,
    /// The bot with a deeper search.
    Hard,
    /// The strongest bot.
    VeryHard,
}

impl Brain {
    /// One level stronger, Very Hard staying itself.
    #[must_use]
    pub fn stronger(self) -> Brain {
        match self {
            Brain::Easy => Brain::Normal,
            Brain::Normal => Brain::Hard,
            Brain::Hard | Brain::VeryHard => Brain::VeryHard,
        }
    }
}

/// A boss seat: a themed deck of real cards, a brain, life that grows with the party, its boss card and phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boss {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is the theme's final boss, fought last.
    #[serde(default, rename = "final")]
    pub final_boss: bool,
    /// Its colours, as WUBRG letters.
    pub colors: String,
    /// How its seat is played.
    pub brain: Brain,
    /// Its life with one hero.
    pub life: i32,
    /// Life it gets for each hero after the first.
    pub life_per_hero: i32,
    /// Its opening hand size.
    pub hand: u32,
    /// Its deck: card names and copies.
    pub deck: Vec<(String, u32)>,
    /// Custom cards it starts with in its command zone.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Its phases: a phase card and the share of its starting life, in percent, at or below which it begins.
    #[serde(default)]
    pub phases: Vec<(String, u32)>,
    /// What it says as the fight begins.
    #[serde(default)]
    pub line: String,
}

/// What an event option does to the hero who takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Gold gained (or lost, never below 0).
    Gold(i32),
    /// Life gained (up to the hero's maximum) or lost (never below 1).
    Life(i32),
    /// A relic drawn from those the hero lacks.
    Relic,
    /// A card pick in the hero's colours.
    CardPick,
    /// A chance: `win` with `odds` percent, `lose` otherwise.
    Gamble {
        /// The chance to win, in percent.
        odds: u32,
        /// What a win gives.
        win: Vec<Outcome>,
        /// What a loss gives.
        lose: Vec<Outcome>,
    },
}

/// One choice of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventOption {
    /// The choice in a few words.
    pub label: String,
    /// What it does.
    pub outcomes: Vec<Outcome>,
}

/// A story room: a short text and the choices each hero makes for themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSpec {
    /// Its id in the content.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its story text.
    pub text: String,
    /// Its choices.
    pub options: Vec<EventOption>,
}

/// A dungeon theme: its bosses, events and the cards its rewards offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its story text.
    pub text: String,
    /// Its bosses: the lieutenants in any order, then the final boss.
    pub bosses: Vec<Boss>,
    /// Its events.
    pub events: Vec<EventSpec>,
    /// The cards rewards offer: a name and its colours as WUBRG letters (empty for colourless).
    pub pool: Vec<(String, String)>,
}

impl Theme {
    /// The boss with this id.
    pub fn boss(&self, id: &str) -> Option<&Boss> {
        self.bosses.iter().find(|m| m.id == id)
    }

    /// The event with this id.
    pub fn event(&self, id: &str) -> Option<&EventSpec> {
        self.events.iter().find(|e| e.id == id)
    }
}

/// Everything the campaign is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    /// The custom cards by id.
    pub cards: BTreeMap<String, CustomCard>,
    /// The themes.
    pub themes: Vec<Theme>,
}

impl Content {
    /// Reads the content from its JSON texts.
    pub fn parse(cards: &str, themes: &[&str]) -> Result<Content, CampaignError> {
        let list: Vec<CustomCard> =
            serde_json::from_str(cards).map_err(|e| CampaignError::Content(e.to_string()))?;
        let cards = list.into_iter().map(|c| (c.id.clone(), c)).collect();
        let themes = themes
            .iter()
            .map(|text| {
                serde_json::from_str(text).map_err(|e| CampaignError::Content(e.to_string()))
            })
            .collect::<Result<Vec<Theme>, _>>()?;
        Ok(Content { cards, themes })
    }

    /// The custom card with this id.
    pub fn card(&self, id: &str) -> Result<&CustomCard, CampaignError> {
        self.cards
            .get(id)
            .ok_or_else(|| CampaignError::Unknown(format!("card {id}")))
    }

    /// The theme with this id.
    pub fn theme(&self, id: &str) -> Result<&Theme, CampaignError> {
        self.themes
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| CampaignError::Unknown(format!("theme {id}")))
    }

    /// The relics, in id order.
    pub fn relics(&self) -> Vec<&CustomCard> {
        self.cards
            .values()
            .filter(|c| c.kind == CardKind::Relic)
            .collect()
    }

    /// The emblem a hero gets on becoming a Spirit.
    pub fn haunt(&self) -> Option<&CustomCard> {
        self.cards.values().find(|c| c.kind == CardKind::Haunt)
    }
}

/// The content shipped with the crate, read once.
pub fn content() -> Result<&'static Content, CampaignError> {
    static CONTENT: OnceLock<Result<Content, CampaignError>> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            Content::parse(
                include_str!("../content/cards.json"),
                &[include_str!("../content/warrens.json")],
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}
