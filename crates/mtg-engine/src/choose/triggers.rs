//! `OrderTriggers`: the order in which a player puts their triggered abilities on the stack (CR 603.3b).
//!
//! It is asked when a player has two or more triggered abilities to put on the stack together and the order can
//! matter; abilities of one source and one ability do the same in any order, and are never asked about
//! (`docs/cards-dsl.md`, Triggers).

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::object::ObjectRef;

/// One triggered ability waiting for the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggerEntry {
    /// The object that triggered.
    pub source: ObjectRef,
    /// Its ability, by index among the source's rules.
    pub ability: usize,
    /// The event it waits for in the words of a card: "When it enters the battlefield".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub when: String,
}

/// The triggers to order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggersOptions {
    /// The triggers of one controller, in the order they triggered.
    pub triggers: Vec<TriggerEntry>,
}

/// Every trigger once, by index, in the order they go on the stack (the last is on top).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggersChoice(pub Vec<usize>);

/// Checks that the answer names every trigger once.
pub(crate) fn answer(
    player: PlayerId,
    options: &TriggersOptions,
    choice: TriggersChoice,
) -> Result<Outcome, EngineError> {
    let order = choice.0;
    if order.len() != options.triggers.len() {
        return Err(IllegalAnswer::WrongCount {
            expected: options.triggers.len(),
            got: order.len(),
        }
        .into());
    }
    for (at, trigger) in order.iter().enumerate() {
        if *trigger >= options.triggers.len() {
            return Err(IllegalAnswer::NotOffered("trigger").into());
        }
        if order[..at].contains(trigger) {
            return Err(IllegalAnswer::Duplicate("trigger").into());
        }
    }
    Ok(Outcome::Events(vec![Event::TriggersOrdered {
        player,
        order,
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to the trigger order is accepted exactly when it names every trigger once, and then it is the event that puts the player's triggers in that order.
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;

    fn options(count: usize) -> TriggersOptions {
        TriggersOptions {
            triggers: (0..count)
                .map(|n| TriggerEntry {
                    source: ObjectRef {
                        id: ObjectId(n as u64),
                        counter: 0,
                    },
                    ability: 0,
                    when: String::new(),
                })
                .collect(),
        }
    }

    #[rstest]
    #[case::in_the_order_they_triggered(3, vec![0, 1, 2], true)]
    #[case::reversed(3, vec![2, 1, 0], true)]
    #[case::shuffled(3, vec![1, 2, 0], true)]
    #[case::two_of_two(2, vec![1, 0], true)]
    #[case::one_missing(3, vec![0, 1], false)]
    #[case::one_too_many(2, vec![0, 1, 0], false)]
    #[case::the_same_twice(3, vec![0, 0, 1], false)]
    #[case::one_that_does_not_exist(3, vec![0, 1, 3], false)]
    #[case::none(2, vec![], false)]
    fn an_order_names_each_trigger_once(
        #[case] count: usize,
        #[case] order: Vec<usize>,
        #[case] accepted: bool,
    ) {
        let result = answer(PlayerId(1), &options(count), TriggersChoice(order.clone()));
        match (accepted, result) {
            (true, Ok(Outcome::Events(events))) => assert_eq!(
                events,
                [Event::TriggersOrdered {
                    player: PlayerId(1),
                    order
                }]
            ),
            (false, Err(EngineError::Illegal(_))) => {}
            (_, other) => panic!("{other:?}"),
        }
    }
}
