//! A keyword a card gives under a condition the renderers do not word ("As long as this creature has three or more
//! +1/+1 counters on it, it has flying.", "This creature has trample as long as it has a shield counter on it."): the
//! ability is written under "it's your turn" and the condition's words are put in its place.

use super::w4b_stand::{json, stand, swapped, word};
use super::w21_controls::controls;
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of a static ability whose condition one of the phrases words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        condition: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let condition = json(condition);
    let Some(stand) = stand(ability, |value| {
        *value.pointer_mut("/Static/condition")? = json!("YourTurn");
        Some(())
    }) else {
        return;
    };
    if condition == json!({"Not": "YourTurn"}) {
        texts.extend(swapped(
            &stand,
            &[("During your turn", "During turns other than yours")],
        ));
        return;
    }
    let Some((trailing, leading)) = phrases(&condition) else {
        return;
    };
    texts.extend(swapped(
        &stand,
        &[(
            "as long as it's your turn",
            &format!("as long as {trailing}"),
        )],
    ));
    let head = format!("As long as {leading}");
    texts.extend(swapped(&stand, &[("As long as it's your turn", &head)]));
    // "As long as this creature is renowned, it has menace."
    if leading.starts_with("~ ") {
        texts.extend(swapped(
            &stand,
            &[("As long as it's your turn, ~", &format!("{head}, it"))],
        ));
    }
}

/// The words of the condition after "as long as" at the end of the sentence and at its head: the card is "it" after its
/// own name and "~" before it.
fn phrases(condition: &Value) -> Option<(String, String)> {
    let said = |trailing: &str, leading: &str| Some((trailing.to_owned(), leading.to_owned()));
    if let Some(selector) = condition.pointer("/Matches/selector") {
        (condition.pointer("/Matches/subject")? == "This").then_some(())?;
        let [only] = selector.as_array()?.as_slice() else {
            return None;
        };
        return match only {
            Value::String(state) if state == "Renowned" => said("it's renowned", "~ is renowned"),
            Value::Object(map) => {
                if let Some(counters) = map.get("CounterCount") {
                    let (trailing, leading) = counters_held(counters)?;
                    return said(&trailing, &leading);
                }
                if let Some(kind) = map.get("HasCounter").and_then(|kind| kind.get("Other")) {
                    // A shield counter alone: a card with an ice counter prints a damage prevention in the same sentence.
                    let kind = kind.as_str().filter(|kind| *kind == "shield")?;
                    let held = format!("has a {kind} counter on it");
                    return said(&format!("it {held}"), &format!("~ {held}"));
                }
                (map.get("HasKeyword")? == "Defender").then_some(())?;
                said("it has defender", "~ has defender")
            }
            _ => None,
        };
    }
    if condition.pointer("/Compare/left/Count/0") == Some(&json!("This"))
        && condition.pointer("/Compare/left/Count/1") == Some(&json!("EnteredThisTurn"))
    {
        return said("it entered this turn", "~ entered this turn");
    }
    let compare = condition.get("Compare")?;
    let enchanted = json!(["This", {"HasAttached": [{"Subtype": "Aura"}]}]);
    if compare.pointer("/left/Count") == Some(&enchanted)
        && compare.get("op")? == "Ge"
        && *compare.get("right")? == 1
    {
        return said("~ is enchanted", "~ is enchanted");
    }
    if compare.get("op")? == "Ge" {
        let many = word(compare.get("right")?);
        let left = compare.get("left")?;
        if let Some(counters) = left.get("HighestCounters") {
            let poison = json!({"counter": "poison", "who": "Opponents"});
            (*counters == poison).then_some(())?;
            let phrase = format!("an opponent has {} or more poison counters", many?);
            return Some((phrase.clone(), phrase));
        }
        if let Some(phrase) = turn_events(left, many) {
            return Some((phrase.clone(), phrase));
        }
    }
    if let Some(phrase) = no_spell(compare) {
        return Some((phrase.clone(), phrase));
    }
    if let Some(phrase) = controls(compare) {
        return Some((phrase.clone(), phrase));
    }
    let half = json!({"left": {"Times": {"by": 2, "of": {"Life": "You"}}}, "op": "Le", "right": {"StartingLife": "You"}});
    (*compare == half).then_some(())?;
    let phrase = "your life total is less than or equal to half your starting life total";
    said(phrase, phrase)
}

/// "it has three or more +1/+1 counters on it": the counters held, from "at least" (Ge) or "more than" (Gt).
fn counters_held(counters: &Value) -> Option<(String, String)> {
    let number = counters.get("to")?.as_u64()?;
    let least = match counters.get("op")?.as_str()? {
        "Ge" => number,
        "Gt" => number + 1,
        _ => return None,
    };
    let kind = counters.get("kind")?.as_str()?;
    let many = word(&json!(least))?;
    let held = format!("has {many} or more {kind} counters on it");
    Some((format!("it {held}"), format!("~ {held}")))
}

/// What happened this turn, at least `many` times: "you've cast two or more noncreature spells this turn".
fn turn_events(left: &Value, many: Option<&'static str>) -> Option<String> {
    if let Some(tally) = left.get("Tally") {
        let filter = tally.get("filter")?;
        let (event, who) = (tally.get("event")?, tally.get("who")?);
        (*who == "EachPlayer").then_some(())?;
        let (spells, entered) = (
            json!([{"Not": {"Type": "Creature"}}, {"ControlledBy": "You"}]),
            json!([{"Not": {"Type": "Land"}}, {"ControlledBy": "You"}]),
        );
        if *event == "SpellCast" && *filter == spells {
            return Some(format!(
                "you've cast {} or more noncreature spells this turn",
                many?
            ));
        }
        if *event == "Entered" && *filter == entered {
            return Some(format!(
                "{} or more nonland permanents entered the battlefield under your control this turn",
                many?
            ));
        }
        return None;
    }
    let done = json!({"what": "DungeonsCompleted", "who": "You"});
    (left.get("Tallied") == Some(&done) && many == Some("one"))
        .then(|| "you've completed a dungeon".to_owned())
}

/// "you haven't cast a spell this turn", "you've cast an instant or sorcery spell this turn".
fn no_spell(compare: &Value) -> Option<String> {
    let tally = compare.pointer("/left/Tally")?;
    (tally.get("event")? == "SpellCast" && tally.get("who")? == "EachPlayer").then_some(())?;
    let (op, right) = (compare.get("op")?, compare.get("right")?);
    let filter = tally.get("filter")?;
    if *op == "Lt" && *right == 1 && *filter == json!([{"ControlledBy": "You"}]) {
        return Some("you haven't cast a spell this turn".to_owned());
    }
    let instants =
        json!([{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}, {"ControlledBy": "You"}]);
    (*op == "Ge" && *right == 1 && *filter == instants)
        .then(|| "you've cast an instant or sorcery spell this turn".to_owned())
}
