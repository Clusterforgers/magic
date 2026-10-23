//! Revealing from the top of the library until a card matches, and where the card and the rest go, read from the
//! wordings of the cards and written back the same way. The reveal and the moves after it are one passage in the data.

use super::library::{
    destination_text, filter_text, found_destination, rest_destination, rest_text, what,
};
use super::tokens::Reader;
use crate::effect::{Destination, Effect, FOUND, REVEALED};
use crate::refs::{Ref, Selector};
use crate::value::Amount;

/// The words that open the sentence, after the verb: the library and the card that ends the revealing.
const OPENING: &str = "reveal cards from the top of your library until you reveal";

/// What follows the reveal: the card found goes somewhere and the rest elsewhere, or every card revealed goes to one place.
enum Tail {
    /// "Put that card into your hand and the rest on the bottom of your library in a random order."
    Found(Destination, Destination),
    /// ", then put all cards revealed this way into your hand."
    All(Destination),
}

fn reveal(filter: Selector) -> Effect {
    Effect::RevealUntil {
        who: Ref::You,
        filter,
        count: Amount::one(),
        bind: FOUND.to_owned(),
        revealed_as: REVEALED.to_owned(),
    }
}

fn move_to(bound: &str, to: Destination) -> Effect {
    Effect::Move {
        what: Ref::Bound(bound.to_owned()),
        to,
    }
}

/// "Reveal cards from the top of your library until you reveal a creature card.", then "Put that card into your hand and
/// the rest on the bottom of your library in a random order." or ", then put all cards revealed this way into your hand".
pub fn reveal_until(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(OPENING)?;
    let (filter, bounds) = what(r)?;
    bounds.is_one().then_some(())?;
    let mut effects = vec![reveal(filter)];
    match r.attempt(tail) {
        Some(Tail::Found(found, rest)) => {
            effects.push(move_to(FOUND, found));
            effects.push(move_to(REVEALED, rest));
        }
        Some(Tail::All(to)) => effects.push(move_to(REVEALED, to)),
        None => {}
    }
    Some(effects)
}

fn tail(r: &mut Reader<'_>) -> Option<Tail> {
    if r.punct(',') {
        r.need("then put all cards revealed this way")?;
        return Some(Tail::All(found_destination(r)?));
    }
    r.punct('.').then_some(())?;
    r.need("put that card")?;
    let found = found_destination(r)?;
    r.need("and the rest")?;
    Some(Tail::Found(found, rest_destination(r)?))
}

/// The sentences the effects at the front of the list are when they reveal until a card matches, and how many effects they take.
pub fn sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::RevealUntil {
            who: Ref::You,
            filter,
            count,
            bind,
            revealed_as,
        },
        after @ ..,
    ] = effects
    else {
        return None;
    };
    (count.is_one() && bind == FOUND && revealed_as == REVEALED).then_some(())?;
    let card = filter_text(filter, false)?;
    let article = super::triggers::article(&card);
    let head = format!("{OPENING} {article} {card}");
    match after {
        [
            Effect::Move {
                what: Ref::Bound(found),
                to: found_to,
            },
            Effect::Move {
                what: Ref::Bound(rest),
                to: rest_to,
            },
            ..,
        ] if found == FOUND && rest == REVEALED => {
            let text = format!(
                "{head}. Put that card {} and the rest {}",
                destination_text(found_to)?,
                rest_text(rest_to)?
            );
            Some((text, 3))
        }
        [
            Effect::Move {
                what: Ref::Bound(all),
                to,
            },
            ..,
        ] if all == REVEALED => {
            let text = format!(
                "{head}, then put all cards revealed this way {}",
                destination_text(to)?
            );
            Some((text, 2))
        }
        _ => Some((head, 1)),
    }
}
