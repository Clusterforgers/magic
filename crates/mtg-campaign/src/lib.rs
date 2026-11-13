//! Delves, the co-op dungeon campaign: content, the floor generator, the run as a fold of its events, the fight plans
//! that become normal games, and the bots' choices between fights. No IO, like the engine.

pub mod content;
pub mod fight;
pub mod map;
pub mod rng;
pub mod run;
pub mod scale;

pub use content::{Boss, Brain, CardKind, Content, CustomCard, Minion, content};
pub use fight::{BossSeat, FightPlan, HeroSeat, MinionSeat};
pub use map::{FloorMap, Node, RoomKind};
pub use run::{Hero, HeroDeck, Offer, Pick, Run, RunEvent, ShopItem, Status};
pub use scale::Scale;

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
