//! Replacements that a permanent holds beside the entering ones: skipped steps (`Event$ BeginPhase`).

use serde_json::{Value, json};

use super::ctx::{Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// `Event$ BeginPhase | Skip$ True`: the players skip the step (CR 614.10) as a rule of the permanent.
pub fn skip_step(p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    if line_condition.is_some() {
        return unmapped("key:Replacement.BeginPhase.condition");
    }
    check_keys(
        p,
        "Replacement.BeginPhase",
        &["Event", "ActiveZones", "ValidPlayer", "Phase", "Skip"],
        |_, _| false,
    )?;
    if p.get("Skip") != Some("True") || p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("replacement:BeginPhase");
    }
    let step = match p.get("Phase") {
        Some("Upkeep") => "Upkeep",
        Some("Draw") => "Draw",
        Some("Untap") => "Untap",
        _ => return unmapped("key:Replacement.BeginPhase.Phase"),
    };
    let who = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some("Opponent") => "Opponents",
        Some(_) => return unmapped("key:Replacement.BeginPhase.ValidPlayer"),
    };
    Ok(json!({"Rule": {"rule": {"Cant": {"who": who, "action": {"SkipStep": step}}}}}))
}
