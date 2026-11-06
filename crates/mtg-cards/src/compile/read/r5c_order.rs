//! The order of the words of a selector: a selector is a conjunction, so "creature cards in your graveyard" holds the
//! same cards whatever order its parts come in, but the writers name a group from the order the readers give
//! (types, then the zone, then the owner or controller, then the chosen type or color). A translation that lists the
//! same parts in another order is written in the readers' order.

use serde_json::Value;

use crate::ability::Ability;

/// What the card's own next untap step is: its controller's is yours, and a card may say either.
const R5C_YOUR_STEP: &str = "~ doesn't untap during your next untap step";

/// The card held down through "its controller's next untap step" ("it" after a trigger names the card) beside "your".
pub(super) fn r5c_held_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts {
        let Some(at) = text.find(R5C_YOUR_STEP) else {
            continue;
        };
        more.push(text.replacen(
            R5C_YOUR_STEP,
            "~ doesn't untap during its controller's next untap step",
            1,
        ));
        // "it" stands for the card only after words that name the card ("Whenever ~ attacks, it ...").
        let since_name = text[..at].rfind('~').map(|name| &text[name..at]);
        if since_name.is_some_and(|words| !words.contains('.') && !words.contains("target")) {
            more.push(text.replacen(
                R5C_YOUR_STEP,
                "it doesn't untap during its controller's next untap step",
                1,
            ));
        }
    }
    more
}

/// Where a part of a selector goes in the readers' order; 0 keeps its place before the parts named here.
fn rank(part: &Value) -> u8 {
    match part {
        Value::String(name) if name == "ChosenSubtype" || name == "ChosenColor" => 4,
        Value::Object(fields) if fields.len() == 1 => {
            match fields.keys().next().map_or("", String::as_str) {
                "In" => 1,
                "OwnedBy" => 2,
                "ControlledBy" => 3,
                _ => 0,
            }
        }
        _ => 0,
    }
}

/// Whether a part names a zone inside another part ("in your hand or graveyard"): the engine looks in the first zone a
/// selector names, so a selector with such a part keeps its order.
fn nested_zone(part: &Value) -> bool {
    rank(part) == 0 && part.to_string().contains("{\"In\":")
}

/// Puts the parts of each selector in the readers' order; true when something moved. `in_place` orders only the
/// parts named in `rank` among the places they hold ("creatures you control with flying" keeps its keyword last).
fn order(value: &mut Value, in_place: bool) -> bool {
    match value {
        Value::Array(parts) => {
            let mut moved = false;
            for part in parts.iter_mut() {
                moved |= order(part, in_place);
            }
            if parts.iter().any(|part| rank(part) > 0) && !parts.iter().any(nested_zone) {
                let before = parts.clone();
                if in_place {
                    let places: Vec<usize> = (0..parts.len())
                        .filter(|&at| rank(&parts[at]) > 0)
                        .collect();
                    let mut named: Vec<Value> =
                        places.iter().map(|&at| parts[at].clone()).collect();
                    named.sort_by_key(rank);
                    for (at, part) in places.into_iter().zip(named) {
                        parts[at] = part;
                    }
                } else {
                    parts.sort_by_key(rank);
                }
                moved |= *parts != before;
            }
            moved
        }
        Value::Object(fields) => {
            let mut moved = false;
            for (_, field) in fields.iter_mut() {
                moved |= order(field, in_place);
            }
            moved
        }
        _ => false,
    }
}

/// The ability with the parts of its selectors in the readers' order, in place and all after the other parts; empty
/// when they are in that order already.
pub(super) fn r5c_reordered(ability: &Ability) -> Vec<Ability> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let mut orders: Vec<Value> = Vec::new();
    for in_place in [true, false] {
        let mut ordered = value.clone();
        if order(&mut ordered, in_place) && !orders.contains(&ordered) {
            orders.push(ordered);
        }
    }
    orders
        .into_iter()
        .filter_map(|ordered| serde_json::from_value(ordered).ok())
        .collect()
}
