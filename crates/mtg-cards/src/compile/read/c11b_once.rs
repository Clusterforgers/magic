//! "Once during each of your turns, you may cast a creature spell from your graveyard.": a permission over the spells
//! that match, cast from the graveyard once in each of its controller's turns.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// "Once during each of your turns, you may cast a(n) <kind> spell from your graveyard."
pub fn c11b_once(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("once during each of your turns")?;
    r.punct(',').then_some(())?;
    r.need("you may cast")?;
    (r.word("a") || r.word("an")).then_some(())?;
    // The phrase takes "from your graveyard" in as the zone of the spell's card.
    let spells = phrase(r, Form::Target)?;
    let [
        Pred::Spell,
        kinds @ ..,
        Pred::In(ZoneKind::Graveyard),
        Pred::OwnedBy(Ref::You),
    ] = spells.0.as_slice()
    else {
        return None;
    };
    (!kinds.is_empty()).then_some(())?;
    Some(Ability::Rule {
        rule: GameRule::MayCast(c11b_permission(Selector(kinds.to_vec()))),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The permission `c11b_once` gives over the cards `what` selects.
fn c11b_permission(what: Selector) -> CastPermission {
    CastPermission {
        who: Ref::You,
        what,
        from: Some(ZoneKind::Graveyard),
        free: false,
        flash: false,
        only: None,
        play: false,
        once: true,
    }
}

/// The wording of the permission `c11b_once` reads, none for another.
pub fn c11b_once_text(permission: &CastPermission) -> Option<String> {
    (*permission == c11b_permission(permission.what.clone()) && !permission.what.0.is_empty())
        .then_some(())?;
    let mut preds = vec![Pred::Spell];
    preds.extend(permission.what.0.iter().cloned());
    let spell = phrase_text(&Selector(preds), Form::Target)?;
    let article = if spell.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "Once during each of your turns, you may cast {article} {spell} from your graveyard"
    ))
}
