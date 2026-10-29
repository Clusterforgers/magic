//! Three events of one trigger named in one clause: "Whenever another creature dies, or a creature card is put into a
//! graveyard from anywhere other than the battlefield, or a creature card leaves your graveyard" (Syr Konrad). The
//! renderers word each event of an `AnyOf` apart and say "creature" for the card put into a graveyard.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// The words of an event after its "Whenever": a creature card of yours that leaves your graveyard, a creature card put
/// into a graveyard from anywhere else, or the renderers' own words for the event.
fn t15a_event(event: &TriggerPattern) -> Option<String> {
    if let TriggerPattern::ZoneChange {
        what,
        from: Some(ZoneKind::Graveyard),
        to: None,
    } = event
        && matches!(
            what.0.as_slice(),
            [Pred::Type(CardType::Creature), Pred::OwnedBy(Ref::You)]
        )
    {
        return Some("a creature card leaves your graveyard".to_owned());
    }
    let text = pattern_text(event)?;
    let text = text
        .strip_prefix("Whenever ")
        .or_else(|| text.strip_prefix("whenever "))?;
    Some(text.replacen(
        "a creature is put into a graveyard",
        "a creature card is put into a graveyard",
        1,
    ))
}

/// Adds the wordings of a trigger of exactly three events, each named after the first one's "Whenever".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    let [first, second, third] = events.as_slice() else {
        return;
    };
    let (Some(one), Some(two), Some(three)) =
        (pattern_text(first), t15a_event(second), t15a_event(third))
    else {
        return;
    };
    let mut single = ability.clone();
    if let Ability::Triggered { when, .. } = &mut single {
        *when = first.clone();
    }
    let opening = format!("{one}, ");
    for text in variants(&single) {
        if let Some(rest) = text.strip_prefix(&opening) {
            texts.push(format!("{one}, or {two}, or {three}, {rest}"));
        }
    }
}
