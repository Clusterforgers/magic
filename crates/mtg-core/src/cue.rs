//! Satchel cues: when a boss's bot casts a card of its Satchel, read from the campaign content by the bot brain.

use serde::{Deserialize, Serialize};

/// The moment a boss's bot casts one Satchel card; it holds the card at every other moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "when", rename_all = "snake_case", deny_unknown_fields)]
pub enum SatchelCue {
    /// In the seat's first `turns` turns.
    EarlyTurns {
        /// The last turn of the seat's that counts.
        turns: u32,
    },
    /// When the heroes' creatures outweigh the boss's side's in power by `power` or more.
    BehindOnBoard {
        /// The power the heroes are ahead by.
        power: u32,
    },
    /// When a hero creature has toughness `toughness` or less, so the card kills it.
    Kill {
        /// The highest toughness it kills.
        toughness: u32,
    },
    /// In the seat's turn before combat, when its creatures with `reach` more power each reach a hero's life.
    Lethal {
        /// The power the card adds to each attacker.
        reach: u32,
    },
    /// In answer to a hero's spell or ability that targets the seat's permanent, or a hero's sweeper.
    Protect,
    /// In answer to a hero's spell of mana value `mana_value` or more.
    Counter {
        /// The smallest mana value worth answering.
        mana_value: u32,
    },
    /// In the declare blockers step, while creatures fight.
    Combat,
    /// With `cards` or fewer cards in the seat's hand.
    HandLow {
        /// The most cards the hand holds.
        cards: u32,
    },
    /// In the end step of a hero's turn, with `mana` or more untapped mana sources.
    HeroesEnd {
        /// The fewest untapped mana sources.
        mana: u32,
    },
    /// With `mana` or more untapped mana sources.
    Mana {
        /// The fewest untapped mana sources.
        mana: u32,
    },
    /// When the hero the seat eyes controls a creature (`creature`) or any nonland permanent, other than a token.
    Eyed {
        /// Only a creature counts.
        creature: bool,
    },
    /// With `creatures` or more creature cards in the seat's graveyard.
    Graveyard {
        /// The fewest creature cards.
        creatures: u32,
    },
    /// With `creatures` or more creatures on the seat's battlefield.
    Board {
        /// The fewest creatures.
        creatures: u32,
    },
}

impl SatchelCue {
    /// Whether the cue answers something now (a spell, a combat or lethal), so a higher tax is still worth paying.
    pub fn urgent(self) -> bool {
        matches!(
            self,
            SatchelCue::Protect
                | SatchelCue::Counter { .. }
                | SatchelCue::Combat
                | SatchelCue::Lethal { .. }
        )
    }
}
