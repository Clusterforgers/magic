//! `DivideAmount`: how a spell or ability divides damage or counters among its targets (CR 601.2d).
//!
//! The division is kept in `Choices.division`, in the order of the targets. A single target gets the whole amount
//! without a question.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Target;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// An amount to divide and the targets that share it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DivideOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The amount to divide; all of it is assigned.
    pub total: u32,
    /// The targets, each of which gets at least one.
    pub among: Vec<Target>,
    /// What is divided in the words of a card ("Divide 3 damage among the targets."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for. A triggered
    /// ability that divides as it goes on the stack cannot (CR 603.3c).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub can_cancel: bool,
}

/// The answer to a division decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DivideChoice {
    /// What each target gets: every target of `among` once, at least one each, all of the total in sum.
    Division(Vec<(Target, u32)>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

/// Checks the answer against the options it answers; the shares come back in the order of `among`.
pub(crate) fn answer(
    _player: PlayerId,
    options: &DivideOptions,
    choice: DivideChoice,
) -> Result<Outcome, EngineError> {
    let shares = match choice {
        DivideChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        DivideChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        DivideChoice::Division(shares) => shares,
    };
    for (at, (target, share)) in shares.iter().enumerate() {
        if shares[..at].iter().any(|(earlier, _)| earlier == target) {
            return Err(IllegalAnswer::Duplicate("target").into());
        }
        if !options.among.contains(target) {
            return Err(IllegalAnswer::NotOffered("target").into());
        }
        if *share == 0 {
            return Err(IllegalAnswer::NotOffered("share of nothing").into());
        }
    }
    if shares.len() != options.among.len() {
        return Err(IllegalAnswer::WrongCount {
            expected: options.among.len(),
            got: shares.len(),
        }
        .into());
    }
    let got = shares
        .iter()
        .fold(0u32, |sum, (_, share)| sum.saturating_add(*share));
    if got != options.total {
        return Err(IllegalAnswer::WrongTotal {
            expected: options.total,
            got,
        }
        .into());
    }
    let ordered = options
        .among
        .iter()
        .filter_map(|target| shares.iter().find(|(named, _)| named == target).copied())
        .collect();
    Ok(Outcome::Division(ordered))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a division decision is accepted exactly when it names every target once with at least one each and adds up to the whole amount, or backs out of a cast that can be backed out of, and the shares come back in the order of the targets.
    use rstest::rstest;

    use super::*;

    const A: Target = Target::Player(PlayerId(0));
    const B: Target = Target::Player(PlayerId(1));
    const C: Target = Target::Player(PlayerId(2));

    fn options(total: u32, among: &[Target], can_cancel: bool) -> DivideOptions {
        DivideOptions {
            source: ObjectId(5),
            total,
            among: among.to_vec(),
            text: String::new(),
            can_cancel,
        }
    }

    #[rstest]
    #[case::an_even_split(3, &[(A, 1), (B, 2)], true)]
    #[case::in_another_order(3, &[(B, 2), (A, 1)], true)]
    #[case::all_to_one_of_one(3, &[(A, 3)], true)]
    fn a_division_that_covers_the_targets_and_the_total_is_accepted(
        #[case] total: u32,
        #[case] shares: &[(Target, u32)],
        #[case] accepted: bool,
    ) {
        let among: Vec<Target> = if shares.len() == 1 {
            vec![A]
        } else {
            vec![A, B]
        };
        let result = answer(
            PlayerId(0),
            &options(total, &among, true),
            DivideChoice::Division(shares.to_vec()),
        );
        assert_eq!(accepted, result.is_ok(), "{shares:?}: {result:?}");
        if let Ok(Outcome::Division(taken)) = result {
            let order: Vec<Target> = taken.iter().map(|(target, _)| *target).collect();
            assert_eq!(order, among, "the shares follow the targets");
        }
    }

    #[rstest]
    #[case::a_target_not_among_them(&[(A, 1), (C, 2)], IllegalAnswer::NotOffered("target"))]
    #[case::a_target_twice(&[(A, 1), (A, 2)], IllegalAnswer::Duplicate("target"))]
    #[case::a_share_of_nothing(&[(A, 0), (B, 3)], IllegalAnswer::NotOffered("share of nothing"))]
    #[case::a_target_left_out(&[(A, 3)], IllegalAnswer::WrongCount { expected: 2, got: 1 })]
    #[case::too_little(&[(A, 1), (B, 1)], IllegalAnswer::WrongTotal { expected: 3, got: 2 })]
    #[case::too_much(&[(A, 2), (B, 2)], IllegalAnswer::WrongTotal { expected: 3, got: 4 })]
    fn any_other_division_is_refused_by_name(
        #[case] shares: &[(Target, u32)],
        #[case] refusal: IllegalAnswer,
    ) {
        assert_eq!(
            answer(
                PlayerId(0),
                &options(3, &[A, B], true),
                DivideChoice::Division(shares.to_vec())
            )
            .unwrap_err(),
            EngineError::Illegal(refusal)
        );
    }

    #[rstest]
    #[case::before_the_payment(true)]
    #[case::for_a_trigger(false)]
    fn backing_out_follows_the_flag(#[case] can_cancel: bool) {
        let result = answer(
            PlayerId(0),
            &options(2, &[A, B], can_cancel),
            DivideChoice::Cancel,
        );
        assert_eq!(can_cancel, matches!(result, Ok(Outcome::CancelCast)));
    }
}
