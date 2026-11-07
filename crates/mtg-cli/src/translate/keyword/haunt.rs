//! Haunt (CR 702.55): the keyword, and the card's ability of the part the keyword names, which works twice: as the
//! creature enters (as the spell resolves, on an instant or sorcery) and when the creature the card haunts dies.

use serde_json::{Value, json};

use crate::translate::card::audited_one;
use crate::translate::ctx::{Ctx, Tr, unmapped};
use crate::translate::effects::chain_at;

/// `Haunt:<part>`: the keyword (the engine exiles the card haunting a creature), then "when this enters" on a creature
/// or the spell ability on an instant or sorcery, then "when the creature it haunts dies", each with the effects of the
/// part. A card of another type stays a gap.
pub fn haunt(ctx: &mut Ctx, name: &str) -> Vec<Tr<Value>> {
    let creature = ctx.types.contains("Creature");
    let spell = ctx.types.contains("Instant") || ctx.types.contains("Sorcery");
    if creature == spell || name.is_empty() || name.contains(':') {
        return vec![unmapped("keyword:Haunt-shape")];
    }
    let first = if creature {
        audited_one(|| {
            triggered(
                ctx,
                name,
                json!({"ZoneChange": {"what": ["This"], "to": "Battlefield"}}),
                None,
            )
        })
    } else {
        audited_one(|| spell_ability(ctx, name))
    };
    let haunted = json!({"ZoneChange": {"what": [{"HauntedBy": "This"}], "from": "Battlefield", "to": "Graveyard"}});
    vec![
        Ok(json!({"Keyword": "Haunt"})),
        first,
        audited_one(|| triggered(ctx, name, haunted, Some("Exile"))),
    ]
}

/// The effects of the part with the targets it declares.
fn effects(ctx: &mut Ctx, name: &str) -> Tr<(Vec<Value>, Vec<Value>)> {
    ctx.reset();
    let part = ctx.sub(name)?;
    let effects = chain_at(ctx, &part, false)?;
    Ok((effects, ctx.targets.clone()))
}

fn triggered(ctx: &mut Ctx, name: &str, when: Value, from: Option<&str>) -> Tr<Value> {
    let (effects, targets) = effects(ctx, name)?;
    let mut ability = json!({"when": when, "effects": effects});
    if let Some(zone) = from {
        ability["from"] = json!(zone);
    }
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets);
    }
    Ok(json!({ "Triggered": ability }))
}

fn spell_ability(ctx: &mut Ctx, name: &str) -> Tr<Value> {
    let (effects, targets) = effects(ctx, name)?;
    let mut ability = json!({"effects": effects});
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets);
    }
    Ok(json!({ "Spell": ability }))
}
