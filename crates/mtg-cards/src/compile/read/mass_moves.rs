//! Every object a phrase names changes zones at once: "Put all creatures on the bottom of their owners' libraries.",
//! "Return all nonland permanents to their owners' hands.", "Return all artifact and enchantment cards from your
//! graveyard to the battlefield.", each read from its wording and written back the same way.

use super::library::place_text;
use super::selectors::{Form, all, phrase_text};
use super::tokens::Reader;
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Whether the selector names permanents (not cards in a zone), which go to "their owners'" places.
fn permanents(selector: &Selector) -> bool {
    !selector
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell))
}

/// Whether the selector names cards in the graveyard of the player the effect is about.
fn from_your_graveyard(selector: &Selector) -> bool {
    selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        && selector.0.contains(&Pred::OwnedBy(Ref::You))
}

/// "their owners'": the apostrophe of a plural stands alone in the tokens.
pub(super) fn their_owners(r: &mut Reader<'_>) -> Option<()> {
    (r.words("their owners") && r.punct('\'')).then_some(())
}

/// "Put all creatures on the bottom of their owners' libraries.": each permanent goes to its owner's library.
pub fn put_all(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    let what = all(r)?;
    let Ref::Each(selector) = &what else {
        return None;
    };
    permanents(selector).then_some(())?;
    let place = if r.words("on top of") {
        Place::Top
    } else {
        r.need("on the bottom of")?;
        Place::Bottom
    };
    their_owners(r)?;
    r.need("libraries")?;
    Some(vec![Effect::Move {
        what,
        to: Destination::Library { place, order: None },
    }])
}

/// "Return all nonland permanents to their owners' hands.", "Return all artifact and enchantment cards from your
/// graveyard to the battlefield.".
pub fn return_all(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("return")?;
    let what = all(r)?;
    let Ref::Each(selector) = &what else {
        return None;
    };
    if from_your_graveyard(selector) {
        r.need("to the battlefield")?;
        let tapped = r.word("tapped");
        return Some(vec![Effect::Move {
            what,
            to: Destination::Battlefield {
                tapped,
                under: None,
                transformed: false,
                counters: Vec::new(),
            },
        }]);
    }
    permanents(selector).then_some(())?;
    r.need("to")?;
    their_owners(r)?;
    r.need("hands")?;
    Some(vec![Effect::Move {
        what,
        to: Destination::Hand,
    }])
}

/// What `put_all` and `return_all` read, written back; `None` for any other move.
pub fn text(effect: &Effect) -> Option<String> {
    let Effect::Move {
        what: Ref::Each(selector),
        to,
    } = effect
    else {
        return None;
    };
    let phrase = phrase_text(selector, Form::All)?;
    match to {
        Destination::Library { place, order: None } if permanents(selector) => Some(format!(
            "put all {phrase} {} their owners' libraries",
            place_text(*place)?
        )),
        Destination::Hand if permanents(selector) => {
            Some(format!("return all {phrase} to their owners' hands"))
        }
        Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters,
        } if counters.is_empty() && from_your_graveyard(selector) => Some(format!(
            "return all {phrase} to the battlefield{}",
            if *tapped { " tapped" } else { "" }
        )),
        _ => None,
    }
}
