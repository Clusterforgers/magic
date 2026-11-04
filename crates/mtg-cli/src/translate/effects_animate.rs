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
    for (key, trigger) in [("Triggers", true), ("Abilities", false)] {
        let Some(names) = p.get(key) else { continue };
        for name in names.split(',').map(str::trim) {
            let Some(text) = ctx.svar(name).map(str::to_owned) else {
                return unmapped("svar:missing");
            };
            let given = ctx.nested(None, |ctx| {
                if trigger {
                    Ok(vec![super::triggers::triggered(ctx, &text)?])
                } else {
                    super::ability::ability_line(ctx, &text)
                }
            })?;
            mods.extend(given.into_iter().map(|a| json!({"GrantAbility": a})));
        }
    }
    Ok(mods)
}

/// The static a `Mode$` word of an animation stands for; `None` until a word is read.
pub fn animate_words(_ctx: &mut Ctx, _p: &Params) -> Option<Tr<Value>> {
    None
}
