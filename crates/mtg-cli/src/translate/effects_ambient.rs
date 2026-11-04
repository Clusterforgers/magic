//! An `Effect` object with static abilities and no remembered objects ("creatures you control get +1/+0 and gain haste
//! this turn"): the continuous effects of its statics for the effect's duration.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::effects_obj::duration;
use super::statics::static_line;
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
        let line = ctx.nested(|ctx| static_line(ctx, text))?;
        if let Some(rule) = line.get("Rule").and_then(|rule| rule.get("rule")) {
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
