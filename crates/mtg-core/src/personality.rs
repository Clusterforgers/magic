//! Boss personalities: how a Delves boss weighs the heroes' threat when it picks whom to eye, read by the spotlight
//! inside its cap (`docs/delves.md`, Spotlight).

use serde::{Deserialize, Serialize};

/// How a boss weighs each hero's threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Personality {
    /// Weighs the whole table evenly: the spotlight's plain presence.
    #[default]
    Watchful,
    /// Hunts the healer: the life a hero gained in the recent rounds counts.
    Spiteful,
    /// Hunts the biggest board: creature power counts twice, hands not at all.
    Brutish,
    /// Hunts the combo: noncreature permanents and cards in hand count most.
    Cunning,
}

/// What each part of a hero's table adds to their threat under a personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weights {
    /// Per point of creature power.
    pub power: i64,
    /// Per nonland permanent that is no creature.
    pub permanent: i64,
    /// Per card in hand.
    pub hand: i64,
    /// Per life gained in the spotlight's window.
    pub gained: i64,
}

impl Personality {
    /// Every personality, the default first.
    pub const ALL: [Personality; 4] = [
        Personality::Watchful,
        Personality::Spiteful,
        Personality::Brutish,
        Personality::Cunning,
    ];

    /// The weights of its threat reading.
    pub const fn weights(self) -> Weights {
        match self {
            Personality::Watchful => Weights {
                power: 1,
                permanent: 2,
                hand: 1,
                gained: 0,
            },
            Personality::Spiteful => Weights {
                power: 1,
                permanent: 2,
                hand: 1,
                gained: 2,
            },
            Personality::Brutish => Weights {
                power: 2,
                permanent: 1,
                hand: 0,
                gained: 0,
            },
            Personality::Cunning => Weights {
                power: 1,
                permanent: 4,
                hand: 2,
                gained: 0,
            },
        }
    }

    /// Its name as players read it.
    pub const fn name(self) -> &'static str {
        match self {
            Personality::Watchful => "Watchful",
            Personality::Spiteful => "Spiteful",
            Personality::Brutish => "Brutish",
            Personality::Cunning => "Cunning",
        }
    }

    /// Whom it hunts, in plain words.
    pub const fn text(self) -> &'static str {
        match self {
            Personality::Watchful => {
                "Eyes the hero with the most on the table: creature power, 2 for each other nonland permanent and the cards in hand."
            }
            Personality::Spiteful => {
                "Hunts the healer: eyes the hero with the most on the table, each life they gained in the last rounds counting 2 more."
            }
            Personality::Brutish => {
                "Hunts the biggest board: creature power counts twice, other nonland permanents 1 each and cards in hand not at all."
            }
            Personality::Cunning => {
                "Hunts the combo: each noncreature, nonland permanent counts 4 and each card in hand 2, creature power once."
            }
        }
    }

    /// Whether it is the default, which old fight plans and plain games carry.
    pub fn is_watchful(&self) -> bool {
        *self == Personality::Watchful
    }
}
