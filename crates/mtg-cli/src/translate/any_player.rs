//! Readings that must not narrow or widen what the card says: "any opponent may" is a payment asked in turn until one pays, and a controller the script adds to a target is a gap.

use serde_json::{Value, json};

use super::ctx::{Tr, unmapped};

/// Players that a "may" or a payment can name several of.
fn several(who: &Value) -> bool {
    matches!(who.as_str(), Some("Opponents" | "EachPlayer"))
}

/// The effects with a "may sacrifice" by several players written as a payment of the sacrifice by them: a `May` asks until
/// one player says yes and then does its effects for all of the players it names, so a yes would make every one of them
/// sacrifice. A `Pay` takes the cost from the one who pays, and the steps after it that ask whether a sacrifice happened go
/// into its `paid` list. A sacrifice the later steps read in any other way is a gap.
pub fn pay_for_any(mut effects: Vec<Value>) -> Tr<Vec<Value>> {
    let Some(at) = effects.iter().position(is_shared_sacrifice) else {
        return Ok(effects);
    };
    let may = effects.remove(at);
    let who = may["May"]["who"].clone();
    let sacrifice = &may["May"]["effects"][0]["Sacrifice"];
    let name = sacrifice["as"].as_str().map(str::to_owned);
    let mut item = json!({"filter": sacrifice["filter"].clone()});
    if let Some(count) = sacrifice.get("count").filter(|count| **count != json!(1)) {
        item["count"] = count.clone();
    }
    let mut paid = Vec::new();
    if let Some(name) = &name {
        let moved = format!("{name}~moved");
        while let Some(step) = effects.get(at) {
            let Some(then) = did_it(step, &moved) else {
                break;
            };
            paid.extend(then);
            effects.remove(at);
        }
        if effects.iter().any(|step| step.to_string().contains(name)) {
            return unmapped("key:Sacrifice.Optional-several-players-read");
        }
    }
    let mut pay = json!({"Pay": {"who": who, "cost": [{"Sacrifice": item}]}});
    if !paid.is_empty() {
        pay["Pay"]["paid"] = Value::Array(paid);
    }
    effects.insert(at, pay);
    Ok(effects)
}

/// A `May` by several players around the sacrifice by the same players.
fn is_shared_sacrifice(step: &Value) -> bool {
    let Some(may) = step.get("May") else {
        return false;
    };
    let (Some(who), Some([inner])) = (may.get("who"), may["effects"].as_array().map(Vec::as_slice))
    else {
        return false;
    };
    several(who)
        && inner
            .get("Sacrifice")
            .is_some_and(|s| s.get("who") == Some(who))
}

/// The steps of an `If` that holds when at least one object was moved, as the name of the count reads it.
fn did_it(step: &Value, moved: &str) -> Option<Vec<Value>> {
    let branch = step.get("If")?;
    let test = branch.pointer("/condition/Compare")?;
    let counts = test.get("left") == Some(&json!({ "Bound": moved }))
        && test.get("op") == Some(&json!("Ge"))
        && test.get("right") == Some(&json!(1));
    (counts && branch.get("otherwise").is_none()).then(|| branch["then"].as_array().cloned())?
}

/// Words of a target phrase that say whose the target is or where it is, so a controller in the script is not Forge's own
/// narrowing of the target.
const WHOSE: [&str; 12] = [
    "control",
    "your",
    " own",
    "opponent",
    "attacking",
    "blocking",
    "that player",
    "defending",
    "its controller",
    "enchanted",
    "equipped",
    "each player",
];

/// A target filter with a controller (`YouCtrl`, `OppCtrl`) that no target phrase of the Oracle text states: the script
/// narrows the target as a hint for its AI, so the card would not play as printed.
pub fn narrows_target(oracle: &str, valid: &str) -> bool {
    let controller = |part: &str| part == "YouCtrl" || part == "OppCtrl";
    let named = valid
        .split(',')
        .flat_map(|alternative| alternative.split(['.', '+']))
        .any(controller);
    if !named {
        return false;
    }
    let text = oracle.replace("\\n", "\n").to_lowercase();
    let phrases: Vec<&str> = text.split("target ").skip(1).collect();
    !phrases.is_empty()
        && !phrases.iter().any(|phrase| {
            let window = phrase
                .split(['.', ':', ';', '\n'])
                .next()
                .unwrap_or_default();
            WHOSE.iter().any(|word| window.contains(word))
        })
}
