//! Delves, the co-op dungeon campaign: content, the floor generator, the run as a fold of its events, the fight plans
//! that become normal games, and the bots' choices between fights. No IO, like the engine.

pub mod boss_plan;
pub mod boss_relics;
pub mod content;
pub mod fight;
pub mod innate;
pub mod map;
pub mod modifiers;
pub mod relics;
pub mod rng;
pub mod run;
pub mod scale;
pub mod sim;
pub mod upgrade;

pub use content::{
    Boss, BossArt, Brain, CardKind, Content, CustomCard, DeckFile, EventOption, Needs, Outcome,
    Rarity, content,
};
pub use fight::{BossSeat, FightPlan, HeroSeat};
pub use map::{FloorMap, Node, RoomKind};
pub use relics::{DeckMakeup, Want};
pub use run::{
    Forfeit, GambleRoll, Hero, HeroDeck, Offer, Pick, Resolved, Run, RunEvent, ShopItem, Status,
};
pub use scale::Scale;
pub use upgrade::{Bane, Boon, CommanderFacts, Gift, Upgrade};

/// Why the campaign refused something.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CampaignError {
    /// The content files do not read.
    #[error("campaign content: {0}")]
    Content(String),
    /// A content id that does not exist.
    #[error("unknown {0}")]
    Unknown(String),
    /// An event the run cannot take now.
    #[error("{0}")]
    Refused(String),
}
