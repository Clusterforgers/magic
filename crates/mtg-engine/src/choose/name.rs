//! `ChooseName`: a player names a card (CR 201.3). Any card may be named, so the decision carries no list: the client
//! searches the whole catalog itself and the answer is the name as text. An empty name names nothing, which is the
//! answer of a seat that does not answer.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;

/// The longest card name an answer may hold (the longest printed name has well under half of it).
pub const MAX_NAME_LEN: usize = 160;

/// What the player names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// What is named, in the words of the card ("Choose a card name."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
    /// The card must not be a land ("choose a nonland card name"); the client offers only such cards.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub nonland: bool,
}

/// The answer to a name decision: the card name, or nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameChoice(pub String);

/// Whether the text can be a name: a single line of at most [`MAX_NAME_LEN`] characters.
pub fn is_name(text: &str) -> bool {
    text.chars().count() <= MAX_NAME_LEN && !text.chars().any(char::is_control)
}

/// Checks the answer; it is logged for the step that asked.
pub(crate) fn answer(player: PlayerId, choice: &NameChoice) -> Result<Outcome, EngineError> {
    let name = choice.0.trim().to_owned();
    if !is_name(&name) {
        return Err(IllegalAnswer::NotOffered("name").into());
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Name(name),
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: a name is accepted when it is one line within the length limit (empty for no name), is trimmed, and is logged as the chooser's answer; a control character or a name over the limit is refused.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::a_name("Llanowar Elves", Some("Llanowar Elves"))]
    #[case::trimmed("  Sol Ring ", Some("Sol Ring"))]
    #[case::nothing("", Some(""))]
    #[case::a_split_card("Fire // Ice", Some("Fire // Ice"))]
    #[case::a_new_line("Sol\nRing", None)]
    #[case::too_long(&"x".repeat(MAX_NAME_LEN + 1), None)]
    fn a_name_is_one_short_line(#[case] text: &str, #[case] kept: Option<&str>) {
        let done = answer(PlayerId(0), &NameChoice(text.to_owned()));
        match (done, kept) {
            (Ok(Outcome::Events(events)), Some(name)) => assert_eq!(
                events,
                [Event::ChoiceMade {
                    player: PlayerId(0),
                    choice: Choice::Name(name.to_owned()),
                }]
            ),
            (Err(_), None) => {}
            (other, _) => panic!("{other:?}"),
        }
    }
}
