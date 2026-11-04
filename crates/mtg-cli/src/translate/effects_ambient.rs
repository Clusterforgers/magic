//! An `Effect` object with static abilities and no remembered objects ("creatures you control get +1/+0 and gain haste
//! this turn"): the continuous effects of its statics for the effect's duration.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::effects_obj::duration;
use super::statics::static_line;
use super::triggers::triggered;
use crate::forge::script::Params;

/// The statics `StaticAbilities$` names as `Continuous` effects over what they affect now.
pub fn continuous_statics(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let duration = duration(p.get("Duration"))?;
    let Some(names) = p.get("StaticAbilities") else {
        return unmapped("key:Effect.no-StaticAbilities");
    };
    let mut out = Vec::new();
    for name in names.split(',') {
        let Some(text) = ctx.svar(name) else {
            return unmapped("svar:missing");
        };
        // The effect belongs to the card whose ability made it, which is `This` here; Forge's `Self` is the effect object.
        let text = text.replace(".EffectSource", ".Self");
        let line = ctx.nested(None, |ctx| static_line(ctx, &text))?;
        if let Some(rule) = line.get("Rule").and_then(|rule| rule.get("rule")) {
            if line.pointer("/Rule/condition").is_some() {
                return unmapped("effect:Effect-rule-with-condition");
            }
            out.push(json!({"Rule": {"rule": rule, "duration": duration}}));
            continue;
        }
        let Some(inner) = line.get("Static").and_then(Value::as_object) else {
            return unmapped("effect:Effect-static-not-a-continuous-change");
        };
        if inner.keys().any(|key| key != "affected" && key != "mods") {
            return unmapped("effect:Effect-static-with-options");
        }
        let (Some(affected), Some(mods)) = (inner.get("affected"), inner.get("mods")) else {
            return unmapped("effect:Effect-static-without-affected");
        };
        out.push(json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}));
    }
    Ok(out)
}

/// The triggers `Triggers$` names as delayed triggers that fire every time for the effect's duration; `remembered` is
/// what the effect remembers, which a trigger reads as `IsRemembered`.
pub fn delayed_triggers(ctx: &mut Ctx, p: &Params, remembered: Option<&Value>) -> Tr<Vec<Value>> {
    let this_turn = match p.get("Duration") {
        None | Some("EOT") => true,
        Some("Permanent") => false,
        Some(other) => return unmapped(format!("duration:{other}")),
    };
    let Some(names) = p.get("Triggers") else {
        return unmapped("key:Effect.no-Triggers");
    };
    let mut out = Vec::new();
    for name in names.split(',') {
        let Some(text) = ctx.svar(name) else {
            return unmapped("svar:missing");
        };
        let body = without_zones(text);
        let line = ctx.nested(remembered.cloned(), |ctx| triggered(ctx, &body))?;
        let Some(inner) = line.get("Triggered").and_then(Value::as_object) else {
            return unmapped("effect:Effect-trigger-not-triggered");
        };
        if inner.keys().any(|key| key != "when" && key != "effects") {
            return unmapped("effect:Effect-trigger-with-options");
        }
        let (Some(when), Some(effects)) = (inner.get("when"), inner.get("effects")) else {
            return unmapped("effect:Effect-trigger-without-parts");
        };
        // "This" would be the Effect object in Forge, not the card whose ability made it.
        if when.to_string().contains("\"This\"") || effects.to_string().contains("\"This\"") {
            return unmapped("effect:Effect-trigger-about-the-source");
        }
        let boon = p.get("Boon") == Some("True");
        let mut delayed = json!({"when": when, "once": boon, "effects": effects});
        if this_turn {
            delayed["this_turn"] = json!(true);
        }
        out.push(json!({"DelayedTrigger": delayed}));
    }
    Ok(out)
}

/// The trigger line without the zone it works from, which is the command zone for an `Effect`.
fn without_zones(body: &str) -> String {
    let parsed = Params::parse(body);
    let kept: Vec<String> = parsed
        .iter()
        .filter(|(key, value)| !(*key == "TriggerZones" && *value == "Command"))
        .map(|(key, value)| format!("{key}$ {value}"))
        .collect();
    kept.join(" | ")
}
