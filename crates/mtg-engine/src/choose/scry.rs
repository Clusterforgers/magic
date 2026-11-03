//! `Scry`: a player splits the cards looked at into a top pile and a bottom pile and orders both in one answer.
//!
//! An effect asks through `Questions::scry` (`ask.rs`). Both piles list their cards in the order they will lie, the
//! card nearest the top of the library first.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;

/// The cards looked at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScryOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The cards, top of the library first.
    pub cards: Vec<ObjectId>,
    /// What the scry asks, in the words of a card; empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// Where each card goes: every card offered once, in one pile or the other, each pile top first.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScryChoice {
    /// The cards that stay on top of the library, in order.
    pub top: Vec<ObjectId>,
    /// The cards that go to the bottom of the library, in order.
    pub bottom: Vec<ObjectId>,
}

/// Checks that the two piles together are exactly the cards offered, each once.
pub(crate) fn check(options: &ScryOptions, choice: &ScryChoice) -> Result<(), IllegalAnswer> {
    let all: Vec<ObjectId> = choice.top.iter().chain(&choice.bottom).copied().collect();
    for (at, card) in all.iter().enumerate() {
        if all[..at].contains(card) {
            return Err(IllegalAnswer::Duplicate("card"));
        }
        if !options.cards.contains(card) {
            return Err(IllegalAnswer::NotOffered("card"));
        }
    }
    if all.len() == options.cards.len() {
        Ok(())
    } else {
        Err(IllegalAnswer::WrongCount {
            expected: options.cards.len(),
            got: all.len(),
        })
    }
}

pub(crate) fn answer(
    player: PlayerId,
    options: &ScryOptions,
    choice: ScryChoice,
) -> Result<Outcome, EngineError> {
    check(options, &choice)?;
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Scry(choice),
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a scry is accepted exactly when its two piles together list every card offered once and no other, whatever the split and the orders, and is logged as the chooser's answer.
    use rstest::rstest;

    use super::*;

    fn ids(numbers: &[u64]) -> Vec<ObjectId> {
        numbers.iter().copied().map(ObjectId).collect()
    }

    fn options(cards: &[u64]) -> ScryOptions {
        ScryOptions {
            source: ObjectId(99),
            cards: ids(cards),
            text: String::new(),
        }
    }

    fn piles(top: &[u64], bottom: &[u64]) -> ScryChoice {
        ScryChoice {
            top: ids(top),
            bottom: ids(bottom),
        }
    }

    #[rstest]
    #[case::all_on_top(&[1, 2, 3], &[1, 2, 3], &[])]
    #[case::all_on_top_reordered(&[1, 2, 3], &[3, 1, 2], &[])]
    #[case::all_on_the_bottom(&[1, 2, 3], &[], &[2, 3, 1])]
    #[case::mixed(&[1, 2, 3, 4], &[4, 1], &[3, 2])]
    #[case::one_card(&[7], &[], &[7])]
    fn any_split_and_orders_of_the_cards_offered_are_logged(
        #[case] offered: &[u64],
        #[case] top: &[u64],
        #[case] bottom: &[u64],
    ) {
        let outcome = answer(PlayerId(1), &options(offered), piles(top, bottom)).unwrap();
        let Outcome::Events(events) = outcome else {
            panic!("an answer to a choice is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: PlayerId(1),
                choice: Choice::Scry(piles(top, bottom))
            }]
        );
    }

    #[rstest]
    #[case::foreign(&[1, 2], &[1], &[3], IllegalAnswer::NotOffered("card"))]
    #[case::twice_in_a_pile(&[1, 2], &[1, 1], &[2], IllegalAnswer::Duplicate("card"))]
    #[case::in_both_piles(&[1, 2], &[1, 2], &[2], IllegalAnswer::Duplicate("card"))]
    #[case::missing(&[1, 2, 3], &[1], &[2], IllegalAnswer::WrongCount { expected: 3, got: 2 })]
    #[case::nothing(&[1, 2], &[], &[], IllegalAnswer::WrongCount { expected: 2, got: 0 })]
    fn piles_that_are_not_every_card_once_are_refused_by_name(
        #[case] offered: &[u64],
        #[case] top: &[u64],
        #[case] bottom: &[u64],
        #[case] refusal: IllegalAnswer,
    ) {
        let error = answer(PlayerId(1), &options(offered), piles(top, bottom)).unwrap_err();
        assert!(
            matches!(&error, EngineError::Illegal(got) if *got == refusal),
            "{error:?}"
        );
    }
}
