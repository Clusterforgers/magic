//! A group the text names where the file wrote something else: the controller of a player in an object slot ("untap all
//! lands you control" as `ControllerOf: You`) and a bare target as the subject of a static ability ("Creatures you
//! control have lifelink" with no target declared). The group is the one the printed words name, read by the reader the
//! compiler uses for printed text, when the text names exactly one.

use serde_json::{Value, json};

use super::targets::scoped;
use super::text::{ability_text, plain_lines};
use super::{EFFECT_ABILITIES, Repair, RepairClass, Site, body_of};
use crate::compile::{all_groups, subject_group};
use crate::refs::Selector;
use crate::support::PrintedFace;

/// The fields of an effect that hold the object it acts on.
const OBJECT_SLOTS: [&str; 3] = ["what", "affected", "on"];

/// Whether the group is of the players' own things, when the file wrote the controller of this player.
fn controlled_by(group: &Selector, player: &str) -> bool {
    serde_json::to_value(group).is_ok_and(|value| {
        value.as_array().is_some_and(|preds| {
            preds
                .iter()
                .any(|pred| *pred == json!({ "ControlledBy": player }))
        })
    })
}

/// The player inside an object slot `{"ControllerOf": player}`.
fn controller_player(value: &Value) -> Option<&str> {
    let map = value.as_object().filter(|map| map.len() == 1)?;
    map.get("ControllerOf")?
        .as_str()
        .filter(|player| ["You", "Opponents"].contains(player))
}

/// The object slots of the effects that hold the controller of a player.
fn controller_slots(effects: &mut [Value]) -> Vec<&mut Value> {
    effects
        .iter_mut()
        .filter_map(|effect| body_of(effect).map(|(_, fields)| fields))
        .flat_map(|fields| fields.iter_mut())
        .filter(|(key, value)| {
            OBJECT_SLOTS.contains(&key.as_str()) && controller_player(value).is_some()
        })
        .map(|(_, value)| value)
        .collect()
}

/// Puts the group the text names where the effects wrote the controller of a player.
pub(super) fn controller_group(ability: &mut Value, site: &Site<'_>, repairs: &mut Vec<Repair>) {
    let Some((name, body)) = body_of(ability) else {
        return;
    };
    if !EFFECT_ABILITIES.contains(&name) || body.values().any(scoped) {
        return;
    }
    let Some(text) = ability_text(site.face, name, site.nth, site.count) else {
        return;
    };
    let Some(Value::Array(effects)) = body.get_mut("effects") else {
        return;
    };
    let mut slots = controller_slots(effects);
    let [slot] = &mut slots[..] else {
        return;
    };
    let Some(player) = controller_player(slot).map(str::to_owned) else {
        return;
    };
    let groups: Vec<Selector> = all_groups(&text)
        .into_iter()
        .filter(|group| controlled_by(group, &player))
        .collect();
    let [group] = &groups[..] else {
        return;
    };
    let Ok(group) = serde_json::to_value(group) else {
        return;
    };
    **slot = json!({ "Each": group });
    repairs.push(Repair {
        class: RepairClass::ControllerGroup,
        at: site.at.to_owned(),
        note: "the controller of a player is the group the printed words name".to_owned(),
    });
}

/// Whether the subject of a static ability is a bare reference to a target the ability does not declare.
fn bare_target(body: &serde_json::Map<String, Value>) -> bool {
    body.get("targets")
        .is_none_or(|targets| targets.as_array().is_some_and(Vec::is_empty))
        && body
            .get("affected")
            .and_then(Value::as_object)
            .is_some_and(|map| map.len() == 1 && map.contains_key("Target"))
}

/// Makes the bare target of each static ability the group its printed line starts with, when the static abilities with a
/// bare target are as many as the printed lines that start with a group; they pair in order.
pub(super) fn static_subjects(
    abilities: &mut [Value],
    face: &PrintedFace,
    at: &str,
    repairs: &mut Vec<Repair>,
) {
    let broken: Vec<usize> = abilities
        .iter()
        .enumerate()
        .filter(|(_, ability)| {
            ability
                .get("Static")
                .and_then(Value::as_object)
                .is_some_and(bare_target)
        })
        .map(|(index, _)| index)
        .collect();
    let groups: Vec<Value> = plain_lines(&face.oracle_text)
        .iter()
        .filter_map(|line| subject_group(line))
        .filter_map(|group| serde_json::to_value(group).ok())
        .collect();
    if broken.len() != groups.len() {
        return;
    }
    for (index, group) in broken.into_iter().zip(groups) {
        let Some((_, body)) = abilities.get_mut(index).and_then(body_of) else {
            continue;
        };
        body.insert("affected".to_owned(), json!({ "Each": group }));
        repairs.push(Repair {
            class: RepairClass::StaticSubject,
            at: format!("{at}.abilities[{index}]"),
            note: "a static ability with a bare target is the group its printed line starts with"
                .to_owned(),
        });
    }
}
