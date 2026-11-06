//! `DigMultiple`: look at the top cards and take at most one card for each alternative of the filter ("a creature card
//! and/or a land card"), a card never counting for two of them.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects_zone::dig_places;
use super::frame::moves;
use super::valid::{selector_any, subject};
use crate::forge::script::Params;

/// The keys of a `DigMultiple` line that are read.
const KEYS: [&str; 10] = [
    "DigNum",
    "ChangeValid",
    "Optional",
    "Reveal",
    "SourceZone",
    "DestinationZone",
    "DestinationZone2",
    "LibraryPosition",
    "RestRandomOrder",
    "Defined",
];

/// The effects of a `DigMultiple` line.
pub fn dig_multiple(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "DigMultiple", &KEYS)?;
    if matches!(p.get("SourceZone"), Some(zone) if zone != "Library") {
        return unmapped("key:DigMultiple.SourceZone");
    }
    // Taking a card for each alternative is optional ("you may"): the pieces without it choose differently.
    if p.get("Optional") != Some("True") {
        return unmapped("key:DigMultiple.Optional-absent");
    }
    let Some(valid) = p.get("ChangeValid") else {
        return unmapped("key:DigMultiple.ChangeValid-absent");
    };
    let (to, rest_to) = dig_places(p)?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("DigNum").unwrap_or("1"))?;
    let top = ctx.fresh("top");
    let from_top = json!({ "Bound": top });
    let mut effects = vec![json!({"LookAtTop": {"who": who, "count": count, "as": top}})];
    if p.get("Reveal") == Some("True") {
        effects.push(json!({"Reveal": {"what": from_top}}));
    }
    let mut kept = Vec::new();
    for alternative in valid.split(',') {
        let mut pool = vec![json!({"In": "Library"}), json!({"Is": from_top})];
        pool.extend(selector_any(ctx, alternative)?);
        pool.extend(super::valid::permanent_card(alternative)?);
        // A card taken for an earlier alternative is not taken again.
        for earlier in &kept {
            pool.push(json!({"Not": {"Is": earlier}}));
        }
        let name = ctx.fresh("kept");
        effects.push(json!({"Choose": {
            "from": {"Each": pool}, "as": name, "count": {"min": 0, "max": 1}}}));
        kept.push(json!({ "Bound": name }));
    }
    for card in &kept {
        effects.push(moves(card, to.clone()));
    }
    effects.push(moves(&from_top, rest_to));
    Ok(effects)
}
