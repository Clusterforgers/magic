//! A mana ability the basic land types of a face already give is dropped: the types are the ability (CR 305.6), the
//! reminder text of the card only states it, so no clause of the text is left for an explicit copy.

use mtg_core::mana::ManaType;
use serde_json::Value;

use super::text::without_reminders;
use super::{Repair, RepairClass};
use crate::ability::Ability;
use crate::assemble::{bare_tap_mana, intrinsic_mana};
use crate::effect::Produce;
use crate::support::PrintedFace;

/// The mana types one activation of a bare `{T}: Add ...` can add when each way adds one mana: `{T}: Add {U}` and
/// `{T}: Add {U} or {R}` (in either of the two shapes), nothing for any other ability.
fn one_mana_of(ability: &Ability) -> Vec<ManaType> {
    match bare_tap_mana(ability) {
        Some(Produce::Fixed(mana)) if mana.len() == 1 => mana.clone(),
        Some(Produce::OneOf(mana)) => mana.clone(),
        Some(Produce::OneOfLists(lists)) if lists.iter().all(|list| list.len() == 1) => {
            lists.iter().flatten().copied().collect()
        }
        _ => Vec::new(),
    }
}

/// Whether the text adds mana outside its reminders: such a face prints its mana ability, which a clause stands for.
fn prints_mana(text: &str) -> bool {
    without_reminders(text).contains("Add {")
}

/// Drops every bare tap ability of the list whose one mana is of a type the basic land types of the face give.
pub(super) fn repair(
    abilities: &mut Vec<Value>,
    face: &PrintedFace,
    at: &str,
    repairs: &mut Vec<Repair>,
) {
    let given = intrinsic_mana(face);
    if given.is_empty() || prints_mana(&face.oracle_text) {
        return;
    }
    let mut index = 0;
    abilities.retain(|value| {
        let n = index;
        index += 1;
        let types = serde_json::from_value::<Ability>(value.clone())
            .map(|ability| one_mana_of(&ability))
            .unwrap_or_default();
        let implied = !types.is_empty() && types.iter().all(|mana| given.contains(mana));
        if implied {
            repairs.push(Repair {
                class: RepairClass::ImpliedMana,
                at: format!("{at}.abilities[{n}]"),
                note: "the basic land types of the face give this mana: dropped, as no clause of the text is left for it"
                    .to_owned(),
            });
        }
        !implied
    });
}

#[cfg(test)]
mod tests;
