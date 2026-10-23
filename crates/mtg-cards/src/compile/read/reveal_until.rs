//! Revealing from the top of the library until a card matches, and where the card and the rest go, read from the
//! wordings of the cards and written back the same way. The reveal and the moves after it are one passage in the data.

use super::library::{
    destination_text, filter, filter_text, found_destination, rest_destination, rest_text, what,
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
    /// "You may put that card onto the battlefield. Then put all cards revealed this way that weren't put onto the
    /// battlefield on the bottom of your library in a random order."
    MayPut(Destination, Destination),
}

fn reveal(filter: Selector, count: Amount) -> Effect {
    Effect::RevealUntil {
        who: Ref::You,
        filter,
        count,
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

/// The cards that end the revealing: "a creature card" for one, "X land cards" for the X of the spell.
fn ending(r: &mut Reader<'_>) -> Option<(Selector, Amount)> {
    if r.x() {
        return Some((filter(r, true)?, Amount::X));
    }
    let (filter, bounds) = what(r)?;
    bounds.is_one().then_some((filter, Amount::one()))
}

/// "Reveal cards from the top of your library until you reveal a creature card.", then "Put that card into your hand and
/// the rest on the bottom of your library in a random order." or ", then put all cards revealed this way into your hand".
pub fn reveal_until(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(OPENING)?;
    let (filter, count) = ending(r)?;
    let many = !count.is_one();
    let mut effects = vec![reveal(filter.clone(), count)];
    match r.attempt(|r| tail(r, many.then_some(&filter))) {
        Some(Tail::Found(found, rest)) => {
            effects.push(move_to(FOUND, found));
            effects.push(move_to(REVEALED, rest));
        }
        Some(Tail::All(to)) => effects.push(move_to(REVEALED, to)),
        Some(Tail::MayPut(found, rest)) => {
            effects.push(Effect::May {
                who: Ref::You,
                effects: vec![move_to(FOUND, found)],
            });
            effects.push(move_to(REVEALED, rest));
        }
        None => {}
    }
    Some(effects)
}

/// The cards that were found, as the tail names them: "that card", or for several "those cards" or "those land cards".
fn found_cards(r: &mut Reader<'_>, several: Option<&Selector>) -> Option<()> {
    let Some(ending) = several else {
        return r.words("that card").then_some(());
    };
    r.need("those")?;
    let named = filter(r, true)?;
    (named.0.is_empty() || named == *ending).then_some(())
}

fn tail(r: &mut Reader<'_>, several: Option<&Selector>) -> Option<Tail> {
    if r.punct(',') {
        several.is_none().then_some(())?;
        r.need("then put all cards revealed this way")?;
        return Some(Tail::All(found_destination(r)?));
    }
    r.punct('.').then_some(())?;
    if several.is_none() && r.words("you may put that card") {
        let found = found_destination(r)?;
        r.punct('.').then_some(())?;
        r.need("then put all cards revealed this way that weren't put")?;
        r.need(&destination_text(&found)?)?;
        return Some(Tail::MayPut(found, rest_destination(r)?));
    }
    r.need("put")?;
    found_cards(r, several)?;
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
    (bind == FOUND && revealed_as == REVEALED).then_some(())?;
    let one = count.is_one();
    let (head, those) = if one {
        let card = filter_text(filter, false)?;
        let article = super::triggers::article(&card);
        (
            format!("{OPENING} {article} {card}"),
            "that card".to_owned(),
        )
    } else if *count == Amount::X {
        let cards = filter_text(filter, true)?;
        (format!("{OPENING} X {cards}"), format!("those {cards}"))
    } else {
        return None;
    };
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
                "{head}. Put {those} {} and the rest {}",
                destination_text(found_to)?,
                rest_text(rest_to)?
            );
            Some((text, 3))
        }
        [
            Effect::May {
                who: Ref::You,
                effects: put,
            },
            Effect::Move {
                what: Ref::Bound(rest),
                to: rest_to,
            },
            ..,
        ] if one && rest == REVEALED => {
            let [
                Effect::Move {
                    what: Ref::Bound(found),
                    to: found_to,
                },
            ] = put.as_slice()
            else {
                return None;
            };
            (found == FOUND).then_some(())?;
            let to = destination_text(found_to)?;
            let text = format!(
                "{head}. You may put that card {to}. Then put all cards revealed this way that weren't put {to} {}",
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
        ] if one && all == REVEALED => {
            let text = format!(
                "{head}, then put all cards revealed this way {}",
                destination_text(to)?
            );
            Some((text, 2))
        }
        _ => Some((head, 1)),
    }
}
