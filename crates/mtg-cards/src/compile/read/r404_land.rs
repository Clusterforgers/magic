//! r40-4 land type and snow changes and their writers: "target snow land is no longer snow", "target nonsnow permanent
//! becomes snow until end of turn" (`AddSupertype`/`RemoveSupertype` of Snow), "target land becomes a Forest until this
//! creature leaves the battlefield" (`WhileSourceOnBattlefield`), "until its controller's next untap step"
//! (`UntilItsControllersNextUntapStep`) and "Until end of turn, target land you control becomes the basic land type of
//! your choice in addition to its other types" (`AddLandTypeChosen`).
//!
//! Each is read from a twin sentence another reader knows ("gains flying", "until end of turn", "becomes the basic land
//! type of your choice until end of turn") with the one part changed back, and written from the twin's wordings; the
//! twin part is chosen so that the printed text cannot hold it.

use serde_json::{Value, json};

use super::tokens::same_text;
use crate::ability::Ability;

/// What a twin changes in the one kind of continuous effect it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Snow is added: the twin gains flying.
    SnowAdded,
    /// Snow is taken away: the twin loses flying.
    SnowRemoved,
    /// The duration: the twin lasts until end of turn.
    Lasts(&'static str),
    /// The chosen land type is added: the twin sets it.
    ChosenAdded,
}

/// A printed phrase, its twin and the change between them; a duration of `None` keeps the twin's.
struct Rule {
    printed: &'static str,
    twin: &'static str,
    change: Change,
    /// The duration the printed effect has instead of the twin's "until end of turn", for a snow change.
    duration: Option<&'static str>,
}

const RULES: [Rule; 8] = [
    Rule {
        printed: " becomes snow until end of turn",
        twin: " gains flying until end of turn",
        change: Change::SnowAdded,
        duration: None,
    },
    Rule {
        printed: " isn't snow until end of turn",
        twin: " loses flying until end of turn",
        change: Change::SnowRemoved,
        duration: None,
    },
    Rule {
        printed: " becomes snow.",
        twin: " gains flying until end of turn.",
        change: Change::SnowAdded,
        duration: Some("Forever"),
    },
    Rule {
        printed: " is no longer snow.",
        twin: " loses flying until end of turn.",
        change: Change::SnowRemoved,
        duration: Some("Forever"),
    },
    Rule {
        printed: " until ~ leaves the battlefield",
        twin: " until end of turn",
        change: Change::Lasts("WhileSourceOnBattlefield"),
        duration: None,
    },
    Rule {
        printed: " until this creature leaves the battlefield",
        twin: " until end of turn",
        change: Change::Lasts("WhileSourceOnBattlefield"),
        duration: None,
    },
    Rule {
        printed: " until its controller's next untap step",
        twin: " until end of turn",
        change: Change::Lasts("UntilItsControllersNextUntapStep"),
        duration: None,
    },
    Rule {
        printed: " becomes the basic land type of your choice in addition to its other types",
        twin: " becomes the basic land type of your choice until end of turn",
        change: Change::ChosenAdded,
        duration: None,
    },
];

/// What the reader reads in place of the chosen land type added.
const CHOSEN_TWIN: &str = " becomes a Swamp until end of turn";

/// The name the chosen land type is bound to.
const CHOSEN: &str = "chosen";

/// Turns the one continuous effect that makes a land a Swamp for the turn into the choice of a basic land type and the
/// effect that adds it.
fn chosen_from_swamp(value: &mut Value) -> Option<()> {
    let swamp = (json!([{ "SetLandTypes": ["Swamp"] }]), json!("UntilEndOfTurn"));
    let added = (json!([{ "AddLandTypeChosen": CHOSEN }]), json!("UntilEndOfTurn"));
    let mut paths = Vec::new();
    continuous_paths(value, &mut Vec::new(), &mut paths);
    let mut found = paths
        .into_iter()
        .filter(|path| at_path(value, path).is_some_and(|body| body.get("mods") == Some(&swamp.0)));
    let path = found.next()?;
    found.next().is_none().then_some(())?;
    convert_body(at_path(value, &path)?, &swamp, &added).then_some(())?;
    // The effect is a step of a list: the choice goes right before it.
    let [parent @ .., Step::Index(at), Step::Key(_)] = path.as_slice() else {
        return None;
    };
    let steps = at_path(value, parent)?.as_array_mut()?;
    steps.insert(
        *at,
        json!({ "ChooseValue": { "kind": "BasicLandType", "as": CHOSEN } }),
    );
    Some(())
}

/// The leading duration of the chosen land type's sentence, which its twin says at the end.
const LEADING: &str = "Until end of turn, ";

/// The twin mods and duration of a change, as JSON, and the printed ones.
fn forms(change: Change, duration: Option<&str>) -> Option<((Value, Value), (Value, Value))> {
    let end = json!("UntilEndOfTurn");
    let kept = |d: Option<&str>| d.map_or_else(|| end.clone(), |d| json!(d));
    Some(match change {
        Change::SnowAdded => (
            (json!([{ "AddKeyword": "Flying" }]), end.clone()),
            (json!([{ "AddSupertype": "Snow" }]), kept(duration)),
        ),
        Change::SnowRemoved => (
            (json!([{ "RemoveKeyword": "Flying" }]), end.clone()),
            (json!([{ "RemoveSupertype": "Snow" }]), kept(duration)),
        ),
        Change::Lasts(to) => ((Value::Null, end), (Value::Null, json!(to))),
        Change::ChosenAdded => ((json!("SetLandTypesChosen"), end.clone()), (json!("AddLandTypeChosen"), end)),
    })
}

/// The paths of the continuous effects of a JSON value, each a list of keys and indexes.
fn continuous_paths(value: &Value, at: &mut Vec<Step>, found: &mut Vec<Vec<Step>>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("Continuous") {
                let mut path = at.clone();
                path.push(Step::Key("Continuous".to_owned()));
                found.push(path);
                return;
            }
            for (key, child) in map {
                at.push(Step::Key(key.clone()));
                continuous_paths(child, at, found);
                at.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                at.push(Step::Index(index));
                continuous_paths(child, at, found);
                at.pop();
            }
        }
        _ => {}
    }
}

/// One step of a path into a JSON value.
#[derive(Debug, Clone)]
enum Step {
    Key(String),
    Index(usize),
}

/// The value at a path.
fn at_path<'a>(value: &'a mut Value, path: &[Step]) -> Option<&'a mut Value> {
    let mut here = value;
    for step in path {
        here = match step {
            Step::Key(key) => here.get_mut(key.as_str())?,
            Step::Index(index) => here.get_mut(*index)?,
        };
    }
    Some(here)
}

/// Changes one continuous effect body from the form `from` into the form `to`; whether it did.
fn convert_body(body: &mut Value, from: &(Value, Value), to: &(Value, Value)) -> bool {
    if body.get("duration") != Some(&from.1) {
        return false;
    }
    let mods = body.get("mods").cloned().unwrap_or(Value::Null);
    let matched = match (&from.0, &to.0) {
        (Value::Null, _) => true,
        (Value::String(set), Value::String(add)) => {
            let list = mods.as_array().cloned().unwrap_or_default();
            match list.as_slice() {
                [only] => match only.get(set.as_str()) {
                    Some(name) => {
                        body["mods"] = json!([{ add.as_str(): name.clone() }]);
                        true
                    }
                    None => false,
                },
                _ => false,
            }
        }
        (want, put) => {
            if mods == *want {
                body["mods"] = put.clone();
                true
            } else {
                false
            }
        }
    };
    if matched {
        body["duration"] = to.1.clone();
    }
    matched
}

/// Changes each continuous effect of `value` in the form `from` into the form `to`; how many it changed.
fn convert_by_path(value: &mut Value, from: &(Value, Value), to: &(Value, Value)) -> usize {
    let mut paths = Vec::new();
    continuous_paths(value, &mut Vec::new(), &mut paths);
    paths
        .iter()
        .filter(|path| at_path(value, path).is_some_and(|body| convert_body(body, from, to)))
        .count()
}

/// The text with the leading "Until end of turn, " of the chosen land type moved to the end of its sentence.
fn moved_duration(text: &str) -> Option<String> {
    let at = text.find(LEADING)?;
    let rest = &text[at + LEADING.len()..];
    let mut chars = rest.chars();
    let first = chars.next()?.to_ascii_uppercase();
    Some(format!("{}{first}{}", &text[..at], chars.as_str()))
}

/// The same text with "Until end of turn, " put before the sentence that ends with `phrase`, which it leaves.
fn leading_duration(text: &str, phrase: &str) -> Option<String> {
    let end = text.find(phrase)?;
    let start = text[..end]
        .rfind([':', '.'])
        .map_or(0, |at| at + 1 + usize::from(text[at + 1..].starts_with(' ')));
    let sentence = &text[start..end];
    let mut chars = sentence.chars();
    let first = chars.next()?.to_ascii_lowercase();
    Some(format!(
        "{}{LEADING}{first}{}{}",
        &text[..start],
        chars.as_str(),
        &text[end..]
    ))
}

/// The rules whose printed phrase the text holds, at most one for each change.
fn rules_in(text: &str) -> Vec<&'static Rule> {
    let mut found: Vec<&'static Rule> = Vec::new();
    for rule in &RULES {
        if text.matches(rule.printed).count() == 1
            && !found.iter().any(|seen| seen.change == rule.change)
        {
            found.push(rule);
        }
    }
    found
}

/// Reads a clause with land type or snow changes from its twin: `read` reads the twin's text.
pub(super) fn r404_land_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let rules = rules_in(text);
    if rules.is_empty() || text.contains("flying") {
        return None;
    }
    let mut twin = text.to_owned();
    for rule in &rules {
        if rule.change == Change::ChosenAdded {
            // The readers know a land made one basic land type: Swamp stands for the type chosen.
            (!text.contains("Swamp")).then_some(())?;
            twin = moved_duration(&twin.replacen(rule.printed, CHOSEN_TWIN, 1))?;
        } else {
            twin = twin.replacen(rule.printed, rule.twin, 1);
        }
    }
    // A twin duration stands for one "until end of turn" only.
    if rules.iter().any(|rule| matches!(rule.change, Change::Lasts(_)))
        && twin.matches(" until end of turn").count() != 1
    {
        return None;
    }
    let mut value = serde_json::to_value(read(&twin)?).ok()?;
    for rule in &rules {
        if rule.change == Change::ChosenAdded {
            chosen_from_swamp(&mut value)?;
            continue;
        }
        let (from, to) = forms(rule.change, rule.duration)?;
        (convert_by_path(&mut value, &from, &to) == 1).then_some(())?;
    }
    serde_json::from_value(value).ok()
}

/// The wordings of an ability with land type or snow changes: the twin's wordings with each twin phrase put back.
pub(super) fn r404_land_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let said = value.to_string();
    if said.contains("Flying") || said.matches("UntilEndOfTurn").count() > 2 {
        return;
    }
    for rule in &RULES {
        let Some((from, to)) = forms(rule.change, rule.duration) else {
            continue;
        };
        let mut twin = value.clone();
        // The printed form back to the twin form: one effect only.
        if convert_by_path(&mut twin, &to, &from) != 1 {
            continue;
        }
        // A duration's twin is the only effect that lasts until end of turn.
        if matches!(rule.change, Change::Lasts(_))
            && twin.to_string().matches("UntilEndOfTurn").count() != 1
        {
            continue;
        }
        let Ok(twin) = serde_json::from_value::<Ability>(twin) else {
            continue;
        };
        for text in written(&twin) {
            if text.matches(rule.twin).count() != 1
                && !(rule.printed.ends_with('.') && text.ends_with(rule.twin.trim_end_matches('.')))
            {
                continue;
            }
            let back = if rule.printed.ends_with('.') && !text.ends_with('.') {
                format!("{text}.").replacen(rule.twin, rule.printed, 1)
            } else {
                text.replacen(rule.twin, rule.printed, 1)
            };
            let back = if rule.change == Change::ChosenAdded {
                match leading_duration(&back, rule.printed) {
                    Some(moved) => moved,
                    None => continue,
                }
            } else {
                back
            };
            if !texts.iter().any(|seen| same_text(seen, &back)) {
                texts.push(back);
            }
        }
    }
}
