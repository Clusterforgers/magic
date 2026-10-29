//! An ability with an empty effect list says nothing; it is dropped and its clause is left to the table.

use serde_json::Value;

use super::{Repair, RepairClass, body_of};

/// The abilities that have effects.
const WITH_EFFECTS: [&str; 4] = ["Spell", "Triggered", "Activated", "Loyalty"];

/// Drops every ability of the list whose effects are an empty list.
pub(super) fn repair(abilities: &mut Vec<Value>, at: &str, repairs: &mut Vec<Repair>) {
    let mut index = 0;
    abilities.retain_mut(|ability| {
        let n = index;
        index += 1;
        let empty = body_of(ability).is_some_and(|(name, body)| {
            WITH_EFFECTS.contains(&name)
                && body
                    .get("effects")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
        });
        if empty {
            repairs.push(Repair {
                class: RepairClass::NoEffect,
                at: format!("{at}.abilities[{n}]"),
                note: "no effect: dropped, the clause stays with the table unless another ability stands for it"
                    .to_owned(),
            });
        }
        !empty
    });
}
