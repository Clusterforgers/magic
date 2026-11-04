//! Tokens and mana: `Token` and `Investigate` read the checkout's token scripts, `Mana` the colours it produces.

use serde_json::{Value, json};

use super::amount::amount;
use super::card::translate_face;
use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, only, unmapped};
use super::valid::defined;
use crate::forge::script::{Face, Params};

/// The effects of `Token`, `Investigate` and `Mana`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let effect = match api {
        "Token" => return token(ctx, p),
        "Investigate" => {
            only(p, "Investigate", &[])?;
            json!({"CreateToken": {"token": {"Named": "clue"}}})
        }
        _ => mana(ctx, p)?,
    };
    Ok(vec![effect])
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

/// `Token`: the created tokens, and with `AtEOT$` the delayed clean-up of exactly those tokens.
fn token(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let Some(word) = p.get("AtEOT") else {
        return token_create(ctx, p);
    };
    let mut created = token_create(ctx, &p.without(&["AtEOT"]))?;
    let create = &mut created[0];
    let bound = if let Some(name) = create["CreateToken"].get("as") {
        json!({ "Bound": name })
    } else {
        let name = ctx.fresh("token");
        create["CreateToken"]["as"] = json!(name);
        json!({ "Bound": name })
    };
    let clean_up = super::tokens_copy::at_eot(word, &bound)?;
    created.push(clean_up);
    Ok(created)
}

fn token_create(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Token",
        &[
            "TokenScript",
            "TokenAmount",
            "TokenOwner",
            "TokenTapped",
            "TokenAttacking",
            "RememberTokens",
            "AttachedTo",
            "ValidTgts",
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
    let mut create = json!({
        "who": who,
        "token": token,
        "count": count,
        "tapped": p.get("TokenTapped") == Some("True"),
        "attacking": p.get("TokenAttacking") == Some("True"),
    });
    if p.get("RememberTokens") == Some("True") {
        let name = ctx.fresh("token");
        ctx.remembered = Some(json!({ "Bound": name }));
        create["as"] = json!(name);
    }
    let attach = attached(ctx, p, face, &mut create)?;
    let mut effects = vec![json!({ "CreateToken": create })];
    effects.extend(attach);
    Ok(effects)
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
    only(p, "Mana", &["Produced", "Amount", "RestrictValid"])?;
    let produced = p.get("Produced").unwrap_or_default();
    let times = p.get("Amount").unwrap_or("1");
    let words: Vec<&str> = produced.split_whitespace().collect();
    let mut effect = match words.as_slice() {
        ["Any"] | ["Combo", "Any"] => json!({"mana": {"AnyColor": amount(ctx, times)?}}),
        ["Combo", rest @ ..] if times == "1" && rest.len() > 1 => {
            let types: Option<Vec<&str>> = rest.iter().map(|w| mana_type(w)).collect();
            match types {
                Some(types) => json!({"mana": {"OneOf": types}}),
                None => return unmapped("key:Mana.Produced=Combo"),
            }
        }
        fixed if !fixed.is_empty() && fixed.iter().all(|w| mana_type(w).is_some()) => {
            fixed_mana(ctx, fixed, times)?
        }
        _ => return unmapped("key:Mana.Produced"),
    };
    if let Some(restriction) = p.get("RestrictValid") {
        effect["only"] = json!([spend_on(restriction)?]);
    }
    Ok(json!({ "AddMana": effect }))
}

/// Fixed mana: the listed types, repeated `times` times (a number repeats the list, anything else is a count).
fn fixed_mana(ctx: &Ctx, words: &[&str], times: &str) -> Tr<Value> {
    let types: Vec<&str> = words.iter().filter_map(|w| mana_type(w)).collect();
    if let Ok(repeat) = times.parse::<usize>() {
        let all: Vec<&str> = (0..repeat).flat_map(|_| types.iter().copied()).collect();
        return Ok(json!({"mana": {"Fixed": all}}));
    }
    Ok(json!({"mana": {"Fixed": types}, "times": amount(ctx, times)?}))
}

/// What mana may be spent on: spells of the card types or supertype a `RestrictValid$` of spells names.
fn spend_on(restriction: &str) -> Tr<Value> {
    let mut types = Vec::new();
    let mut without = Vec::new();
    let mut supertypes = Vec::new();
    for alternative in restriction.split(',') {
        let Some(word) = alternative.strip_prefix("Spell.") else {
            return unmapped(format!("key:Mana.RestrictValid={restriction}"));
        };
        match word.strip_prefix("non") {
            Some(rest) if CARD_TYPES.contains(&rest) => without.push(rest),
            _ if CARD_TYPES.contains(&word) => types.push(word),
            _ if SUPERTYPES.contains(&word) => supertypes.push(word),
            _ => return unmapped(format!("key:Mana.RestrictValid={restriction}")),
        }
    }
    let kinds = [
        !types.is_empty(),
        !without.is_empty(),
        !supertypes.is_empty(),
    ];
    if kinds.iter().filter(|kind| **kind).count() != 1 {
        return unmapped(format!("key:Mana.RestrictValid={restriction}"));
    }
    let mut filter = json!({});
    if !types.is_empty() {
        filter["types"] = json!(types);
    }
    if !without.is_empty() {
        filter["without"] = json!(without);
    }
    if !supertypes.is_empty() {
        filter["supertypes"] = json!(supertypes);
    }
    Ok(json!({ "Spell": filter }))
}

/// `AttachedTo$`: an Aura or Equipment token created attached to the target or to the source. A Role needs the rule that
/// keeps one per controller on an object, which the engine lacks, so it stays a gap.
fn attached(ctx: &mut Ctx, p: &Params, face: &Face, create: &mut Value) -> Tr<Option<Value>> {
    let Some(word) = p.get("AttachedTo") else {
        if p.has("ValidTgts") {
            return unmapped("key:Token.ValidTgts");
        }
        return Ok(None);
    };
    if face.types.split_whitespace().any(|w| w == "Role") {
        return unmapped("token:Role-rule");
    }
    let to = match (word, p.has("ValidTgts")) {
        ("Targeted", true) => super::valid::subject(ctx, p, "", None)?,
        ("Self", false) => defined(ctx, "Self")?,
        _ => return unmapped(format!("key:Token.AttachedTo={word}")),
    };
    let name = if let Some(name) = create.get("as").and_then(Value::as_str) {
        name.to_owned()
    } else {
        let name = ctx.fresh("token");
        create["as"] = json!(name);
        name
    };
    Ok(Some(json!({"Attach": {"what": {"Bound": name}, "to": to}})))
}
