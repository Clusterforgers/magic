//! Facts of the turn's history (e16a): the permanents of a kind you've sacrificed this turn
//! (`PlayerCountPropertyYou$SacrificedThisTurn Artifact`), read from the turn's log as they last existed, and a permanent
//! becoming tapped other than by attacking or for the first time in the turn.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::{first_time, keys};
use super::valid::{selector_any, selector_in};
use crate::forge::script::Params;

/// `SacrificedThisTurn <type>`: the permanents of the type you sacrificed this turn; `None` for another word or for the
/// untyped count, which the plain tally reads.
pub fn sacrificed(ctx: &Ctx, what: &str) -> Option<Tr<Value>> {
    let spec = what.strip_prefix("SacrificedThisTurn ")?;
    if spec == "Permanent" {
        return None;
    }
    Some(
        selector_in(ctx, spec, None).map(
            |filter| json!({"Tally": {"event": "Sacrificed", "who": "You", "filter": filter}}),
        ),
    )
}

/// `Taps` with `Attacker$ False` ("if it isn't being declared as an attacker") or `FirstTime$ True` (the first time this
/// turn the permanent becomes tapped); both together stay a gap.
pub fn taps(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Taps", &["ValidCard", "Attacker", "FirstTime"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    match (p.get("Attacker"), p.get("FirstTime")) {
        (Some("False"), None) => Ok(json!({"TappedNotAttacking": {"what": what}})),
        (None, Some(_)) => first_time(p, json!({"Tapped": {"what": what}})),
        _ => unmapped("key:Trigger.Tapped.Attacker"),
    }
}
