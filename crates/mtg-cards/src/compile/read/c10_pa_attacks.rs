//! The wordings of an attack at a player or a planeswalker of theirs: the attacker is "that creature".

use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;

/// Who the creature attacks after "attacks": "enchanted player", "the monarch", "one of your opponents", or "you or a
/// planeswalker you control" (CR 508.3a); `None` for an attack that names no one.
pub(super) fn c10_pa_attacked(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let what = what.clone();
    for (words, at) in [
        ("enchanted player", Ref::AttachedPlayer),
        ("the monarch", Ref::Monarch),
        ("one of your opponents", Ref::Opponents),
    ] {
        if r.words(words) {
            return Some(TriggerPattern::AttacksPlayer { what, at });
        }
    }
    r.words("you or a planeswalker you control")
        .then_some(TriggerPattern::AttacksPlayerOrPlaneswalker { what, at: Ref::You })
}

/// "That creature's controller" for the controller of the attacker in a trigger on an attack at a player, or at a player
/// or a planeswalker they control.
pub(super) fn c10_pa_wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let attack = matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::AttacksPlayer { .. }
                | TriggerPattern::AttacksPlayerOrPlaneswalker { .. },
            ..
        }
    );
    if !attack {
        return Vec::new();
    }
    texts
        .iter()
        .filter(|text| text.contains("that permanent's controller"))
        .map(|text| text.replace("that permanent's controller", "that creature's controller"))
        .collect()
}
