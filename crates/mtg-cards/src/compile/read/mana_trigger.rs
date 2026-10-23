//! Triggered mana abilities (CR 605.1b): the effect of "Whenever you tap a land for mana, add one mana of any type that
//! land produced." and "Whenever enchanted land is tapped for mana, its controller adds an additional {G}.", read and
//! written back. The event is read by `triggers`.

use super::effects::{combination, mana_type};
use super::tokens::Reader;
use crate::effect::{Effect, Produce};
use crate::mana_trigger::ManaTrigger;
use crate::refs::{Pred, Ref, Selector};
use crate::spend::SpendOn;
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// Whether `what` is the land an Aura is attached to.
pub fn attached(what: &Selector) -> bool {
    what.0.first() == Some(&Pred::Is(Ref::Attached))
}

/// The mana added: "an additional {G}{G}", "an additional one mana of any color", "an additional one mana of the
/// chosen color", or the whole of "one mana of any type that land produced".
fn produced(r: &mut Reader<'_>) -> Option<Produce> {
    if r.words("one mana of any type that land produced") {
        return Some(Produce::OfTapped);
    }
    r.need("an additional")?;
    if r.words("one mana of any color") {
        return Some(Produce::AnyColor(1));
    }
    if r.words("one mana of the chosen color") {
        return Some(Produce::ChosenColor(1));
    }
    if let Some(combination) = r.attempt(combination) {
        return Some(combination);
    }
    let mut list = Vec::new();
    while let Some(mana) = r.attempt(|r| mana_type(r.symbol()?)) {
        list.push(mana);
    }
    (!list.is_empty()).then_some(Produce::Fixed(list))
}

/// The effect of a triggered mana ability: who adds mana ("add" for you, "its controller adds" for the Aura's land,
/// "that player adds" for any player) and the mana. `None` for any other sentence.
pub fn effects(r: &mut Reader<'_>, when: &TriggerPattern) -> Option<Vec<Effect>> {
    let TriggerPattern::TappedForMana { what, by } = when else {
        return None;
    };
    let controller = attached(what) && r.words("its controller adds");
    let player = !attached(what) && *by != Ref::You && r.words("that player adds");
    let who = if controller || player {
        Ref::Triggering
    } else if *by == Ref::You && r.word("add") {
        Ref::You
    } else {
        return None;
    };
    let mana = produced(r)?;
    r.sentence_end().then_some(())?;
    Some(vec![Effect::AddMana {
        who,
        mana,
        times: Amount::one(),
        only: Vec::<SpendOn>::new(),
    }])
}

/// The words of the mana a triggered mana ability adds, after who adds it: "an additional {G}".
fn mana_words(mana: &Produce) -> Option<String> {
    Some(match mana {
        Produce::OfTapped => "one mana of any type that land produced".to_owned(),
        Produce::AnyColor(1) => "an additional one mana of any color".to_owned(),
        Produce::ChosenColor(1) => "an additional one mana of the chosen color".to_owned(),
        Produce::AnyCombination {
            count: count @ 2..,
            among,
        } => format!(
            "an additional {}",
            super::render::combination_text(*count, among)?
        ),
        Produce::Fixed(list) if !list.is_empty() => {
            format!("an additional {}", super::render::symbols_text(list))
        }
        _ => return None,
    })
}

/// The sentence of a triggered mana ability, without its period: "add an additional {G}" or "its controller adds an
/// additional {G}". `None` when the wording is not one the reader reads.
pub fn text(shape: &ManaTrigger<'_>) -> Option<String> {
    let subject = match shape.who {
        Ref::You if *shape.by == Ref::You => "add",
        Ref::Triggering if attached(shape.what) => "its controller adds",
        Ref::Triggering if *shape.by != Ref::You => "that player adds",
        _ => return None,
    };
    Some(format!("{subject} {}", mana_words(shape.mana)?))
}
