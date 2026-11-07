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

/// A target filter with no controller where every target phrase of the Oracle text says "you control" ("Target creature
/// you control becomes ..." over `ValidTgts$ Creature`): the script would let the card target what it can't.
pub fn widens_target(oracle: &str, valid: &str) -> bool {
    if valid.is_empty()
        || ["Ctrl", "Control", "Own"]
            .iter()
            .any(|word| valid.contains(word))
    {
        return false;
    }
    // One plain type ("Creature", "Permanent"), as the target phrases name it.
    let noun = valid.to_lowercase();
    if !noun.chars().all(char::is_alphabetic) {
        return false;
    }
    let text = without_reminders(&oracle.replace("\\n", "\n").to_lowercase());
    let phrases: Vec<Vec<&str>> = text
        .split("target ")
        .skip(1)
        .map(|phrase| phrase.split_whitespace().take(3).collect())
        .filter(|words: &Vec<&str>| {
            words
                .first()
                .is_some_and(|first| first.trim_end_matches([',', '.', ':', ';']) == noun)
        })
        .collect();
    // Each phrase of that type is "target <type> you control": the controller belongs to the target, not to a later
    // clause ("for as long as you control ~", "where X is the number of Gates you control").
    !phrases.is_empty()
        && phrases.iter().all(|words| {
            matches!(words.as_slice(), [_, "you", control] if control.trim_end_matches([',', '.']) == "control")
        })
}

/// The text without its reminder text in parentheses.
fn without_reminders(text: &str) -> String {
    let mut depth = 0usize;
    text.chars()
        .filter(|c| {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    return false;
                }
                _ => {}
            }
            depth == 0
        })
        .collect()
}

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

/// The words of a filter that already set the target apart from another object: the source, a triggering or chosen object.
const SET_APART: [&str; 10] = [
    "Other",
    "Self",
    "Triggered",
    "Defined",
    "Remembered",
    "Targeted",
    "Enchanted",
    "Equipped",
    "Attached",
    "Chosen",
];

/// A target of a permanent's ability whose filter leaves the source in while every target phrase of the Oracle text says
/// "other target" or "another target": no earlier target is there to be other than, so the target is other than the
/// source (Soul of Emancipation's script reads any nonland permanent).
pub fn omits_other(types: &str, oracle: &str, valid: &str) -> bool {
    if valid.is_empty()
        || types.contains("Instant")
        || types.contains("Sorcery")
        || SET_APART.iter().any(|word| valid.contains(word))
    {
        return false;
    }
    let text = without_reminders(&oracle.replace("\\n", "\n").to_lowercase());
    let words: Vec<&str> = text.split_whitespace().collect();
    let before: Vec<Option<&str>> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == "target")
        .map(|(at, _)| at.checked_sub(1).and_then(|at| words.get(at).copied()))
        .collect();
    !before.is_empty()
        && before
            .iter()
            .all(|word| matches!(word, Some("other" | "another")))
}

/// Whether every "... with the same name" phrase of the Oracle text names creatures ("each other creature with the same
/// name as that creature"): a script's `Targeted.sameName` names no type and would take any permanent.
pub fn same_name_creatures(oracle: &str) -> bool {
    let text = without_reminders(&oracle.replace("\\n", "\n").to_lowercase());
    let phrases: Vec<&str> = text
        .split(" with the same name")
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, heads)| heads.to_vec())
        .unwrap_or_default();
    !phrases.is_empty()
        && phrases.iter().all(|head| {
            matches!(
                head.split_whitespace().next_back(),
                Some("creature" | "creatures")
            )
        })
}

/// The filter with `Other` added to each alternative.
pub fn with_other(valid: &str) -> String {
    valid
        .split(',')
        .map(|alternative| {
            let join = if alternative.contains('.') { '+' } else { '.' };
            format!("{alternative}{join}Other")
        })
        .collect::<Vec<_>>()
        .join(",")
}
