//! "When a Faerie is championed with this creature, ...": a creature of the type exiled from the battlefield by an ability
//! of the card (CR 702.72b: its champion ability, the only one that exiles), written from the twin that triggers as the
//! card enters.

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;

/// The subtype of a creature championed with the card: `[Subtype]` moving from the battlefield to exile, caused by the
/// card itself.
fn championed(when: &TriggerPattern) -> Option<&str> {
    let TriggerPattern::Caused {
        inner,
        by: None,
        source,
    } = when
    else {
        return None;
    };
    (source.0 == [Pred::This]).then_some(())?;
    let TriggerPattern::ZoneChange {
        what,
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Exile),
    } = inner.as_ref()
    else {
        return None;
    };
    match what.0.as_slice() {
        [Pred::Subtype(kind)] => Some(kind.0.as_str()),
        _ => None,
    }
}

/// The wordings of a trigger on a creature championed with the card; `None` for any other ability.
pub(super) fn r374_championed(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Triggered { when, .. } = ability else {
        return None;
    };
    let kind = championed(when)?;
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = TriggerPattern::ZoneChange {
            what: crate::refs::Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        };
    }
    let article = if kind.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    let head = format!("When {article} {kind} is championed with ~, ");
    Some(
        written(&twin)
            .iter()
            .filter_map(|text| text.strip_prefix("When ~ enters, "))
            .map(|rest| format!("{head}{rest}"))
            .collect(),
    )
}
