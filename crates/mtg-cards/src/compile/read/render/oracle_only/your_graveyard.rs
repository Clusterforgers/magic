//! A spell the player casts from their own graveyard: "Whenever you cast a spell from your graveyard". The renderers
//! write the zone without an owner ("from a graveyard").

use crate::ability::Ability;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on a cast from the graveyard of the player who casts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { what, by: Ref::You },
        ..
    } = ability
    else {
        return;
    };
    if !what.0.contains(&Pred::CastFrom(ZoneKind::Graveyard)) {
        return;
    }
    let (from, into) = (
        "cast a spell from a graveyard",
        "cast a spell from your graveyard",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
