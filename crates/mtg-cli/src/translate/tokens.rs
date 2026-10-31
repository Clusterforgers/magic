//! Tokens and mana: `Token` and `Investigate` read the checkout's token scripts, `Mana` the colours it produces.

use serde_json::{Value, json};

use super::amount::amount;
use super::card::translate_face;
use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, only, unmapped};
use super::valid::{defined, objects, subject};
use crate::forge::script::{Face, Params};

/// The effects of `Token`, `Investigate` and `Mana`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let effect = match api {
        "Token" => token(ctx, p)?,
        "Investigate" => {
            only(p, "Investigate", &[])?;
            json!({"CreateToken": {"token": {"Named": "clue"}}})
        }
        _ => mana(ctx, p)?,
    };
    Ok(vec![effect])
}

/// `CopyPermanent`: a token that is a copy of a permanent, with none of Forge's exceptions.
pub fn copy_permanent(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "CopyPermanent",
        &[
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "NumCopies",
            "TokenTapped",
        ],
    )?;
    let of = objects(subject(ctx, p, "", None)?)?;
    let count = amount(ctx, p.get("NumCopies").unwrap_or("1"))?;
    Ok(vec![json!({"CreateToken": {
        "token": {"CopyOf": {"of": of}},
        "count": count,
        "tapped": p.get("TokenTapped") == Some("True"),
    }})])
}

/// The predefined tokens of the corpus, by the subtype of their script.
const NAMED: [&str; 8] = [
    "Treasure",
    "Food",
    "Clue",
    "Blood",
    "Gold",
    "Powerstone",
    "Junk",
    "Map",
];

fn token(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Token",
        &[
            "TokenScript",
            "TokenAmount",
            "TokenOwner",
            "TokenTapped",
            "TokenAttacking",
        ],
    )?;
    let Some(script) = p.get("TokenScript") else {
        return unmapped("key:Token.no-TokenScript");
    };
    if script.contains(',') {
        return unmapped("key:Token.TokenScript-list");
    }
    let Some(face) = ctx.world.tokens.get(script.trim()) else {
        return unmapped("token:unknown-script");
    };
    let token = token_ref(ctx, face)?;
    let count = amount(ctx, p.get("TokenAmount").unwrap_or("1"))?;
    let who = defined(ctx, p.get("TokenOwner").unwrap_or("You"))?;
    Ok(json!({"CreateToken": {
        "who": who,
        "token": token,
        "count": count,
        "tapped": p.get("TokenTapped") == Some("True"),
        "attacking": p.get("TokenAttacking") == Some("True"),
    }}))
}

/// What a token script describes: a predefined token of the corpus, or a custom one.
fn token_ref(ctx: &Ctx, face: &Face) -> Tr<Value> {
    let words: Vec<&str> = face.types.split_whitespace().collect();
    if !words.contains(&"Creature") {
        for word in &words {
            if NAMED.contains(word) {
                return Ok(json!({ "Named": word.to_lowercase() }));
            }
        }
    }
    custom(ctx, face, &words)
}

fn custom(ctx: &Ctx, face: &Face, words: &[&str]) -> Tr<Value> {
    let types: Vec<&&str> = words.iter().filter(|w| CARD_TYPES.contains(*w)).collect();
    let supertypes: Vec<&&str> = words.iter().filter(|w| SUPERTYPES.contains(*w)).collect();
    let subtypes: Vec<&&str> = words
        .iter()
        .filter(|w| !CARD_TYPES.contains(*w) && !SUPERTYPES.contains(*w))
        .collect();
    let name = face
        .name
        .strip_suffix(" Token")
        .unwrap_or(&face.name)
        .to_owned();
    let mut spec = json!({"name": name, "types": types});
    if !supertypes.is_empty() {
        spec["supertypes"] = json!(supertypes);
    }
    if !subtypes.is_empty() {
        spec["subtypes"] = json!(subtypes);
    }
    if let Some(colors) = &face.colors {
        spec["colors"] = colors_of(colors)?;
    }
    if words.contains(&"Creature") != face.pt.is_some() {
        return unmapped("token:stats-shape");
    }
    if let Some(pt) = &face.pt {
        let (power, toughness) = parse_pt(pt)?;
        spec["power"] = json!(power);
        spec["toughness"] = json!(toughness);
    }
    // The lines of a token script are read like those of a card: a token with a line left out is not written.
    let inner = translate_face(ctx.world, face);
    if let Some(gap) = inner.gaps.into_iter().next() {
        return unmapped(format!("token:{}", gap.0));
    }
    if !inner.abilities.is_empty() {
        spec["abilities"] = Value::Array(inner.abilities);
    }
    Ok(json!({ "Custom": spec }))
}

fn parse_pt(pt: &str) -> Tr<(i32, i32)> {
    let parsed = pt
        .split_once('/')
        .and_then(|(p, t)| Some((p.parse().ok()?, t.parse().ok()?)));
    parsed.map_or_else(|| unmapped("token:variable-stats"), Ok)
}

fn colors_of(colors: &str) -> Tr<Value> {
    let mut out = Vec::new();
    for word in colors.split(',') {
        out.push(match word.trim() {
            "white" => "White",
            "blue" => "Blue",
            "black" => "Black",
            "red" => "Red",
            "green" => "Green",
            _ => return unmapped("token:colors"),
        });
    }
    Ok(json!(out))
}

/// The mana type a Forge mana word stands for.
fn mana_type(word: &str) -> Option<&'static str> {
    Some(match word {
        "W" => "White",
        "U" => "Blue",
        "B" => "Black",
        "R" => "Red",
        "G" => "Green",
        "C" => "Colorless",
        _ => return None,
    })
}

fn mana(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Mana", &["Produced", "Amount"])?;
    let produced = p.get("Produced").unwrap_or_default();
    let times = p.get("Amount").unwrap_or("1");
    let words: Vec<&str> = produced.split_whitespace().collect();
    let mana = match words.as_slice() {
        ["Any"] | ["Combo", "Any"] => json!({"AnyColor": amount(ctx, times)?}),
        ["Combo", rest @ ..] if times == "1" && rest.len() > 1 => {
            let types: Option<Vec<&str>> = rest.iter().map(|w| mana_type(w)).collect();
            match types {
                Some(types) => json!({ "OneOf": types }),
                None => return unmapped("key:Mana.Produced=Combo"),
            }
        }
        fixed if !fixed.is_empty() && fixed.iter().all(|w| mana_type(w).is_some()) => {
            return fixed_mana(ctx, fixed, times);
        }
        _ => return unmapped("key:Mana.Produced"),
    };
    Ok(json!({"AddMana": {"mana": mana}}))
}

/// Fixed mana: the listed types, repeated `times` times (a number repeats the list, anything else is a count).
fn fixed_mana(ctx: &Ctx, words: &[&str], times: &str) -> Tr<Value> {
    let types: Vec<&str> = words.iter().filter_map(|w| mana_type(w)).collect();
    if let Ok(repeat) = times.parse::<usize>() {
        let all: Vec<&str> = (0..repeat).flat_map(|_| types.iter().copied()).collect();
        return Ok(json!({"AddMana": {"mana": {"Fixed": all}}}));
    }
    Ok(json!({"AddMana": {"mana": {"Fixed": types}, "times": amount(ctx, times)?}}))
}
