//! A permission to cast cards that opens the effects of an ability: cards exiled from the top of a library ("exile the
//! top seven cards of your library. Until end of turn, you may cast spells from among them", "exile the top card of that
//! player's library. You may cast that card for as long as it remains exiled") or a target card in your graveyard ("you
//! may cast target Zombie creature card from your graveyard this turn", "choose target artifact card in your graveyard.
//! You may cast that card this turn"). Later effects follow as sentences of their own.

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, clauses, heads, joined, parts};
use super::w5b_parts::{w5b_graveyard_target, w5b_number};
use crate::ability::Ability;

/// Adds the wordings of an ability whose effects open with a permission to cast.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let targets = value
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|inner| inner.get("targets"))
        .cloned()
        .unwrap_or(json!([]));
    let Some((opening, taken)) = opening(&effects, &targets) else {
        return;
    };
    let rest = &effects[taken..];
    let after: Vec<String> = if rest.is_empty() {
        vec![String::new()]
    } else {
        clauses(rest)
            .iter()
            .map(|clause| format!(" {}.", capitalized(clause)))
            .collect()
    };
    for head in heads(&value) {
        for sentence in &opening {
            for tail in &after {
                texts.push(format!("{}.{tail}", joined(&head, sentence)));
            }
        }
    }
}

/// The sentences of the permission and the number of effects they word.
fn opening(effects: &[Value], targets: &Value) -> Option<(Vec<String>, usize)> {
    match effects {
        [exile, rule, ..] if exile.get("ExileTop").is_some() && *targets == json!([]) => {
            Some((exiled(exile.get("ExileTop")?, rule)?, 2))
        }
        [rule, ..] => Some((from_graveyard(rule, targets)?, 1)),
        [] => None,
    }
}

/// The permission of a rule step: the zone it casts from, what it casts and how long it lasts.
fn permission<'a>(rule: &'a Value, from: &str) -> Option<(&'a Value, &'a str)> {
    let step = rule.get("Rule")?;
    let cast = step.get("rule")?.get("MayCast")?.as_object()?;
    let duration = step.get("duration")?.as_str()?;
    if cast.get("from")? != from {
        return None;
    }
    let cast_what = match (cast.get("only"), cast.get("what")) {
        (Some(only), None) => only,
        (None, Some(Value::Array(what))) => match what.as_slice() {
            [single] => single.get("Is")?,
            _ => return None,
        },
        _ => return None,
    };
    let known = cast
        .keys()
        .all(|key| ["from", "only", "what"].contains(&key.as_str()));
    known.then_some((cast_what, duration))
}

/// "exile the top seven cards of your library. Until end of turn, you may cast spells from among them".
fn exiled(exile: &Value, rule: &Value) -> Option<Vec<String>> {
    let (what, duration) = permission(rule, "Exile")?;
    if *what != json!({"Bound": exile.get("as")?}) {
        return None;
    }
    let library = match exile
        .get("who")
        .map_or("You", |who| who.as_str().unwrap_or(""))
    {
        "You" => "your library",
        "Triggering" => "that player's library",
        _ => return None,
    };
    let count = exile.get("count").cloned().unwrap_or(json!(1));
    let (top, one) = if count == json!(1) {
        (format!("exile the top card of {library}"), true)
    } else if count == json!("Triggering") {
        (
            format!("exile that many cards from the top of {library}"),
            false,
        )
    } else {
        let number = w5b_number(&count)?;
        (format!("exile the top {number} cards of {library}"), false)
    };
    let casts: &[&str] = match (one, duration) {
        (true, "UntilEndOfTurn") => &[
            "Until end of turn, you may cast that card",
            "You may cast that card this turn",
        ],
        (true, "Forever") => &["You may cast that card for as long as it remains exiled"],
        (false, "UntilEndOfTurn") => &[
            "Until end of turn, you may cast spells from among them",
            "Until end of turn, you may cast spells from among those cards",
            "Until end of turn, you may cast spells from among those exiled cards",
        ],
        (false, "Forever") => {
            &["For as long as they remain exiled, you may cast spells from among them"]
        }
        _ => return None,
    };
    Some(casts.iter().map(|cast| format!("{top}. {cast}")).collect())
}

/// "you may cast target Zombie creature card from your graveyard this turn".
fn from_graveyard(rule: &Value, targets: &Value) -> Option<Vec<String>> {
    let (what, duration) = permission(rule, "Graveyard")?;
    if *what != json!({"Target": 0}) || duration != "UntilEndOfTurn" {
        return None;
    }
    if targets.as_array().map(Vec::len) != Some(1) {
        return None;
    }
    let mut found = Vec::new();
    for target in w5b_graveyard_target(targets) {
        found.push(format!(
            "you may cast {target} from your graveyard this turn"
        ));
        found.push(format!(
            "choose {target} in your graveyard. You may cast that card this turn"
        ));
    }
    Some(found)
}
