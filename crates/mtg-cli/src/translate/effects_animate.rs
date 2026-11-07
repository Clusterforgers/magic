//! `Animate` abilities: the keywords and abilities an animate line takes away, and the triggered and activated abilities
//! its `Triggers$` and `Abilities$` give to the object.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::keyword::keyword_list;
use crate::forge::script::Params;

/// The modifications of the abilities an `Animate` line changes: losses first, then the grants (layer 6 order).
pub fn abilities(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if p.get("RemoveAllAbilities") == Some("True") {
        mods.push(json!("LoseAbilities"));
    }
    if let Some(keywords) = p.get("RemoveKeywords") {
        for keyword in keyword_list(ctx, keywords)? {
            mods.push(json!({ "RemoveKeyword": keyword }));
        }
    }
    for (key, kind) in [
        ("Triggers", Given::Trigger),
        ("Abilities", Given::Ability),
        ("staticAbilities", Given::Static),
    ] {
        let Some(names) = p.get(key) else { continue };
        for name in names.split(',').map(str::trim) {
            let Some(text) = ctx.svar(name).map(str::to_owned) else {
                return unmapped("svar:missing");
            };
            let given = ctx.nested(None, |ctx| match kind {
                Given::Trigger => Ok(vec![super::triggers::triggered(ctx, &text)?]),
                Given::Ability => super::ability::ability_line(ctx, &text),
                Given::Static => given_static(ctx, &text).map(|given| vec![given]),
            })?;
            mods.extend(given.into_iter().map(|a| json!({"GrantAbility": a})));
        }
    }
    Ok(mods)
}

/// `HiddenKeywords$` of an `Animate` line: the combat restrictions in words ("CARDNAME can't attack or block."). Any other
/// hidden keyword (a requirement such as "must be blocked if able") stays a gap.
pub fn restrictions(p: &Params) -> Tr<Vec<Value>> {
    let Some(words) = p.get("HiddenKeywords") else {
        return Ok(Vec::new());
    };
    let mut mods = Vec::new();
    for word in words
        .split('&')
        .map(|word| word.trim().trim_end_matches('.'))
    {
        match word {
            "CARDNAME can't block" => mods.push(json!("CantBlock")),
            "CARDNAME can't attack" => mods.push(json!("CantAttack")),
            "CARDNAME can't attack or block" => {
                mods.extend([json!("CantAttack"), json!("CantBlock")]);
            }
            _ => return unmapped("key:Animate.HiddenKeywords"),
        }
    }
    Ok(mods)
}

/// The static a `Mode$` word of an animation stands for; `None` until a word is read.
pub fn animate_words(_ctx: &mut Ctx, _p: &Params) -> Option<Tr<Value>> {
    None
}

/// Which kind of ability an `Animate` key gives.
#[derive(Clone, Copy)]
enum Given {
    Trigger,
    Ability,
    Static,
}

/// A static ability the object gains: only a `Static` that does not itself give abilities, the form the engine grants.
fn given_static(ctx: &mut Ctx, text: &str) -> Tr<Value> {
    let given = super::statics::static_line(ctx, text)?;
    let Some(inner) = given.get("Static") else {
        return unmapped("key:Animate.staticAbilities-kind");
    };
    if inner.get("defining").is_some() {
        return unmapped("key:Animate.staticAbilities-defining");
    }
    let grants = inner
        .get("mods")
        .and_then(Value::as_array)
        .is_some_and(|mods| mods.iter().any(|m| m.get("GrantAbility").is_some()));
    if grants {
        return unmapped("key:Animate.staticAbilities-grants");
    }
    Ok(given)
}
