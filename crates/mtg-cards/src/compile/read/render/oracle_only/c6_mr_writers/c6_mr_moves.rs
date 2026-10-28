//! Moves of the sixth sample of M to R: "exile ... and copy it. You may cast the copy without paying its mana cost.",
//! "If you do, [one step] and [the next steps]", "Target player returns a ... card from their graveyard to their hand",
//! "from your hand or graveyard onto the battlefield" and "you may play that card without paying its mana cost".

use serde_json::Value;

use super::{c6_mr_name_after, c6_mr_swapped, c6_mr_twin_all, c6_mr_written};

/// The most wordings of each part that a sentence joined of two parts takes.
const C6_MR_PARTS: usize = 8;

/// A choice from your hand or graveyard, and the choice from your hand the renderers write.
const C6_MR_HAND_OR_GRAVEYARD: (&str, &str) = (
    r#"{"Each":[{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]},{"OwnedBy":"You"},"#,
    r#"{"Each":[{"In":"Hand"},{"OwnedBy":"You"},"#,
);

/// A free permission to play an exiled card, and the permission with its cost.
const C6_MR_FREE_PLAY: (&str, &str) = (
    r#"{"MayCast":{"from":"Exile","free":true,"only":"#,
    r#"{"MayCast":{"from":"Exile","only":"#,
);

/// The free play the renderers write, and the oracle's.
const C6_MR_PLAYS: [(&str, &str); 2] = [
    (
        "Until end of turn, you may play that card.",
        "Until end of turn, you may play that card without paying its mana cost.",
    ),
    (
        "You may play that card this turn.",
        "You may play that card this turn without paying its mana cost.",
    ),
];

/// The head of a choice of a card from a target player's graveyard.
const C6_MR_THEIR_CARD: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Choose":{"who":{"Target":0},"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}},"#;

/// Adds the wordings of the moves.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    c6_mr_copy_cast(json, texts);
    c6_mr_if_you_do(json, texts);
    c6_mr_their_card(json, texts);
    texts.extend(c6_mr_swapped(
        &c6_mr_twin_all(json, &[C6_MR_HAND_OR_GRAVEYARD]),
        "from your hand onto the battlefield",
        "from your hand or graveyard onto the battlefield",
    ));
    let paid = c6_mr_twin_all(json, &[C6_MR_FREE_PLAY]);
    for (from, to) in C6_MR_PLAYS {
        texts.extend(c6_mr_swapped(&paid, from, to));
    }
}

/// "exile ... and copy it. You may cast the copy without paying its mana cost.", from the twin that only exiles.
fn c6_mr_copy_cast(json: &str, texts: &mut Vec<String>) {
    let Some(exiled) = c6_mr_name_after(json, r#"{"CopyCard":{"what":{"Bound":""#) else {
        return;
    };
    let Some(copy) = c6_mr_name_after(json, &format!(r#"{{"Bound":"{exiled}"}},"as":""#)) else {
        return;
    };
    let tail = format!(
        r#",{{"CopyCard":{{"what":{{"Bound":"{exiled}"}},"as":"{copy}"}}}},{{"CastFromZone":{{"what":{{"Bound":"{copy}"}},"free":true}}}}]"#
    );
    let exile = format!(r#"{{"Exile":{{"what":{{"Target":0}},"as":"{exiled}"}}}}{tail}"#);
    let found: Vec<String> =
        c6_mr_twin_all(json, &[(&exile, r#"{"Exile":{"what":{"Target":0}}}]"#)])
            .iter()
            .filter_map(|text| text.strip_suffix('.'))
            .map(|head| {
                format!("{head} and copy it. You may cast the copy without paying its mana cost.")
            })
            .collect();
    texts.extend(found);
}

/// "If you do, [one step] and [the next steps]" for a last step done if the step before it was: the twin that does only
/// the first step joined to the words of the others as a spell of their own.
fn c6_mr_if_you_do(json: &str, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::from_str::<Value>(json) else {
        return;
    };
    let Some(effects) = value
        .as_object_mut()
        .and_then(|outer| outer.values_mut().next())
        .and_then(|inner| inner.get_mut("effects"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(then) = effects
        .last_mut()
        .and_then(|last| last.get_mut("If"))
        .filter(|step| step.get("condition") == Some(&Value::from("Performed")))
        .filter(|step| step.get("otherwise").is_none())
        .and_then(|step| step.get_mut("then"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if then.len() < 2 {
        return;
    }
    let rest: Vec<Value> = then.drain(1..).collect();
    let first = c6_mr_written(&value.to_string());
    let others = serde_json::json!({ "Spell": { "effects": rest } });
    let others = c6_mr_written(&others.to_string());
    for head in first.iter().take(C6_MR_PARTS) {
        let Some(head) = head
            .strip_suffix('.')
            .filter(|head| head.contains("If you do, "))
        else {
            continue;
        };
        for other in others.iter().take(C6_MR_PARTS) {
            let mut chars = other.chars();
            let Some(letter) = chars.next() else {
                continue;
            };
            texts.push(format!(
                "{head} and {}{}",
                letter.to_lowercase(),
                chars.as_str()
            ));
        }
    }
}

/// "Target player returns a ... card from their graveyard to their hand", with the card named by the twin that puts
/// such a card from your hand onto the battlefield.
fn c6_mr_their_card(json: &str, texts: &mut Vec<String>) {
    let Some(rest) = json.strip_prefix(C6_MR_THEIR_CARD) else {
        return;
    };
    let Some((preds, tail)) = rest.split_once("]},\"as\":\"") else {
        return;
    };
    let Some(name) = tail.get(..tail.find('"').unwrap_or(tail.len())) else {
        return;
    };
    let ending =
        format!(r#"{name}"}}}},{{"Move":{{"what":{{"Bound":"{name}"}},"to":"Hand"}}}}]}}}}"#);
    if tail != ending {
        return;
    }
    let twin = format!(
        r#"{{"Spell":{{"effects":[{{"May":{{"effects":[{{"Choose":{{"from":{{"Each":[{{"In":"Hand"}},{{"OwnedBy":"You"}},{preds}]}},"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":{{"Battlefield":{{}}}}}}}}]}}}}]}}}}"#
    );
    let found: Vec<String> = c6_mr_written(&twin)
        .iter()
        .filter_map(|text| text.strip_prefix("You may put "))
        .filter_map(|rest| rest.strip_suffix(" from your hand onto the battlefield."))
        .map(|card| format!("Target player returns {card} from their graveyard to their hand."))
        .collect();
    texts.extend(found);
}
