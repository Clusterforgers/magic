//! `ETBReplacement` lines of a permanent that makes other permanents enter with counters ("each creature you control enters
//! with an additional +1/+1 counter"), and the line of a prepare card that enters prepared.

use serde_json::{Value, json};

use crate::forge::script::Params;
use crate::translate::amount::{amount, counter_kind};
use crate::translate::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::translate::valid::selector_any;

/// The zones a source may work from, in the DSL's names.
const ZONES: [&str; 5] = ["Battlefield", "Graveyard", "Hand", "Exile", "Command"];

/// `ETBReplacement:Other:<part>:Mandatory:<zone>:<valid>`: every permanent `valid` names enters with the counters the part
/// puts on `ReplacedCard`, while the source is in `zone`. The count is read from the source; any other key of the part is a gap.
pub fn others(ctx: &mut Ctx, name: &str, zone: &str, valid: &str) -> Tr<Value> {
    let part = ctx.sub(name)?;
    if part.get("DB") != Some("PutCounter") || part.get("Defined") != Some("ReplacedCard") {
        return unmapped("keyword:ETBReplacement-others-shape");
    }
    check_keys(
        &part,
        "ETBReplacement",
        &["DB", "ETB", "Defined", "CounterType", "CounterNum"],
        |_, _| false,
    )?;
    if part.get("ETB") != Some("True") {
        return unmapped("keyword:ETBReplacement-others-shape");
    }
    let Some(from) = ZONES.iter().find(|known| **known == zone) else {
        return unmapped(format!("key:ETBReplacement.ActiveZone={zone}"));
    };
    let Some(kind) = part.get("CounterType").and_then(counter_kind) else {
        return unmapped("key:ETBReplacement.CounterType");
    };
    let Some(count) = part.get("CounterNum") else {
        return unmapped("key:ETBReplacement.CounterNum");
    };
    let count = amount(ctx, count)?;
    let what = selector_any(ctx, valid)?;
    let mut replacement = json!({
        "when": {"Enters": {"what": what}},
        "by": {"WithCounters": {"counter": kind, "count": count}},
    });
    if *from != "Battlefield" {
        replacement["from"] = json!(from);
    }
    Ok(json!({ "Replacement": replacement }))
}

/// `ETBReplacement:Other:<part>` where the part makes the card prepared as it enters: `Attributes$ Prepared` with no
/// condition and no other target.
pub fn prepared(part: &Params) -> Tr<Value> {
    check_keys(part, "ETBReplacement", &["DB", "Attributes"], |_, _| false)?;
    match part.get("Attributes") {
        Some("Prepared") => Ok(json!("EntersPrepared")),
        _ => unmapped("keyword:ETBReplacement-AlterAttribute"),
    }
}
