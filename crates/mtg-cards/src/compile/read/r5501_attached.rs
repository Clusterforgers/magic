//! A choice made as an attachment becomes attached: "As this Equipment becomes attached to a creature, choose a color."
//! (`ReplaceWhen::BecomesAttached` with `ReplaceBy::Choose`), and "the last chosen color" that a static ability of the
//! attachment reads. Read from the wording and written back the same way.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};

/// What may be chosen, and its words.
const KINDS: [(EntersChoice, &str); 2] = [
    (EntersChoice::Color, "choose a color"),
    (EntersChoice::CreatureType, "choose a creature type"),
];

/// "As this Equipment becomes attached to a creature, choose a color."
pub(super) fn r5501_as_attached(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("as")?;
    r.source().then_some(())?;
    r.need("becomes attached to a creature")?;
    r.punct(',').then_some(())?;
    let kind = KINDS
        .iter()
        .find(|(_, words)| r.words(words))
        .map(|(kind, _)| *kind)?;
    Some(Ability::Replacement {
        when: ReplaceWhen::BecomesAttached {
            what: Selector::this(),
        },
        by: ReplaceBy::Choose(kind),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// What `r5501_as_attached` reads, written back.
pub(super) fn r5501_as_attached_texts(what: &Selector, by: &ReplaceBy) -> Vec<String> {
    let ReplaceBy::Choose(kind) = by else {
        return Vec::new();
    };
    if *what != Selector::this() {
        return Vec::new();
    }
    KINDS
        .iter()
        .filter(|(listed, _)| listed == kind)
        .flat_map(|(_, words)| {
            ["Equipment", "Aura"]
                .map(|noun| format!("As this {noun} becomes attached to a creature, {words}"))
        })
        .collect()
}

/// The wordings of a static ability of an attachment that reads its choice: what made it chose anew at each attaching,
/// so "the chosen color" is "the last chosen color".
pub(super) fn r5501_last_chosen(written: &[String]) -> Vec<String> {
    written
        .iter()
        .filter(|text| text.starts_with("Equipped creature") && text.contains("the chosen color"))
        .map(|text| text.replace("the chosen color", "the last chosen color"))
        .collect()
}
