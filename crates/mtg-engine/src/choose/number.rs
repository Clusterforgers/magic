//! `ChooseNumber`: the value of X as a spell or ability is announced (CR 601.2b), or "any number" as an effect
//! resolves.
//!
//! The announcement keeps the number in `Choices.x` (CR 107.3); a resolution logs it as `ChoiceMade` and reads it back
//! from the journal like any other answer.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;

/// The range of numbers a player may pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NumberOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The smallest allowed number.
    pub min: u32,
    /// The largest allowed number; no limit when absent.
    pub max: Option<u32>,
    /// Numbers between `min` and `max` that are not allowed (an X with no legal target); empty when every one is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<u32>>", optional))]
    pub except: Vec<u32>,
    /// What the number is for in the words of a card ("Choose a value for X."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub can_cancel: bool,
    /// The total mana cost with X at 0, when the number is the X of a mana cost; left out otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cost: Option<ManaCost>,
    /// The generic mana each 1 of X adds to `cost` (2 for `{X}{X}`); left out while 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub per_x: u32,
}

// serde's `skip_serializing_if` passes the field by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// The answer to a number decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum NumberChoice {
    /// This number, from `min` to `max`.
    Number(u32),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

/// Checks the answer against the options it answers. A number asked while a spell or ability is announced is kept
/// in its choices; one asked by an effect is logged for the step that asked.
pub(crate) fn answer(
    player: PlayerId,
    options: &NumberOptions,
    choice: NumberChoice,
    announcing: bool,
) -> Result<Outcome, EngineError> {
    let number = match choice {
        NumberChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        NumberChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        NumberChoice::Number(number) => number,
    };
    if number < options.min
        || options.max.is_some_and(|max| number > max)
        || options.except.contains(&number)
    {
        return Err(IllegalAnswer::NotOffered("number").into());
    }
    Ok(if announcing {
        Outcome::Number(number)
    } else {
        Outcome::Events(vec![Event::ChoiceMade {
            player,
            choice: Choice::Number(number),
        }])
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a number is accepted exactly when it lies between the bounds, with no upper bound when there is none, and backing out follows `can_cancel`; an announced number is kept for the cast and a number asked by an effect is logged.
    use rstest::rstest;

    use super::*;

    fn options(min: u32, max: Option<u32>, can_cancel: bool) -> NumberOptions {
        NumberOptions {
            source: ObjectId(5),
            min,
            max,
            except: Vec::new(),
            text: String::new(),
            can_cancel,
            cost: None,
            per_x: 0,
        }
    }

    #[rstest]
    #[case::the_lowest(0, Some(4), 0, true)]
    #[case::the_highest(0, Some(4), 4, true)]
    #[case::inside(1, Some(4), 3, true)]
    #[case::below_the_lowest(1, Some(4), 0, false)]
    #[case::above_the_highest(0, Some(4), 5, false)]
    #[case::no_limit_above(0, None, 4_000_000, true)]
    #[case::no_limit_but_the_lowest(2, None, 1, false)]
    fn a_number_is_accepted_only_inside_the_bounds(
        #[case] min: u32,
        #[case] max: Option<u32>,
        #[case] picked: u32,
        #[case] accepted: bool,
    ) {
        let result = answer(
            PlayerId(0),
            &options(min, max, true),
            NumberChoice::Number(picked),
            true,
        );
        match (accepted, result) {
            (true, Ok(Outcome::Number(taken))) => assert_eq!(taken, picked),
            (false, Err(EngineError::Illegal(IllegalAnswer::NotOffered("number")))) => {}
            (_, other) => panic!("{picked} within {min}..={max:?}: {other:?}"),
        }
    }

    #[rstest]
    #[case::below_the_gap(1, true)]
    #[case::an_excepted_number(2, false)]
    #[case::another_excepted_number(3, false)]
    #[case::above_the_gap(4, true)]
    #[case::the_highest(5, true)]
    fn a_number_that_is_excepted_is_refused(#[case] picked: u32, #[case] accepted: bool) {
        let mut offered = options(1, Some(5), true);
        offered.except = vec![2, 3];
        let result = answer(PlayerId(0), &offered, NumberChoice::Number(picked), true);
        assert_eq!(result.is_ok(), accepted, "{picked}: {result:?}");
    }

    #[rstest]
    #[case::before_the_payment(true)]
    #[case::for_a_trigger(false)]
    fn backing_out_follows_the_flag(#[case] can_cancel: bool) {
        let result = answer(
            PlayerId(0),
            &options(0, None, can_cancel),
            NumberChoice::Cancel,
            true,
        );
        let expected = if can_cancel {
            Ok(())
        } else {
            Err(EngineError::Illegal(IllegalAnswer::NotOffered("cancel")))
        };
        assert_eq!(
            result.map(|outcome| assert!(matches!(outcome, Outcome::CancelCast))),
            expected
        );
    }

    #[test]
    fn a_number_an_effect_asked_for_is_logged_for_its_step() {
        let Ok(Outcome::Events(events)) = answer(
            PlayerId(1),
            &options(0, Some(3), false),
            NumberChoice::Number(2),
            false,
        ) else {
            panic!("a number is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: PlayerId(1),
                choice: Choice::Number(2)
            }]
        );
    }
}
