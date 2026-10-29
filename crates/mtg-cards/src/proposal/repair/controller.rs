//! "The controller of" a player is the player: where an effect or an event names a player (`who`, `to`), a
//! `ControllerOf` around a player reference only wraps an object kind around it.

use serde_json::Value;

use super::{Repair, RepairClass};

/// The fields that hold a player.
const PLAYER_SLOTS: [&str; 3] = ["who", "to", "player"];

/// The references to a player that need no object.
const PLAYERS: [&str; 7] = [
    "You",
    "Opponents",
    "EachPlayer",
    "AttachedPlayer",
    "Chosen",
    "Attacked",
    "Defending",
];

/// The player inside `{"ControllerOf": player}`.
fn unwrapped(value: &Value) -> Option<&str> {
    let map = value.as_object().filter(|map| map.len() == 1)?;
    map.get("ControllerOf")?
        .as_str()
        .filter(|player| PLAYERS.contains(player))
}

/// Unwraps the controller of a player in a player field, and says how many it unwrapped.
fn unwrap_all(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut count = 0;
            for (key, inner) in map.iter_mut() {
                let player = PLAYER_SLOTS
                    .contains(&key.as_str())
                    .then(|| unwrapped(inner).map(str::to_owned))
                    .flatten();
                match player {
                    Some(player) => {
                        *inner = Value::String(player);
                        count += 1;
                    }
                    None => count += unwrap_all(inner),
                }
            }
            count
        }
        Value::Array(items) => items.iter_mut().map(unwrap_all).sum(),
        _ => 0,
    }
}

/// Repairs one ability.
pub(super) fn repair(ability: &mut Value, at: &str, repairs: &mut Vec<Repair>) {
    let count = unwrap_all(ability);
    if count > 0 {
        repairs.push(Repair {
            class: RepairClass::ControllerOfPlayer,
            at: at.to_owned(),
            note: format!("{count} `ControllerOf` around a player in a player field: the player"),
        });
    }
}
