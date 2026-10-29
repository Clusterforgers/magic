//! The identity and the faces of a proposal come from the imported card: ids and names are the card's, a card with one
//! face has one face (the abilities a model spread over several are one list), a card with more faces than the file
//! has gets the missing ones empty.

use serde_json::{Map, Value, json};

use super::{Repair, RepairClass};
use crate::support::PrintedCard;

/// The keys of a face that a merge may drop or combine.
const MERGEABLE: [&str; 3] = ["abilities", "name", "oracle_fingerprint"];

/// Sets the oracle id and the names to the card's and makes the number of faces the card's.
pub(super) fn repair(
    object: &mut Map<String, Value>,
    printed: &PrintedCard,
    repairs: &mut Vec<Repair>,
) {
    for (key, shown) in [("oracle_id", &printed.oracle_id), ("name", &printed.name)] {
        if let Some(given) = object.get(key)
            && given.as_str() != Some(shown.as_str())
        {
            repairs.push(Repair {
                class: RepairClass::Identity,
                at: key.to_owned(),
                note: format!("{given} replaced by the imported card's {shown:?}"),
            });
            object.insert(key.to_owned(), shown.clone().into());
        }
    }
    let Some(faces) = object.get_mut("faces").and_then(Value::as_array_mut) else {
        return;
    };
    if printed.faces.len() == 1 && faces.len() > 1 {
        merge(faces, printed, repairs);
    }
    while !faces.is_empty() && faces.len() < printed.faces.len() {
        repairs.push(Repair {
            class: RepairClass::Faces,
            at: format!("faces[{}]", faces.len()),
            note: "the file has fewer faces than the card: an empty face is added".to_owned(),
        });
        faces.push(json!({}));
    }
    for (index, (face, shown)) in faces.iter_mut().zip(&printed.faces).enumerate() {
        let Some(face) = face.as_object_mut() else {
            continue;
        };
        if let Some(given) = face.get("name")
            && given.as_str() != Some(shown.name.as_str())
        {
            repairs.push(Repair {
                class: RepairClass::Identity,
                at: format!("faces[{index}].name"),
                note: format!(
                    "{given} replaced by the imported face name {:?}",
                    shown.name
                ),
            });
            face.insert("name".to_owned(), shown.name.clone().into());
        }
    }
}

/// A card of one face whose file lists several: each extra face holds abilities (and perhaps the card's name), so they
/// are the abilities of the one face, in order.
fn merge(faces: &mut Vec<Value>, printed: &PrintedCard, repairs: &mut Vec<Repair>) {
    let name = printed.faces.first().map(|shown| shown.name.as_str());
    let plain = faces.iter().all(|face| {
        face.as_object().is_some_and(|face| {
            face.keys().all(|key| MERGEABLE.contains(&key.as_str()))
                && face.get("name").is_none_or(|given| {
                    given.as_str() == name || given.as_str().is_some_and(str::is_empty)
                })
        })
    });
    if !plain {
        return;
    }
    let abilities: Vec<Value> = faces
        .iter()
        .filter_map(|face| face.get("abilities").and_then(Value::as_array))
        .flatten()
        .cloned()
        .collect();
    let count = faces.len();
    faces.truncate(1);
    if let Some(face) = faces.first_mut().and_then(Value::as_object_mut) {
        face.insert("abilities".to_owned(), abilities.into());
    }
    repairs.push(Repair {
        class: RepairClass::Faces,
        at: "faces".to_owned(),
        note: format!("the card has one face and the file {count}: their abilities are one list"),
    });
}
