//! An emblem: Forge's permanent `Effect` object named "Emblem ..." that carries static, triggered and replacement
//! abilities, as the DSL's `CreateEmblem`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::replacements::replacement_line;
use super::statics::static_line;
use super::triggers::triggered;
use crate::forge::script::Params;

/// Whether an `Effect` line makes an emblem: it is named so and lasts for the rest of the game.
pub fn is_emblem(p: &Params) -> bool {
    p.get("Name").is_some_and(|name| name.starts_with("Emblem"))
        && p.get("Duration") == Some("Permanent")
}

/// The line without the keys that only say the ability works from the command zone, which an emblem always does.
fn rebuilt(body: &str) -> String {
    let parsed = Params::parse(body);
    let kept: Vec<String> = parsed
        .iter()
        .filter(|(key, value)| {
            !(matches!(*key, "TriggerZones" | "EffectZone" | "ActiveZones") && *value == "Command")
        })
        .map(|(key, value)| format!("{key}$ {value}"))
        .collect();
    kept.join(" | ")
}

/// The abilities one list of the line names (`StaticAbilities$ A,B`), each read by `read`.
fn abilities(
    ctx: &mut Ctx,
    p: &Params,
    key: &str,
    read: fn(&mut Ctx, &str) -> Tr<Value>,
) -> Tr<Vec<Value>> {
    let mut out = Vec::new();
    let Some(names) = p.get(key) else {
        return Ok(out);
    };
    for name in names.split(',') {
        let Some(text) = ctx.svar(name) else {
            return unmapped("svar:missing");
        };
        out.push(read(ctx, &rebuilt(text))?);
    }
    Ok(out)
}

/// `Effect` as `CreateEmblem`.
pub fn emblem(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    for key in ["RememberObjects", "ValidTgts", "Defined", "EffectOwner"] {
        if p.has(key) {
            return unmapped(format!("key:Effect.emblem.{key}"));
        }
    }
    if !ctx.targets.is_empty() {
        return unmapped("key:Effect.emblem-with-targets");
    }
    let mut all = abilities(ctx, p, "StaticAbilities", static_line)?;
    all.extend(abilities(ctx, p, "Triggers", triggered)?);
    all.extend(abilities(ctx, p, "ReplacementEffects", replacement_line)?);
    if all.is_empty() {
        return unmapped("key:Effect.emblem-without-abilities");
    }
    Ok(vec![json!({"CreateEmblem": {"abilities": all}})])
}
