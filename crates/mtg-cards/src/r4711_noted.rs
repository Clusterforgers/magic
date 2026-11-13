//! What a booster draft notes for the cards of a name (Conspiracy). A game played without a draft has no notes, so the
//! rulings of these cards give every note its empty value: a noted number is 0, no name, type, color or player was
//! noted, nothing was removed from the draft or exiled before the game, and no guess was right.

use serde::{Deserialize, Serialize};

/// A note or choice made while drafting cards named like the card that reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DraftNote {
    /// "the highest number you noted for cards named ~" (0 without a draft).
    HighestNumber,
    /// "the number of cards you removed from the draft with cards named ~" (0 without a draft).
    CardsRemoved,
    /// "with a name you noted for cards named ~" (no name without a draft).
    NotedName,
    /// "with a name noted as you drafted cards named ~" (no name without a draft).
    NameNotedAsDrafted,
    /// "of a type you noted for cards named ~" (no creature type without a draft).
    NotedType,
    /// "one or more of the colors chosen as you drafted cards named ~" (no color without a draft).
    ChosenColors,
    /// "you guessed correctly for a card named ~" (no guess without a draft).
    GuessedRight,
    /// "a player you noted for cards named ~", read of each player as "you" (no player without a draft).
    NotedPlayer,
    /// "a card you exiled with cards named ~" before the game (no card without a draft).
    ExiledWith,
}

impl DraftNote {
    /// The words of the note, "~" standing for the card's name.
    pub const fn words(self) -> &'static str {
        match self {
            DraftNote::HighestNumber => "the highest number you noted for cards named ~",
            DraftNote::CardsRemoved => {
                "the number of cards you removed from the draft with cards named ~"
            }
            DraftNote::NotedName => "a name you noted for cards named ~",
            DraftNote::NameNotedAsDrafted => "a name noted as you drafted cards named ~",
            DraftNote::NotedType => "a type you noted for cards named ~",
            DraftNote::ChosenColors => {
                "one or more of the colors chosen as you drafted cards named ~"
            }
            DraftNote::GuessedRight => "you guessed correctly for a card named ~",
            DraftNote::NotedPlayer => "a player you noted for cards named ~",
            DraftNote::ExiledWith => "you exiled with cards named ~",
        }
    }

    /// Every note.
    pub const ALL: [DraftNote; 9] = [
        DraftNote::HighestNumber,
        DraftNote::CardsRemoved,
        DraftNote::NotedName,
        DraftNote::NameNotedAsDrafted,
        DraftNote::NotedType,
        DraftNote::ChosenColors,
        DraftNote::GuessedRight,
        DraftNote::NotedPlayer,
        DraftNote::ExiledWith,
    ];
}
