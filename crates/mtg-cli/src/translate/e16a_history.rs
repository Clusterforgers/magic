//! Facts of the turn's history (e16a): the permanents of a kind you've sacrificed this turn
//! (`PlayerCountPropertyYou$SacrificedThisTurn Artifact`), read from the turn's log as they last existed.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr};
use super::valid::selector_in;

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
