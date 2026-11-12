//! Wordings of pack l31-b: a payment of any amount the payer chooses as it resolves ("you may pay any amount of {E}",
//! "pay any amount of life", "each player may pay any amount of mana"), written from a twin whose cost is "X life", with
//! the X the payment announced said as the card says it ("that much damage", "draw that many cards", "for each {E} paid
//! this way") and a target chosen in a sentence of its own ("Choose target creature. ... that creature").

use serde_json::{Value, json};

use super::selectors::spec_text;
use crate::ability::Ability;

/// The cost part of the twin, worded "pay X life" by the payment writer.
const MARK: &str = "pay X life";

/// The kinds of a payment of any amount, by the cost the payment holds.
fn kind(cost: &Value) -> Option<&'static str> {
    if *cost == json!("{X}") || *cost == json!([{"Mana": "{X}"}]) {
        Some("mana")
    } else if *cost == json!([{"PayLife": "X"}]) {
        Some("life")
    } else if *cost == json!([{"PayEnergy": "X"}]) {
        Some("{E}")
    } else {
        None
    }
}

/// The steps of the ability, wherever its body keeps them.
fn steps_of(value: &mut Value) -> Option<&mut Value> {
    let (_, body) = value.as_object_mut()?.iter_mut().next()?;
    body.get_mut("effects")
}

/// Turns the one payment of any amount in the steps (or in a repeat over players among them) into a payment of X life
/// and gives its kind; `None` when there is not exactly one.
fn twin_steps(steps: &mut Value) -> Option<&'static str> {
    let mut found = None;
    for step in steps.as_array_mut()? {
        let pay = if let Some(pay) = step.get_mut("Pay") {
            pay
        } else if let Some(each) = step.get_mut("RepeatEach")
            && let Some([inner]) = each
                .get_mut("effects")
                .and_then(Value::as_array_mut)
                .map(Vec::as_mut_slice)
            && let Some(pay) = inner.get_mut("Pay")
        {
            pay
        } else {
            continue;
        };
        let Some(kind) = pay.get("cost").and_then(kind) else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        pay["cost"] = json!([{"PayLife": "X"}]);
        found = Some(kind);
    }
    found
}

/// Says each X that stands as a word as the amount paid: "that much" before damage or life ("deals that much damage"),
/// "that many" before anything counted ("draw that many cards"); never an X inside a symbol (`{X}`) or a stat change.
fn replace_x(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, c) in chars.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
        let after = chars.get(i + 1).copied();
        let alone = |c: Option<char>| {
            c.is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '{' | '}' | '/' | '-' | '+')))
        };
        if *c == 'X' && alone(before) && alone(after) {
            let next: String = chars
                .iter()
                .skip(i + 2)
                .take_while(|c| c.is_alphabetic())
                .collect();
            out.push_str(if matches!(next.as_str(), "damage" | "life") {
                "that much"
            } else {
                "that many"
            });
        } else {
            out.push(*c);
        }
    }
    out
}

/// Upper-cases the first letter.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The wordings of what follows the payment, X said the card's way: the amount ("that much", "that many"), a stat
/// change of one for each paid, a mana value at most the amount paid, and the sentence without "If you do," (a payment
/// of 0 does what no payment does).
fn tails(tail: &str, kind: &str) -> Vec<String> {
    let paid = format!("the amount of {kind} paid this way");
    let mut shape = tail.replace(
        "-X/-X until end of turn",
        &format!("-1/-1 until end of turn for each {kind} paid this way"),
    );
    shape = shape.replace(
        "mana value X or less",
        &format!("mana value less than or equal to {paid}"),
    );
    shape = each_of_all(&shape);
    let said = replace_x(&shape);
    let mut out = vec![said.clone()];
    let bare = without_if_you_do(&said);
    if bare != said {
        out.push(bare);
    }
    out
}

/// ". If you do, ~ deals ..." as ". ~ deals ...".
fn without_if_you_do(tail: &str) -> String {
    match tail.split_once(". If you do, ") {
        Some((before, after)) => format!("{before}. {}", capital(after)),
        None => tail.to_owned(),
    }
}

/// "Choose target creature. ..." with the target named later as "that creature", "the creature" or "that permanent".
fn chosen_first(text: &str, target: &str) -> Vec<String> {
    let text = &text.replace(&capital(target), target);
    if text.matches(target).count() != 1 {
        return Vec::new();
    }
    let noun = target.trim_start_matches("target ");
    let mut callings = vec![format!("that {noun}"), format!("the {noun}")];
    if noun.contains(" or ") {
        callings.push("that permanent".to_owned());
    }
    let mut out = Vec::new();
    for calling in callings {
        let renamed = text.replace(target, &calling);
        let renamed = renamed.replace(&format!(". {calling}"), &format!(". {}", capital(&calling)));
        out.push(format!("Choose {target}. {}", capital(&renamed)));
    }
    out
}

/// Adds the wordings of an ability whose one payment is of any amount of mana, life or energy.
pub(super) fn l31b_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let target = value
        .as_object()
        .and_then(|body| body.values().next())
        .and_then(|body| body.get("targets"))
        .and_then(|targets| {
            serde_json::from_value::<Vec<crate::refs::TargetSpec>>(targets.clone()).ok()
        })
        .and_then(|targets| match targets.as_slice() {
            [one] => spec_text(one),
            _ => None,
        });
    let Some(kind) = steps_of(&mut value).and_then(twin_steps) else {
        return;
    };
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let base = if kind == "life" {
        texts.clone()
    } else {
        written(&twin)
    };
    let mut found = Vec::new();
    for text in base {
        let lower = text.replacen("Pay X life", MARK, 1);
        if lower.matches(MARK).count() != 1 {
            continue;
        }
        let Some(at) = lower.find(MARK) else { continue };
        let head = &text[..at];
        let rest = &text[at + MARK.len()..];
        let verb = if text[at..].starts_with('P') {
            "Pay"
        } else {
            "pay"
        };
        for tail in tails(rest, kind) {
            let said = format!("{head}{verb} any amount of {kind}{tail}");
            found.extend(target.iter().flat_map(|target| chosen_first(&said, target)));
            found.push(said);
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "destroy all artifacts, creatures, and enchantments with ..." as "destroy each artifact, creature, and enchantment
/// with ...": a list of card types said one by one.
fn each_of_all(text: &str) -> String {
    let Some((before, rest)) = text.split_once("destroy all ") else {
        return text.to_owned();
    };
    let Some((list, tail)) = rest.split_once(" with ") else {
        return text.to_owned();
    };
    let words: Option<Vec<&str>> = list
        .split(", ")
        .map(|item| item.trim_start_matches("and ").strip_suffix('s'))
        .collect();
    let Some(words) = words else {
        return text.to_owned();
    };
    let singular = match words.as_slice() {
        [one] => (*one).to_owned(),
        [first, second] => format!("{first} and {second}"),
        [leading @ .., final_one] => format!("{}, and {final_one}", leading.join(", ")),
        [] => return text.to_owned(),
    };
    if list.contains(" or ") || list.split(' ').count() > words.len() + 1 {
        return text.to_owned();
    }
    format!("{before}destroy each {singular} with {tail}")
}

/// The words of the ability with these steps in place of its own, each ending in "draw a card." cut off: the head of a
/// trigger ("When this creature dies, "), empty for a spell.
fn heads(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let mut twin = value.clone();
    let Some(steps) = steps_of(&mut twin) else {
        return Vec::new();
    };
    *steps = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(twin) else {
        return Vec::new();
    };
    let mut out: Vec<String> = written(&twin)
        .iter()
        .filter_map(|text| text.strip_suffix("draw a card."))
        .filter(|head| head.is_empty() || head.ends_with(", "))
        .map(str::to_owned)
        .collect();
    out.dedup();
    out
}

/// "1/1 green Squirrel creature tokens": the token as two of it are named.
fn tokens_text(token: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let twin = json!({"Spell": {"effects": [{"CreateToken": {"token": token, "count": 2}}]}});
    let twin = serde_json::from_value::<Ability>(twin).ok()?;
    written(&twin).iter().find_map(|text| {
        text.strip_prefix("Create two ")
            .and_then(|rest| rest.strip_suffix('.'))
            .map(str::to_owned)
    })
}

/// "each player may pay any amount of mana. Then each player creates a number of 1/1 green Squirrel creature tokens
/// equal to the amount of mana they paid this way.": each player pays in turn order, the amount each paid is kept for
/// that player, and the tokens are created together.
pub(super) fn l31b_each_paid(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(steps) = steps_of(&mut value).cloned() else {
        return;
    };
    let Some([pays, creates]) = steps.as_array().map(Vec::as_slice) else {
        return;
    };
    let Some(each) = pays.get("RepeatEach") else {
        return;
    };
    let payer = json!({"Bound": each["as"]});
    let Some(kind) = each["effects"][0]["Pay"].get("cost").and_then(kind) else {
        return;
    };
    let slot = &each["effects"][0]["Pay"]["paid"][0]["RememberValue"]["slot"];
    let pay = json!({"RepeatEach": {"over": "EachPlayer", "as": each["as"], "effects": [{"Pay": {
        "who": payer,
        "cost": each["effects"][0]["Pay"]["cost"],
        "paid": [{"RememberValue": {"value": "X", "slot": slot, "per": payer}}],
    }}]}});
    let inner = &creates["Together"]["effects"][0]["RepeatEach"];
    let creator = json!({"Bound": inner["as"]});
    let token = &inner["effects"][0]["CreateToken"]["token"];
    let create = json!({"Together": {"effects": [{"RepeatEach": {"over": "EachPlayer", "as": inner["as"], "effects": [
        {"CreateToken": {"who": creator, "token": token, "count": {"Remembered": {"on": "This", "slot": slot, "per": creator}}}}
    ]}}]}});
    if *pays != pay || *creates != create || !slot.is_string() {
        return;
    }
    let Some(tokens) = tokens_text(token, written) else {
        return;
    };
    for head in heads(&value, written) {
        let start = if head.is_empty() { "Each" } else { "each" };
        let text = format!(
            "{head}{start} player may pay any amount of {kind}. Then each player creates a number of {tokens} equal to the amount of {kind} they paid this way."
        );
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
