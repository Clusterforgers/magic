//! Effects of Alchemy's digital mechanics: conjure (`MakeCard`) and seek (`Seek`).

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, selector_any};
use crate::forge::script::Params;

/// The effects of a digital verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "MakeCard" => make_card(ctx, p),
        "Seek" => seek(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// A card named in the script, conjured into a hand, a graveyard or exile. A name picked at the table, a copy of a card, a
/// spellbook, a token card, a library or the battlefield stay gaps: the node conjures the named card and nothing else.
fn make_card(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "MakeCard",
        &["Name", "Conjure", "Zone", "Amount", "Defined"],
    )?;
    if p.get("Conjure") != Some("True") {
        return unmapped("key:MakeCard.Conjure");
    }
    let Some(name) = p.get("Name") else {
        return unmapped("key:MakeCard.Name");
    };
    if name == "ChosenName" || name.contains('$') {
        return unmapped("key:MakeCard.Name");
    }
    // A card that conjures a name the game cannot resolve would play wrong and still count as automatic, so the name must
    // be a stored card; a run that cannot tell (no database) leaves the line a gap.
    match &ctx.world.stored {
        Some(stored) if stored.contains(&mtg_cards::load::slug(name)) => {}
        Some(_) => return unmapped("name:MakeCard.Unstored"),
        None => return unmapped("name:MakeCard.Unchecked"),
    }
    let zone = match p.get("Zone") {
        Some(zone @ ("Hand" | "Graveyard" | "Exile")) => zone,
        Some(other) => return unmapped(format!("zone:MakeCard.{other}")),
        None => return unmapped("key:MakeCard.Zone"),
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    Ok(json!({"Conjure": {"who": who, "name": name, "zone": zone, "count": count}}))
}

/// A seek of cards that match the type; the card sought and put into the hand. What the card does with it afterwards (a
/// remembered find) stays a gap.
fn seek(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Seek", &["Type", "Num", "Defined"])?;
    let Some(spec) = p.get("Type") else {
        return unmapped("key:Seek.Type");
    };
    let filter = selector_any(ctx, spec)?;
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let count = amount(ctx, p.get("Num").unwrap_or("1"))?;
    Ok(json!({"Seek": {"who": who, "filter": filter, "count": count}}))
}
