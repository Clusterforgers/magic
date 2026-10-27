//! The smallest hand at the table: "a player has one or fewer cards in hand", "an opponent has no cards in hand". Read
//! as `LowestHand` of every player or of the opponents at most the number, and written back the same way.

use super::facts::{quantity, quantity_texts};
use super::tokens::Reader;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition};

/// "a player has one or fewer cards in hand", "an opponent has no cards in hand".
pub(super) fn read(r: &mut Reader<'_>) -> Option<Condition> {
    let who = if r.words("a player has") {
        Ref::EachPlayer
    } else {
        r.need("an opponent has")?;
        Ref::Opponents
    };
    let number = if r.words("no cards") {
        0
    } else {
        let number = quantity(r).filter(|number| *number > 0)?;
        r.need("or fewer cards")?;
        number
    };
    r.need("in hand")?;
    Some(Condition::Compare {
        left: Amount::LowestHand(who),
        op: Cmp::Le,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// What `read` reads, written back after "if" or "as long as", one text for each wording.
pub(super) fn texts(who: &Ref, number: u32) -> Vec<String> {
    let subject = match who {
        Ref::EachPlayer => "a player has",
        Ref::Opponents => "an opponent has",
        _ => return Vec::new(),
    };
    if number == 0 {
        return vec![format!("{subject} no cards in hand")];
    }
    quantity_texts(number)
        .iter()
        .map(|n| format!("{subject} {n} or fewer cards in hand"))
        .collect()
}
