//! A cost that taps a legendary permanent: "Tap an untapped legendary permanent you control: Untap this artifact." The
//! renderers put the supertype before "untapped".

use crate::ability::Ability;
use crate::cost::CostItem;

/// Adds the wordings with "untapped" before the supertype for a cost that taps permanents.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { cost, .. } = ability else {
        return;
    };
    if !cost
        .0
        .iter()
        .any(|part| matches!(part, CostItem::TapPermanents { .. }))
    {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("Tap a legendary untapped "))
        .map(|text| text.replacen("Tap a legendary untapped ", "Tap an untapped legendary ", 1))
        .collect();
    texts.extend(said);
}
