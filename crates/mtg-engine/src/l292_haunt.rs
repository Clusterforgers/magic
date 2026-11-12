//! A haunt creature's "When ~ enters or the creature it haunts dies" (CR 702.55c): one triggered ability whose first event
//! it waits for on the battlefield and whose second in exile, haunting a creature. Each event can happen only in its own
//! zone (a card leaving exile is a new object, CR 400.7, and a permanent haunts nothing), so the ability works in both.

use mtg_cards::ability::Ability;
use mtg_cards::refs::{Pred, Ref, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

/// Whether `ability` is a triggered ability of the battlefield that also waits in exile for the creature its card haunts.
pub(crate) fn works_in_exile_too(ability: &Ability) -> bool {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        from: ZoneKind::Battlefield,
        ..
    } = ability
    else {
        return false;
    };
    events.iter().any(|event| {
        matches!(
            event,
            TriggerPattern::ZoneChange { what, from: Some(ZoneKind::Battlefield), .. }
                if what.0 == [Pred::HauntedBy(Ref::This)]
        )
    })
}

/// Whether `ability` works in `zone`, besides the zone its `from` names.
pub(crate) fn works_here_too(ability: &Ability, zone: ZoneKind) -> bool {
    zone == ZoneKind::Exile && works_in_exile_too(ability)
}
