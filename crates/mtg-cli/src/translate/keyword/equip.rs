//! Equip with options (CR 702.6): an alternative cost, a once-a-turn limit or a reduction in the line's option fields.

use serde_json::{Value, json};

use crate::forge::script::Params;
use crate::translate::amount::amount;
use crate::translate::cost::activated;
use crate::translate::ctx::{Ctx, Tr, check_keys, unmapped};

/// `Equip:<cost>:<fields>`: the activated abilities of equip, one for each way to pay ("Equip {2} or {B}" is two
/// abilities that attach alike). A field with a `$` is an option (`ActivationLimit`, `ReduceCost`, `AlternateCost`);
/// another option stays a gap, and so does a restriction next to an option.
pub fn equip_with(ctx: &mut Ctx, fields: &[&str]) -> Vec<Tr<Value>> {
    let [_, price, rest @ ..] = fields else {
        return vec![unmapped("keyword:Equip-shape")];
    };
    let joined: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|field| field.contains('$'))
        .collect();
    let options = Params::parse(&joined.join(" | "));
    if rest
        .iter()
        .any(|field| field.contains(['.', '+']) && !field.contains('$') && !field.contains(' '))
    {
        return vec![unmapped("keyword:Equip-restriction-with-options")];
    }
    if let Err(gap) = check_keys(
        &options,
        "Equip",
        &["ActivationLimit", "ReduceCost", "AlternateCost"],
        |_, _| false,
    ) {
        return vec![Err(gap)];
    }
    let mut ways = vec![*price];
    ways.extend(options.get("AlternateCost"));
    ways.into_iter()
        .map(|way| attach(ctx, way, &options))
        .collect()
}

/// The ability that attaches the Equipment for one cost.
fn attach(ctx: &mut Ctx, price: &str, options: &Params) -> Tr<Value> {
    let mut price = activated(ctx, price)?;
    if let Some(text) = options.get("ReduceCost") {
        if text.trim().parse::<i32>().is_ok() {
            return unmapped("keyword:Equip-fixed-reduction");
        }
        let less =
            json!({"Changed": {"change": "Reduce", "mana": "{1}", "per": amount(ctx, text)?}});
        price = match price {
            Value::String(mana) => json!([{ "Mana": mana }, less]),
            Value::Array(mut items) => {
                items.push(less);
                Value::Array(items)
            }
            _ => return unmapped("keyword:Equip-reduction-shape"),
        };
    }
    let mut ability = json!({
        "cost": price,
        "timing": "Sorcery",
        "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}],
        "effects": [{"Attach": {"what": "This", "to": {"Target": 0}}}]});
    match options.get("ActivationLimit") {
        None => {}
        Some("1") => ability["limit"] = json!("OncePerTurn"),
        Some(_) => return unmapped("keyword:Equip-limit"),
    }
    Ok(json!({ "Activated": ability }))
}
