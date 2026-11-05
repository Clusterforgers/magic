//! Tokens and mana: `Token` and `Investigate` read the checkout's token scripts, `Mana` the colours it produces.

use serde_json::{Value, json};

use super::amount::amount;
use super::card::translate_face;
use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, flag, only, unmapped};
use super::valid::{defined, selector_any};
use crate::forge::script::{Face, Params};

/// The effects of `Token`, `Investigate` and `Mana`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let effect = match api {
        "Token" => return token(ctx, p),
        "Investigate" => {
            only(p, "Investigate", &["Num", "Defined"])?;
            let mut create = json!({"token": {"Named": "clue"}});
            if let Some(num) = p.get("Num") {
                create["count"] = amount(ctx, num)?;
            }
            if let Some(who) = p.get("Defined") {
                create["who"] = defined(ctx, who)?;
            }
            json!({"CreateToken": create})
        }
        "ManaReflected" => reflected(ctx, p)?,
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
            "TokenPower",
            "TokenToughness",
            "RememberTokens",
            "AttachedTo",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "PumpKeywords",
            "PumpDuration",
        ],
    )?;
    let Some(script) = p.get("TokenScript") else {
        return unmapped("key:Token.no-TokenScript");
    };
    if script.contains(',') {
        // Each script is its own token, made in the order listed with the same count and owner.
        if [
            "TokenPower",
            "TokenToughness",
            "RememberTokens",
            "AttachedTo",
            "ValidTgts",
            "PumpKeywords",
            "PumpDuration",
        ]
        .iter()
        .any(|key| p.has(key))
        {
            return unmapped("key:Token.TokenScript-list");
        }
        let mut effects = Vec::new();
        for one in script.split(',') {
            effects.extend(token_of(ctx, p, one)?);
        }
        return Ok(effects);
    }
    token_of(ctx, p, script)
}

/// The token one script names.
fn token_of(ctx: &mut Ctx, p: &Params, script: &str) -> Tr<Vec<Value>> {
    let Some(face) = ctx.world.tokens.get(script.trim()) else {
        return unmapped("token:unknown-script");
    };
    // A token whose size is counted: it enters as 0/0 and its base power and toughness are set to the numbers fixed before it
    // was made, in the same resolution (state-based actions wait for the end of it).
    let sized = match (p.get("TokenPower"), p.get("TokenToughness")) {
        (None, None) => None,
        (Some(power), Some(toughness))
            if face
                .pt
                .as_deref()
                .is_some_and(|pt| pt.split('/').any(|part| part.parse::<i32>().is_err())) =>
        {
            Some((power, toughness))
        }
        _ => return unmapped("key:Token.TokenPower-shape"),
    };
    let zero;
    let face = if sized.is_some() {
        zero = Face {
            pt: Some("0/0".to_owned()),
            ..face.clone()
        };
        &zero
    } else {
        face
    };
    let mut token = token_ref(ctx, face)?;
    // Its size is part of what the token is (CR 707.2), so a copy of it has the size: the description leaves power and
    // toughness out and the amounts are counted as the token is created.
    if let Some((power, toughness)) = sized {
        let Some(spec) = token.get_mut("Custom").and_then(Value::as_object_mut) else {
            return unmapped("token:sized-shape");
        };
        spec.remove("power");
        spec.remove("toughness");
        let spec = Value::Object(std::mem::take(spec));
        let mut size = json!({"size": amount(ctx, power)?, "token": spec});
        if power != toughness {
            size["toughness"] = amount(ctx, toughness)?;
        }
        token = json!({"Sized": size});
    }
    let count = amount(ctx, p.get("TokenAmount").unwrap_or("1"))?;
    let who = defined(ctx, p.get("TokenOwner").unwrap_or("You"))?;
    let mut create = json!({
        "who": who,
        "token": token,
        "count": count,
        "tapped": flag(p, "TokenTapped")?,
        "attacking": flag(p, "TokenAttacking")?,
    });
    if flag(p, "RememberTokens")? {
        let name = match create.get("as") {
            Some(name) => name.clone(),
            None => json!(ctx.fresh("token")),
        };
        ctx.remembered = Some(json!({ "Bound": name }));
        create["as"] = name;
    }
    let attach = attached(ctx, p, &mut create)?;
    let after = pumped(ctx, p, &mut create)?;
    let mut effects = vec![json!({ "CreateToken": create })];
    effects.extend(after);
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
    // The lines of a token script are read like those of a card: a token with a line left out is not written.
    let inner = translate_face(ctx.world, face);
    if let Some(gap) = inner.gaps.into_iter().next() {
        return unmapped(format!("token:{}", gap.0));
    }
    if let Some(pt) = &face.pt {
        let (power, toughness) = match parse_pt(pt) {
            Ok(stats) => stats,
            // A `*` stat is set by the token's own characteristic-defining ability (CR 604.3), from a base of zero.
            Err(_) if defines_stats(&inner.abilities) => {
                let part = |text: &str| text.parse().unwrap_or(0);
                pt.split_once('/')
                    .map_or((0, 0), |(p, t)| (part(p), part(t)))
            }
            Err(gap) => return Err(gap),
        };
        spec["power"] = json!(power);
        spec["toughness"] = json!(toughness);
    }
    if !inner.abilities.is_empty() {
        spec["abilities"] = Value::Array(inner.abilities);
    }
    Ok(json!({ "Custom": spec }))
}

/// Whether one of the abilities is a characteristic-defining static that sets power and toughness.
fn defines_stats(abilities: &[Value]) -> bool {
    abilities.iter().any(|ability| {
        ability.pointer("/Static/defining") == Some(&json!(true))
            && ability
                .pointer("/Static/mods")
                .and_then(Value::as_array)
                .is_some_and(|mods| mods.iter().any(|m| m.get("SetPT").is_some()))
    })
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
            // A colorless token has no color; "all" is the five colors.
            "colorless" => continue,
            "all" => return Ok(json!(["White", "Blue", "Black", "Red", "Green"])),
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
    only(
        p,
        "Mana",
        &[
            "Produced",
            "Amount",
            "RestrictValid",
            "AddsNoCounter",
            "Defined",
            "ValidTgts",
        ],
    )?;
    let produced = p.get("Produced").unwrap_or_default();
    let times = p.get("Amount").unwrap_or("1");
    let words: Vec<&str> = produced.split_whitespace().collect();
    let mut effect = match words.as_slice() {
        // "Any combination of colors" is not one color: only a single mana is the same thing.
        ["Combo", "Any"] if times != "1" => {
            json!({"mana": {"AnyCombination": {"count": amount(ctx, times)?}}})
        }
        ["Any"] | ["Combo", "Any"] => json!({"mana": {"AnyColor": amount(ctx, times)?}}),
        ["Chosen" | "ChosenColor"] => match times.parse::<u32>() {
            Ok(count) if count > 0 => json!({"mana": {"ChosenColor": count}}),
            _ => return unmapped("key:Mana.Produced=Chosen"),
        },
        ["Combo", "ColorIdentity"] if times == "1" => json!({"mana": {"CommanderIdentity": 1}}),
        ["Combo", color, "Chosen"] if times == "1" && mana_type(color).is_some() => {
            json!({"mana": {"OrChosenColor": mana_type(color)}})
        }
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
    // Whose pool: the player the line names (the default is the controller).
    if let Some(word) = p.get("Defined")
        && !matches!(
            word,
            "You"
                | "Targeted"
                | "ChosenPlayer"
                | "TriggeredPlayer"
                | "TriggeredActivator"
                | "TriggeredCardController"
        )
    {
        return unmapped(format!("key:Mana.Defined={word}"));
    }
    if p.has("Defined") || p.has("ValidTgts") {
        let who = super::valid::subject(ctx, p, "You", None)?;
        if who != json!("You") {
            effect["who"] = who;
        }
    }
    let mut only = Vec::new();
    let restriction = p.get("RestrictValid");
    if let Some(restriction) = restriction {
        only.push(json!({ "Spell": spend_filter(restriction)? }));
    }
    // "That spell can't be countered": a rider on the spells the mana may pay for (`!Permanent` is the instant or sorcery spells).
    match p.get("AddsNoCounter") {
        None => {}
        Some("True") => {
            let filter = restriction.map_or_else(|| Ok(json!({})), spend_filter)?;
            only.push(json!({ "Uncounterable": filter }));
        }
        Some("!Permanent") if restriction.is_none() => {
            only.push(json!({ "Uncounterable": {"types": ["Instant", "Sorcery"]} }));
        }
        Some(other) => return unmapped(format!("key:Mana.AddsNoCounter={other}")),
    }
    if !only.is_empty() {
        effect["only"] = json!(only);
    }
    Ok(json!({ "AddMana": effect }))
}

/// `ManaReflected`: one mana of a type (or colour) that the permanents it names could produce.
fn reflected(ctx: &Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "ManaReflected",
        &["Valid", "ColorOrType", "ReflectProperty", "Amount"],
    )?;
    if p.get("ReflectProperty") != Some("Produce") {
        return unmapped("effect:ManaReflected-property");
    }
    let colorless = match p.get("ColorOrType") {
        Some("Type") => true,
        Some("Color") => false,
        _ => return unmapped("effect:ManaReflected-ColorOrType"),
    };
    let Ok(count) = p.get("Amount").unwrap_or("1").parse::<u32>() else {
        return unmapped("effect:ManaReflected-Amount");
    };
    let among = selector_any(ctx, p.get("Valid").unwrap_or_default())?;
    Ok(
        json!({"AddMana": {"mana": {"ProducedBy": {"among": among, "count": count, "colorless": colorless}}}}),
    )
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

/// The spells a `RestrictValid$` names: card types, a supertype or non-types, and `+ChosenType` for the creature type chosen.
fn spend_filter(restriction: &str) -> Tr<Value> {
    let mut types = Vec::new();
    let mut without = Vec::new();
    let mut supertypes = Vec::new();
    let (restriction, chosen) = restriction
        .strip_suffix("+ChosenType")
        .map_or((restriction, false), |r| (r, true));
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
    if chosen
        && (types.is_empty() || supertypes.len() + without.len() > 0 || restriction.contains(','))
    {
        return unmapped(format!("key:Mana.RestrictValid={restriction}+ChosenType"));
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
    if chosen {
        filter["chosen_type"] = json!(true);
    }
    Ok(filter)
}

/// `AttachedTo$`: an Aura or Equipment token created attached to the target or to the source. A Role stays one per controller
/// on a creature through the state-based actions (CR 704.5y).
fn attached(ctx: &mut Ctx, p: &Params, create: &mut Value) -> Tr<Option<Value>> {
    let Some(word) = p.get("AttachedTo") else {
        if p.has("ValidTgts") {
            return unmapped("key:Token.ValidTgts");
        }
        return Ok(None);
    };
    // One token goes to at most one target; with none chosen the token is not attached and ceases to exist as an Aura would.
    if p.get("TargetMax").is_some_and(|max| max != "1") {
        return unmapped("key:Token.TargetMax");
    }
    let to = match (word, p.has("ValidTgts")) {
        ("Targeted" | "ThisTargetedCard", true) => super::valid::subject(ctx, p, "", None)?,
        ("Self", false) => defined(ctx, "Self")?,
        // The target an earlier step of the ability chose.
        ("Targeted", false) => defined(ctx, "Targeted")?,
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

/// `PumpKeywords$`: "it gains haste until end of turn", a grant on the tokens just made.
fn pumped(ctx: &mut Ctx, p: &Params, create: &mut Value) -> Tr<Option<Value>> {
    let Some(words) = p.get("PumpKeywords") else {
        if p.has("PumpDuration") {
            return unmapped("key:Token.PumpDuration");
        }
        return Ok(None);
    };
    let mods: Vec<Value> = super::keyword::keyword_list(ctx, words)?
        .into_iter()
        .map(|keyword| json!({ "AddKeyword": keyword }))
        .collect();
    let duration = super::effects_obj::duration(p.get("PumpDuration"))?;
    let name = match create.get("as") {
        Some(name) => name.clone(),
        None => json!(ctx.fresh("token")),
    };
    create["as"] = name.clone();
    Ok(Some(
        json!({"Continuous": {"affected": {"Bound": name}, "mods": mods, "duration": duration}}),
    ))
}
