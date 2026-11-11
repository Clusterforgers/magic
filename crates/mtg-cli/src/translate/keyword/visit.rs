//! Visit (CR 702.159): `K:Visit:<part>` is the visit ability of an Attraction, a triggered ability whose effects are the
//! part's; the engine triggers it when its controller's roll to visit lights it up.

use serde_json::{Value, json};

use crate::translate::card::audited_one;
use crate::translate::ctx::{Ctx, Tr, unmapped};
use crate::translate::effects::chain_at;

/// The visit ability with the effects of the part `name`; a card that is no Attraction stays a gap.
pub fn visit(ctx: &mut Ctx, name: &str) -> Vec<Tr<Value>> {
    if !ctx.types.contains("Attraction") || name.is_empty() || name.contains(':') {
        return vec![unmapped("keyword:Visit-shape")];
    }
    vec![audited_one(|| {
        ctx.reset();
        let part = ctx.sub(name)?;
        let effects = chain_at(ctx, &part, false)?;
        let mut ability = json!({"when": {"Visit": {}}, "effects": effects});
        if !ctx.targets.is_empty() {
            ability["targets"] = Value::Array(ctx.targets.clone());
        }
        Ok(json!({ "Triggered": ability }))
    })]
}
