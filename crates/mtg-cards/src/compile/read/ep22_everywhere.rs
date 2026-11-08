//! "Creatures you control are artifacts in addition to their other types. The same is true for creature spells you control
//! and creature cards you own that aren't on the battlefield.": a static ability over every zone (an `AnyOf` of each zone),
//! written as the sentence about the permanents and the sentence that extends it.

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Every zone of a game, as the selector of a static ability over all of them lists it.
pub const EVERY_ZONE: [ZoneKind; 7] = [
    ZoneKind::Battlefield,
    ZoneKind::Stack,
    ZoneKind::Hand,
    ZoneKind::Library,
    ZoneKind::Graveyard,
    ZoneKind::Exile,
    ZoneKind::Command,
];

/// The wordings of a static ability of one card type you control in every zone; empty for any other ability.
pub(super) fn texts(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::Each(selector),
        mods,
        condition,
        defining,
        from,
    } = ability
    else {
        return Vec::new();
    };
    let every = |pred: &Pred| {
        matches!(pred, Pred::AnyOf(list) if list.len() == EVERY_ZONE.len()
            && EVERY_ZONE.iter().all(|zone| list.contains(&Pred::In(*zone))))
    };
    if !selector.0.iter().any(every) {
        return Vec::new();
    }
    let rest: Vec<Pred> = selector.0.iter().filter(|p| !every(p)).cloned().collect();
    let noun = match rest.as_slice() {
        [Pred::Type(kind), Pred::ControlledBy(Ref::You)]
        | [Pred::ControlledBy(Ref::You), Pred::Type(kind)] => format!("{kind:?}").to_lowercase(),
        _ => return Vec::new(),
    };
    let plain = Ability::Static {
        affected: Ref::Each(Selector(rest)),
        mods: mods.clone(),
        condition: condition.clone(),
        defining: *defining,
        from: *from,
    };
    super::render::written_variants(&plain)
        .into_iter()
        .map(|text| {
            let text = text.strip_suffix('.').unwrap_or(&text);
            format!(
                "{text}. The same is true for {noun} spells you control and {noun} cards you own that aren't on the battlefield."
            )
        })
        .collect()
}
