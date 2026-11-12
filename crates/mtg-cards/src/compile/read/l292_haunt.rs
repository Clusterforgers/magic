//! The event of a haunt creature (CR 702.55c): "When ~ enters or the creature it haunts dies", one trigger of two events,
//! read and written as one head.

use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// The two events of the head: the card entering and the creature it haunts dying.
pub(super) fn l292_haunt_events() -> TriggerPattern {
    TriggerPattern::AnyOf(vec![
        TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::HauntedBy(Ref::This)]),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
    ])
}

/// Reads "When ~ enters or the creature it haunts dies".
pub(super) fn l292_haunt_head(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.word("when").then_some(())?;
    r.source().then_some(())?;
    r.need("enters or the creature it haunts dies")?;
    Some(l292_haunt_events())
}

/// Writes the head of [`l292_haunt_events`].
pub(super) fn l292_haunt_text(patterns: &[TriggerPattern]) -> Option<String> {
    let TriggerPattern::AnyOf(events) = l292_haunt_events() else {
        return None;
    };
    (patterns == events.as_slice())
        .then(|| "When ~ enters or the creature it haunts dies".to_owned())
}
