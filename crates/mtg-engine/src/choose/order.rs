//! `OrderCards`: a player puts cards in an order (scry, cards put back on a library, "in any order").
//!
//! An effect asks through `Questions::order` (`ask.rs`). The answer lists every card offered once, in the order they
//! will lie, the card nearest the top of the library first.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;

/// The cards to put in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OrderOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The cards, in their current order.
    pub cards: Vec<ObjectId>,
    /// What the order is for, in the words of a card ("Put the cards on the bottom in any order."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// Every card once, in the order chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OrderChoice(pub Vec<ObjectId>);

/// Checks that `cards` are exactly the cards offered, each once.
pub(super) fn check(options: &OrderOptions, cards: &[ObjectId]) -> Result<(), IllegalAnswer> {
    for (at, card) in cards.iter().enumerate() {
        if cards[..at].contains(card) {
            return Err(IllegalAnswer::Duplicate("card"));
        }
        if !options.cards.contains(card) {
            return Err(IllegalAnswer::NotOffered("card"));
        }
    }
    if cards.len() == options.cards.len() {
        Ok(())
    } else {
        Err(IllegalAnswer::WrongCount {
            expected: options.cards.len(),
            got: cards.len(),
        })
    }
}

pub(crate) fn answer(
    player: PlayerId,
    options: &OrderOptions,
    choice: OrderChoice,
) -> Result<Outcome, EngineError> {
    check(options, &choice.0)?;
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Order(choice.0),
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to an order of cards is accepted exactly when it lists every card offered once and no other, in whatever order, and is logged as the chooser's answer.
    use rstest::rstest;

    use super::*;

    const FOE: PlayerId = PlayerId(1);

    fn ids(numbers: &[u64]) -> Vec<ObjectId> {
        numbers.iter().copied().map(ObjectId).collect()
    }

    fn options(cards: &[u64]) -> OrderOptions {
        OrderOptions {
            source: ObjectId(99),
            cards: ids(cards),
            text: "Put the cards back in any order.".into(),
        }
    }

    #[rstest]
    #[case::the_order_offered(&[1, 2, 3], &[1, 2, 3])]
    #[case::reversed(&[1, 2, 3], &[3, 2, 1])]
    #[case::shuffled(&[1, 2, 3, 4], &[3, 1, 4, 2])]
    #[case::one_card(&[7], &[7])]
    #[case::no_cards(&[], &[])]
    fn any_order_of_the_cards_offered_is_logged_as_the_choosers_choice(
        #[case] offered: &[u64],
        #[case] chosen: &[u64],
    ) {
        let outcome = answer(FOE, &options(offered), OrderChoice(ids(chosen))).unwrap();
        let Outcome::Events(events) = outcome else {
            panic!("an answer to a choice is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: FOE,
                choice: Choice::Order(ids(chosen))
            }]
        );
    }

    #[rstest]
    #[case::a_card_that_was_not_offered(&[1, 2, 3], &[1, 2, 4], IllegalAnswer::NotOffered("card"))]
    #[case::only_foreign_cards(&[1, 2], &[8, 9], IllegalAnswer::NotOffered("card"))]
    #[case::the_same_card_twice(&[1, 2, 3], &[1, 2, 2], IllegalAnswer::Duplicate("card"))]
    #[case::a_repeat_before_the_count_is_judged(&[1, 2, 3], &[1, 1], IllegalAnswer::Duplicate("card"))]
    #[case::too_few(&[1, 2, 3], &[2, 1], IllegalAnswer::WrongCount { expected: 3, got: 2 })]
    #[case::nothing_at_all(&[1, 2], &[], IllegalAnswer::WrongCount { expected: 2, got: 0 })]
    #[case::too_many(&[1, 2], &[1, 2, 3], IllegalAnswer::NotOffered("card"))]
    fn an_answer_that_is_not_every_card_once_is_refused_by_name(
        #[case] offered: &[u64],
        #[case] chosen: &[u64],
        #[case] refusal: IllegalAnswer,
    ) {
        let error = answer(FOE, &options(offered), OrderChoice(ids(chosen))).unwrap_err();
        assert!(
            matches!(&error, EngineError::Illegal(got) if *got == refusal),
            "{error:?}"
        );
    }
}
