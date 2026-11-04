//! `Static$ True` zone-change triggers that only clear Forge's remembered or imprinted objects: bookkeeping that the
//! DSL's bound names and `Effect` objects do not need, so the line prints nothing and is left out.

use super::ctx::Ctx;
use crate::forge::script::Params;

/// The keys of a clearing part that change nothing a card prints.
const CLEARS: [&str; 6] = [
    "DB",
    "ClearRemembered",
    "ClearImprinted",
    "ClearChosenCard",
    "ForgetDefined",
    "SubAbility",
];

/// Whether every line of the group is a stackless zone-change trigger whose parts only forget objects.
pub fn is_bookkeeping(ctx: &Ctx, group: &[&String]) -> bool {
    group.iter().all(|body| {
        let p = Params::parse(body);
        p.get("Mode") == Some("ChangesZone")
            && p.get("Static") == Some("True")
            && p.get("Execute")
                .is_some_and(|name| only_forgets(ctx, name, 0))
    })
}

fn only_forgets(ctx: &Ctx, name: &str, depth: usize) -> bool {
    let Some(text) = ctx.svar(name) else {
        return false;
    };
    let part = Params::parse(text);
    let forgets = match part.get("DB") {
        Some("Cleanup") => part.iter().all(|(key, _)| CLEARS.contains(&key)),
        Some("Pump") => {
            part.iter().all(|(key, _)| {
                matches!(
                    key,
                    "DB" | "ForgetImprinted" | "ForgetObjects" | "SubAbility"
                )
            }) && (part.has("ForgetImprinted") || part.has("ForgetObjects"))
        }
        _ => false,
    };
    forgets
        && match part.get("SubAbility") {
            Some(next) if depth < 8 => only_forgets(ctx, next, depth + 1),
            Some(_) => false,
            None => true,
        }
}
