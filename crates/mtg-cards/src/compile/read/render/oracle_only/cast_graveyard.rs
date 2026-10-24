//! A trigger on casting a spell from your own graveyard: "Whenever you cast a spell from your graveyard, ...". The
//! spell is a card of the graveyard of its caster, so it is owned by them.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on the casting of a spell you own from a graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { what, by: Ref::You },
        ..
    } = ability
    else {
        return;
    };
    if what.0 != [Pred::CastFrom(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)] {
        return;
    }
    let plain = TriggerPattern::SpellCast {
        what: Selector::default(),
        by: Ref::You,
    };
    let Some(event) = pattern_text(&plain) else {
        return;
    };
    let mut bare = ability.clone();
    if let Ability::Triggered { when, .. } = &mut bare {
        *when = plain;
    }
    for text in variants(&bare) {
        if let Some(rest) = text.strip_prefix(&event) {
            texts.push(format!("{event} from your graveyard{rest}"));
        }
    }
}
