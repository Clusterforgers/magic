//! The manual action that gives a permanent an ability, a keyword the engine applies or a line of text for the table,
//! until end of turn or for good, as a continuous effect in the ability layer.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::events::Event;
use crate::manual::{MAX_NOTE_CHARS, ManualRefusal};
use crate::state::GameState;

/// A keyword the table can give by hand: one the engine applies with no parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManualKeyword {
    /// Flying.
    Flying,
    /// Reach.
    Reach,
    /// First strike.
    FirstStrike,
    /// Double strike.
    DoubleStrike,
    /// Deathtouch.
    Deathtouch,
    /// Trample.
    Trample,
    /// Lifelink.
    Lifelink,
    /// Vigilance.
    Vigilance,
    /// Haste.
    Haste,
    /// Menace.
    Menace,
    /// Defender.
    Defender,
    /// Indestructible.
    Indestructible,
    /// Hexproof.
    Hexproof,
    /// Shroud.
    Shroud,
    /// Flash.
    Flash,
}

impl ManualKeyword {
    /// The keyword the engine runs.
    pub fn keyword(self) -> Keyword {
        match self {
            ManualKeyword::Flying => Keyword::Flying,
            ManualKeyword::Reach => Keyword::Reach,
            ManualKeyword::FirstStrike => Keyword::FirstStrike,
            ManualKeyword::DoubleStrike => Keyword::DoubleStrike,
            ManualKeyword::Deathtouch => Keyword::Deathtouch,
            ManualKeyword::Trample => Keyword::Trample,
            ManualKeyword::Lifelink => Keyword::Lifelink,
            ManualKeyword::Vigilance => Keyword::Vigilance,
            ManualKeyword::Haste => Keyword::Haste,
            ManualKeyword::Menace => Keyword::Menace,
            ManualKeyword::Defender => Keyword::Defender,
            ManualKeyword::Indestructible => Keyword::Indestructible,
            ManualKeyword::Hexproof => Keyword::Hexproof,
            ManualKeyword::Shroud => Keyword::Shroud,
            ManualKeyword::Flash => Keyword::Flash,
        }
    }
}

/// What a seat gives a permanent by hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManualGrant {
    /// A keyword the engine applies.
    Keyword(ManualKeyword),
    /// A line of text the table follows by hand, up to 80 characters on one line.
    Text(String),
}

/// The event of the permanent `object` getting `grant`, as an effect the seat controls that ends with the turn or never.
pub(crate) fn grant(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    grant: &ManualGrant,
    until_end_of_turn: bool,
) -> Result<Vec<Event>, ManualRefusal> {
    let permanent = crate::manual::permanent(state, object)?;
    let modification = match grant {
        ManualGrant::Keyword(keyword) => Modification::AddKeyword(keyword.keyword()),
        ManualGrant::Text(text) => {
            let text = text.trim();
            if text.is_empty()
                || text.chars().count() > MAX_NOTE_CHARS
                || text.chars().any(char::is_control)
            {
                return Err(ManualRefusal::OutOfRange("the text of the ability"));
            }
            Modification::GrantText(text.to_owned())
        }
    };
    let duration = if until_end_of_turn {
        Duration::UntilEndOfTurn
    } else {
        Duration::Forever
    };
    let effect = ContinuousEffect::new(
        EffectId(state.next_effect),
        Selector::Objects(vec![permanent.reference()]),
        modification,
        duration,
        state.next_timestamp,
    )
    .begun(player, state.turn().number());
    Ok(vec![Event::EffectCreated {
        effect: effect.into(),
    }])
}

/// The lines of text given to `object` by hand that still hold, oldest first.
pub fn granted_text(state: &GameState, object: ObjectId) -> Vec<String> {
    let Some(held) = state.object(object).map(|object| object.reference()) else {
        return Vec::new();
    };
    state
        .effects()
        .iter()
        .filter_map(|effect| match (&effect.modification, &effect.selector) {
            (Modification::GrantText(text), Selector::Objects(named)) if named.contains(&held) => {
                Some(text.clone())
            }
            _ => None,
        })
        .collect()
}
