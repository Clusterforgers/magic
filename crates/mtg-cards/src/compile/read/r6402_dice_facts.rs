//! The results the controller rolled this turn (r64-02): "as long as you've rolled a 6 this turn", a tally of the dice
//! that came up one number (`TurnTally::R6402Rolled`).

use super::tokens::Reader;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

/// The condition that the controller rolled `result` on a die this turn.
fn r6402_condition(result: u32) -> Condition {
    Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what: TurnTally::R6402Rolled(result),
        },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// The article of a number written as digits: "an 8", "an 11", "an 18", "a 6".
fn r6402_article(result: u32) -> &'static str {
    let digits = result.to_string();
    if digits.starts_with('8') || result == 11 || result == 18 {
        "an"
    } else {
        "a"
    }
}

/// Reads "you've rolled a N this turn".
pub(super) fn r6402_read(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you've rolled")?;
    let article = r.take_word()?;
    let result = r.number()?;
    (result > 0 && article == r6402_article(result)).then_some(())?;
    r.need("this turn")?;
    Some(r6402_condition(result))
}

/// The words of a tally of one result the controller rolled, compared as "one or more".
pub(super) fn r6402_texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let Amount::Tallied {
        who: Ref::You,
        what: TurnTally::R6402Rolled(result),
    } = left
    else {
        return Vec::new();
    };
    if op != Cmp::Ge || number != 1 || *result == 0 {
        return Vec::new();
    }
    vec![format!(
        "you've rolled {} {result} this turn",
        r6402_article(*result)
    )]
}
