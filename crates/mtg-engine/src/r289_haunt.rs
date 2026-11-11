//! Haunt's second trigger condition beside another (CR 702.55c): "When this creature enters or the creature it haunts
//! dies" is one ability of the permanent, and its "the creature it haunts dies" part also triggers while the card is in
//! exile haunting a creature.

use mtg_cards::ability::Ability;
use mtg_cards::refs::{Pred, Ref, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

/// Whether the pattern is "the creature this card haunts dies" (`Pred::HauntedBy(This)`).
fn r289_is_haunted_death(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::ZoneChange { what, .. } if what.0.contains(&Pred::HauntedBy(Ref::This))
    )
}

/// The part of a triggered ability that works from the battlefield and also from exile: the "the creature it haunts
/// dies" events of an `AnyOf` whose other events need the permanent ("When this creature enters or the creature it
/// haunts dies"). `None` for every other ability.
pub(crate) fn r289_haunt_part(ability: &Ability) -> Option<TriggerPattern> {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(parts),
        from: ZoneKind::Battlefield,
        ..
    } = ability
    else {
        return None;
    };
    let haunted: Vec<TriggerPattern> = parts
        .iter()
        .filter(|part| r289_is_haunted_death(part))
        .cloned()
        .collect();
    match haunted.len() {
        0 => None,
        1 => haunted.into_iter().next(),
        _ => Some(TriggerPattern::AnyOf(haunted)),
    }
}
