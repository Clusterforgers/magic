//! Bodies of the Delves endpoints of `mtg-server` (`/api/delves`): creating and joining a run, the party's actions,
//! the run as the party sees it and the campaign content the web draws (custom cards, bosses, events).

use mtg_campaign::{Brain, CardKind, Needs, Outcome, Pick, Rarity, Run};
use mtg_core::house::Timers;
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
    /// How long the party may take in each fight's game; relaxed when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timers: Option<Timers>,
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
    /// On a cleared floor, a vote to descend to the next floor or bank the win.
    FloorVote {
        /// Whether to descend.
        descend: bool,
    },
    /// An answer to the caller's first offer.
    Pick {
        /// The answer.
        pick: Pick,
    },
    /// The host ends the run.
    Abandon,
    /// The host retreats from the boss fight: it counts as lost and is made again.
    Retreat,
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
    /// Each boss of the ladder by id with the life it starts its fight with against this party, at its rank and Heat.
    #[serde(default)]
    pub boss_lives: Vec<(String, i32)>,
    /// The heroes whose decks the waiting fight cannot be made with; empty when nothing stops it.
    #[serde(default)]
    pub deck_problems: Vec<DelveDeckProblem>,
    /// The boss fight as its game is made, while the party is in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fight: Option<DelveFightView>,
    /// On a cleared floor, what descending changes, one line each; empty otherwise.
    #[serde(default)]
    pub descent: Vec<String>,
    /// The cards the caller may copy at the Mirror Pool or give up for an event's cost while that is their first offer:
    /// the nonland cards of their deck (less those given up) and their Satchel, each name once, in name order; left out
    /// otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub mirror: Vec<String>,
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
    /// Its commander card's name: a real card it casts free as its last phase begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub commander: Option<String>,
    /// What the floor changes for it (Intensify), one line each; empty on the first floor.
    #[serde(default)]
    pub intensify: Vec<String>,
    /// Its minions: name and starting life.
    pub minions: Vec<(String, i32)>,
    /// A reworked boss's innate at its level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub innate: Option<crate::view::InnateView>,
    /// The fight's encounter modifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub modifier: Option<crate::view::BossPartView>,
    /// A reworked boss's two phases, each with the round it begins at the end of.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<crate::view::RoundPhaseView>>", optional)]
    pub round_phases: Vec<crate::view::RoundPhaseView>,
    /// The relics a reworked boss brings, copies included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<crate::view::BossPartView>>", optional)]
    pub relics: Vec<crate::view::BossPartView>,
    /// A reworked boss's Satchel, each card with its reusable mark.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<crate::view::BossSatchelView>>", optional)]
    pub satchel: Vec<crate::view::BossSatchelView>,
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
    /// Where the run is: lobby, map, fight, room, `floor_cleared`, victory, defeat or abandoned.
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

/// The Delves leaderboard: one board per Heat, the greatest Heat first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveLeaderboard {
    /// The boards, Heat 2 to 0.
    pub boards: Vec<DelveBoard>,
}

/// The deepest run of each player at one Heat, deepest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveBoard {
    /// The Heat the runs were started at, 0 to 2.
    pub heat: u8,
    /// One entry per player, ranked.
    pub entries: Vec<DelveBoardEntry>,
}

/// A player's deepest run at a Heat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveBoardEntry {
    /// The player's name in that run.
    pub player: String,
    /// The floors whose final boss fell.
    pub floors: u8,
    /// The bosses beaten on the floor below those.
    pub bosses: u8,
    /// The commanders of the player's deck, or its name for a deck chosen before commanders were read.
    pub commanders: Vec<String>,
    /// Heroes in the party, bot allies included.
    pub party: u8,
    /// The day the run last changed, `YYYY-MM-DD`.
    pub date: String,
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
    /// A relic's rarity.
    pub rarity: Rarity,
    /// The heroes' turn from which an Enrage applies; 0 for other cards.
    pub turn: u32,
}

/// A boss as the web shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveBossView {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The id of the boss an upgraded form came from; left out for a boss of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub upgrade_of: Option<String>,
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
    /// The real card whose art crop pictures it.
    pub art: BossArtView,
}

/// The real card whose art pictures a boss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BossArtView {
    /// The card's name.
    pub card: String,
    /// The card's Scryfall oracle id, which its picture goes by.
    pub oracle_id: String,
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
    /// Its options, in order.
    pub options: Vec<DelveOptionView>,
}

/// One option of an event: its words, what it does as data the web draws chips from, and what it asks of a hero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DelveOptionView {
    /// The choice in a few words.
    pub label: String,
    /// What it does, gambles with their odds.
    pub outcomes: Vec<Outcome>,
    /// The gold and life a hero needs to take it.
    pub needs: Needs,
    /// What it does in plain words, for a screen reader.
    pub summary: String,
    /// What it takes from the hero in plain words, "nothing" for a free option.
    pub cost: String,
    /// What it gives the hero in plain words.
    pub reward: String,
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
