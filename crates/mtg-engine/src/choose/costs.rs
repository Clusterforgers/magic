//! `ChooseCosts`: which optional costs to pay as a spell is announced (kicker, buyback, "you may pay"; CR 601.2b).
//!
//! The costs chosen are kept in `Choices.optional_costs` and added to the total cost (CR 601.2f). Alternative costs
//! are not chosen here: each is a way of casting the spell of its own.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// One optional cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionalCost {
    /// Its index among the ability's optional costs.
    pub index: usize,
    /// The cost as the card prints it.
    pub label: String,
}

/// The optional costs a player may add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CostsOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The costs that may be paid.
    pub optional: Vec<OptionalCost>,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub can_cancel: bool,
}

/// The answer to a costs decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CostsChoice {
    /// These optional costs, by index, each once; none pays no optional cost.
    Costs(Vec<usize>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

/// Checks the answer against the options it answers; the costs it names come back in ascending order.
pub(crate) fn answer(
    _player: PlayerId,
    options: &CostsOptions,
    choice: CostsChoice,
) -> Result<Outcome, EngineError> {
    let mut costs = match choice {
        CostsChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        CostsChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        CostsChoice::Costs(costs) => costs,
    };
    for (at, cost) in costs.iter().enumerate() {
        if costs[..at].contains(cost) {
            return Err(IllegalAnswer::Duplicate("cost").into());
        }
        if !options
            .optional
            .iter()
            .any(|offered| offered.index == *cost)
        {
            return Err(IllegalAnswer::NotOffered("cost").into());
        }
    }
    costs.sort_unstable();
    Ok(Outcome::Costs(costs))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a costs decision is accepted exactly when it names offered costs, each once (none is allowed), or backs out of a cast that can be backed out of, and the costs come back in ascending order.
    use rstest::rstest;

    use super::*;

    fn options(indices: &[usize], can_cancel: bool) -> CostsOptions {
        CostsOptions {
            source: ObjectId(5),
            optional: indices
                .iter()
                .map(|index| OptionalCost {
                    index: *index,
                    label: format!("Kicker {index}"),
                })
                .collect(),
            can_cancel,
        }
    }

    #[rstest]
    #[case::none(&[], Some(vec![]))]
    #[case::one(&[1], Some(vec![1]))]
    #[case::both_in_any_order(&[2, 0], Some(vec![0, 2]))]
    #[case::one_not_offered(&[1, 5], None)]
    #[case::one_twice(&[0, 0], None)]
    fn costs_are_accepted_when_offered_and_not_repeated(
        #[case] picked: &[usize],
        #[case] expected: Option<Vec<usize>>,
    ) {
        let result = answer(
            PlayerId(0),
            &options(&[0, 1, 2], true),
            CostsChoice::Costs(picked.to_vec()),
        );
        match (expected, result) {
            (Some(costs), Ok(Outcome::Costs(taken))) => assert_eq!(taken, costs),
            (None, Err(EngineError::Illegal(_))) => {}
            (expected, other) => panic!("{picked:?}: expected {expected:?}, got {other:?}"),
        }
    }

    #[rstest]
    #[case::before_the_payment(true)]
    #[case::for_a_trigger(false)]
    fn backing_out_follows_the_flag(#[case] can_cancel: bool) {
        let result = answer(PlayerId(0), &options(&[0], can_cancel), CostsChoice::Cancel);
        assert_eq!(can_cancel, matches!(result, Ok(Outcome::CancelCast)));
    }
}
