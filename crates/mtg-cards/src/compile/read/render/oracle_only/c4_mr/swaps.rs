//! A table of words that the renderers write one way and a card writes another: "its controller" for the controller of
//! the creature that attacks, "equipped creature" for an Equipment, "When this token dies" inside a token's quote, "When"
//! for a trigger that sacrifices its source and "Creatures can't be blocked this turn". Each swap applies to the abilities
//! whose rules hold its needle, alone and together with the other swaps that apply.

use crate::ability::Ability;

/// A swap: the text the rules must hold, the renderers' words and the card's words.
type Swap = (&'static str, &'static str, &'static str);

const SWAPS: [Swap; 10] = [
    (
        r#""Modal":{"choose":{"min":0,"max":3}"#,
        "choose up to three —",
        "choose any number —",
    ),
    (
        r#"{"LoseLife":{"who":"EachPlayer","amount":1}},{"Draw":{"who":"EachPlayer"}}"#,
        "Each player loses 1 life. Then each player draws a card.",
        "Each player loses 1 life and draws a card.",
    ),
    (
        r#""Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}]}"#,
        "Each player sacrifices a creature.",
        "Each player sacrifices a creature of their choice.",
    ),
    (
        r#""Attacks":{"what":[{"Is":"Attached"}]}"#,
        ", that permanent's controller loses ",
        ", its controller loses ",
    ),
    (
        r#""PutCounters":{"on":"Attached""#,
        " on enchanted creature if it's ",
        " on equipped creature if it's ",
    ),
    (
        r#""Sacrifice":{"filter":["This"]}},{"CreateToken""#,
        ", sacrifice ~. Create ",
        ", sacrifice it and create ",
    ),
    (
        r#""Sacrifice":{"filter":["This"]}},{"CreateToken""#,
        "Whenever ~ deals combat damage to a player, sacrifice ",
        "When ~ deals combat damage to a player, sacrifice ",
    ),
    (
        r#""mods":["CantBeBlocked"]"#,
        "All creatures can't be blocked this turn",
        "Creatures can't be blocked this turn",
    ),
    (
        r#""Custom":{"#,
        "\"When ~ dies, ",
        "\"When this token dies, ",
    ),
    (
        r#""Custom":{"#,
        "\"When ~ is put into a graveyard from the battlefield, ",
        "\"When this token dies, ",
    ),
];

/// "if you control no Pest creature tokens, create ..." as "if you don't control a Pest creature token, create ...".
fn single_token(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(", if you control no ")?;
    let (kind, tail) = rest.split_once(" creature tokens, ")?;
    let article = if kind.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "{head}, if you don't control {article} {kind} creature token, {tail}"
    ))
}

/// Adds the wordings with each swap that applies to the rules, one at a time and all together.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let json = serde_json::to_string(ability).unwrap_or_default();
    let applying: Vec<&Swap> = SWAPS.iter().filter(|swap| json.contains(swap.0)).collect();
    let tokens = json.contains(r#""Token""#);
    let mut found = Vec::new();
    for text in texts.iter() {
        let mut all =
            if tokens { single_token(text) } else { None }.unwrap_or_else(|| text.clone());
        for (_, from, to) in &applying {
            if text.contains(from) {
                found.push(text.replacen(from, to, 1));
            }
            all = all.replacen(from, to, 1);
        }
        if all != *text {
            found.push(all);
        }
    }
    texts.extend(found);
}
