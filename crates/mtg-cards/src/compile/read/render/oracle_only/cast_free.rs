//! A free cast from the hand in a trigger: "Whenever this creature attacks, you may cast an instant or sorcery spell
//! from your hand without paying its mana cost." The data chooses a card of the hand and casts it.

use super::super::{article, trigger_head};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, ZoneKind};
use crate::value::Amount;

/// The kind of spell the cards chosen are, as the card words it.
fn kind(preds: &[Pred]) -> Option<String> {
    let name = |pred: &Pred| match pred {
        Pred::Type(found) => Some(format!("{found:?}").to_lowercase()),
        _ => None,
    };
    match preds {
        [Pred::AnyOf(kinds)] => {
            let names: Option<Vec<String>> = kinds.iter().map(name).collect();
            names
                .filter(|names| names.len() > 1)
                .map(|names| names.join(" or "))
        }
        [single] => name(single),
        _ => None,
    }
}

/// Adds the wordings of a trigger that casts a spell of a kind from the hand for free.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(chosen),
            count,
            bind,
            rest_as: None,
        },
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound(cast),
            free: true,
            play: false,
            way: None,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let may = Bounds::up_to(Some(Amount::Fixed(1)));
    let [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You), rest @ ..] = chosen.0.as_slice() else {
        return;
    };
    if bind != cast || !otherwise.is_empty() || *count != may {
        return;
    }
    let (Some(spell), Some(head)) = (kind(rest), trigger_head(when, intervening_if.as_ref()))
    else {
        return;
    };
    texts.push(format!(
        "{} you may cast {} {spell} spell from your hand without paying its mana cost.",
        head.join(" "),
        article(&spell)
    ));
}
