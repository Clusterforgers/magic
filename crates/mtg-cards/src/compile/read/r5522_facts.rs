//! Facts of the turn an intervening "if" or an activation condition asks (r55-22), each read and written by one row:
//! "you've played a land or cast a spell this turn from anywhere other than your hand".

use super::tokens::Reader;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

/// The rows: the tally of the controller, the fewest that make it hold, and the words.
const R5522_ROWS: [(TurnTally, i32, &str); 1] = [(
    TurnTally::PlayedNotFromHand,
    1,
    "you've played a land or cast a spell this turn from anywhere other than your hand",
)];

/// The condition of a row.
fn r5522_condition(what: TurnTally, least: i32) -> Condition {
    Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(least),
    }
}

/// Reads the words of a row.
pub(super) fn r5522_read(r: &mut Reader<'_>) -> Option<Condition> {
    R5522_ROWS
        .iter()
        .find(|(_, _, words)| r.words(words))
        .map(|(what, least, _)| r5522_condition(*what, *least))
}

/// The words of the row the amount compared with the number is.
pub(super) fn r5522_texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    R5522_ROWS
        .iter()
        .filter(|(what, least, _)| {
            let Condition::Compare {
                left: row,
                op: row_op,
                ..
            } = r5522_condition(*what, *least)
            else {
                return false;
            };
            row == *left && row_op == op && u32::try_from(*least) == Ok(number)
        })
        .map(|(_, _, words)| (*words).to_owned())
        .collect()
}
