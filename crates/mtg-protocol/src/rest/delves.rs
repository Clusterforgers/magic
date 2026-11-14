//! Bodies of the Delves endpoints of `mtg-server` (`/api/delves`): creating and joining a run, the party's actions,
//! the run as the party sees it and the campaign content the web draws (custom cards, bosses, events).

use mtg_campaign::{Brain, CardKind, Pick, Run};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A new run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CreateDelve {
    /// The difficulty level, 0 to 2.
    pub heat: u8,
    /// A seed code to replay a known ladder; a random one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seed: Option<String>,
}

/// What a member of the party does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DelveAction {
    /// The host adds a bot ally.
    AddBot {
        /// Its brain.
        level: Brain,
    },
    /// The host removes a hero from the lobby.
    Remove {
        /// The hero.
        hero: u8,
    },
    /// A player chooses their deck, or the host one for a bot ally.
    ChooseDeck {
        /// The hero.
        hero: u8,
        /// The deck id: `precon:<code>`, a built-in deck or one of the caller's stored decks.
        deck: String,
    },
    /// The host leads the party into the ladder.
    Start,
    /// A vote for the next room.
    Vote {
        /// The room.
        node: u8,
    },
    /// An answer to the caller's first offer.
    Pick {
        /// The answer.
        pick: Pick,
    },
    /// The host ends the run.
    Abandon,
}

/// A run as a member sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveView {
    /// The run's id.
    pub id: String,
    /// The invite code friends join with.
    pub code: String,
    /// The seed code that replays the same ladder.
    pub seed: String,
    /// Whether the caller hosts the run.
    pub host: bool,
    /// The caller's hero, if they are in the party.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub me: Option<u8>,
    /// The run.
    pub run: Run,
    /// The boss of the room the party is in or votes towards, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub boss: Option<String>,
    /// The heroes whose decks the waiting fight cannot be made with; empty when nothing stops it.
    #[serde(default)]
    pub deck_problems: Vec<DelveDeckProblem>,
    /// The boss fight as its game is made, while the party is in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fight: Option<DelveFightView>,
}

/// The boss's side of a fight as the engine gets it: what the intro screen lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveFightView {
    /// The boss's name.
    pub boss: String,
    /// The boss's starting life, scaled for the party.
    pub life: i32,
    /// The custom cards in its command zone from the start, by id; one given twice is listed twice.
    pub rules: Vec<String>,
    /// Its phases by card id, each with the life at or below which it begins.
    pub phases: Vec<(String, i32)>,
    /// Its minions: name and starting life.
    pub minions: Vec<(String, i32)>,
}

/// A hero's deck that no longer plays, so the next fight waits until they choose another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveDeckProblem {
    /// The hero, by place in the party.
    pub hero: u8,
    /// Why the deck does not play.
    pub reason: String,
}

/// One run in the caller's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveSummary {
    /// The run's id.
    pub id: String,
    /// The invite code.
    pub code: String,
    /// The theme's name.
    pub theme: String,
    /// Where the run is: lobby, map, fight, room, victory, defeat or abandoned.
    pub status: String,
    /// The heroes' names.
    pub heroes: Vec<String>,
    /// Bosses beaten.
    pub cleared: u32,
    /// Bosses on the ladder.
    pub bosses: u32,
}

/// The caller's runs, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveList {
    /// The runs.
    pub delves: Vec<DelveSummary>,
}

/// A custom card as the web draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CustomCardView {
    /// Its content id.
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
    pub art: String,
    /// A relic's role.
    pub role: String,
    /// A relic's shop price.
    pub price: u32,
}

/// A boss as the web shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveBossView {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colours as WUBRG letters.
    pub colors: String,
    /// What it says as the fight begins.
    pub line: String,
    /// Its boss card's id.
    pub card: Option<String>,
    /// Its phase cards' ids, in order, with the share of life (percent) at which each begins.
    pub phases: Vec<(String, u32)>,
    /// Its life with one hero and for each hero more.
    pub life: (i32, i32),
}

/// An event as the web shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveEventView {
    /// Its id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its story.
    pub text: String,
    /// Its options' labels, in order.
    pub options: Vec<String>,
}

/// The campaign content the web draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveContent {
    /// The theme's name.
    pub theme: String,
    /// Its story.
    pub text: String,
    /// The custom cards.
    pub cards: Vec<CustomCardView>,
    /// The bosses.
    pub bosses: Vec<DelveBossView>,
    /// The events.
    pub events: Vec<DelveEventView>,
}
