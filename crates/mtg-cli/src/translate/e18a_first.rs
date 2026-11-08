//! The turn's activations by kind (e18a): `Count$ThisTurnActivated_Activated.Equip+YouCtrl` as a tally of the abilities of
//! the kind you activated this turn, and `Mode$ AlternativeCost` of an activated ability ("you may pay {0} rather than pay
//! the equip cost of the first equip ability you activate each turn") as a `FreeActivation`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The DSL kind of an ability-kind word of a valid-string (`Activated.Equip`); none for another word.
fn kind(word: &str) -> Option<&'static str> {
    Some(match word {
        "Equip" => "Equip",
        "Cycling" => "Cycling",
        "Loyalty" => "Loyalty",
        "Unearth" => "Unearth",
        _ => return None,
    })
}

/// `ThisTurnActivated_Activated.<kind>+YouCtrl+<more>`: the abilities you activated this turn, of the kind when one is
/// named, whose source the rest of the valid-string selects as it was.
pub fn this_turn_activated(ctx: &Ctx, word: &str) -> Tr<Value> {
    let Some(spec) = word.strip_prefix("ThisTurnActivated_Activated.") else {
        return unmapped("amount:Count$ThisTurnActivated_Activated");
    };
    let mut parts: Vec<&str> = spec.split('+').collect();
    let Some(you) = parts.iter().position(|part| *part == "YouCtrl") else {
        return unmapped("amount:Count$ThisTurnActivated_Activated-of-another-player");
    };
    parts.remove(you);
    let named = parts.first().and_then(|first| kind(first));
    if named.is_some() {
        parts.remove(0);
    }
    // A loyalty ability is a planeswalker's: the type adds nothing.
    if named == Some("Loyalty") && parts.first() == Some(&"Planeswalker") {
        parts.remove(0);
    }
    if parts
        .iter()
        .any(|part| part.contains(' ') || part.starts_with("IsTargeting"))
    {
        return unmapped("amount:Count$ThisTurnActivated_Activated-targeting");
    }
    // Its one card (Laboratory Drudge) checks it as the trigger resolves ("draw a card if ..."), which an intervening "if"
    // does not say.
    if parts.contains(&"inZoneGraveyard") {
        return unmapped("amount:Count$ThisTurnActivated_Activated-in-a-graveyard");
    }
    // A type word leads the rest of the valid-string (`Artifact+inZoneBattlefield`); properties alone are of any card.
    let (head, props) = match parts.split_first() {
        None => ("Card", &[][..]),
        Some((first, rest)) if first.starts_with(char::is_uppercase) => (*first, rest),
        Some(_) => ("Card", &parts[..]),
    };
    let filter = match (head, props) {
        ("Card", []) => Vec::new(),
        (head, []) => selector_any(ctx, head)?,
        (head, props) => selector_any(ctx, &format!("{head}.{}", props.join("+")))?,
    };
    let event = named.map_or_else(|| json!("Activated"), |kind| json!({"KindActivated": kind}));
    Ok(json!({"Tally": {"event": event, "who": "You", "filter": filter}}))
}

/// `AlternativeCost` of the activated abilities of a kind (`ValidSA$ Activated.Equip`) with a cost of {0}: a
/// `FreeActivation`; `None` for a line about spells. The condition keys are already stripped from `p`.
pub fn free_activation(p: &Params) -> Option<Tr<Value>> {
    let word = p.get("ValidSA")?.strip_prefix("Activated.")?;
    Some(free(p, word))
}

/// `CastWithFlash` of the activated abilities of a kind (`ValidSA$ Activated.Equip`, "you may activate equip abilities any
/// time you could cast an instant") as an `InstantActivation`; `None` for a line about spells.
pub fn instant_activation(p: &Params) -> Option<Tr<Value>> {
    if p.get("Mode") != Some("CastWithFlash") {
        return None;
    }
    let word = p.get("ValidSA")?.strip_prefix("Activated.")?;
    Some(instant(p, word))
}

fn instant(p: &Params, word: &str) -> Tr<Value> {
    check_keys(
        p,
        "Static.CastWithFlash",
        &["Mode", "ValidSA", "Caster"],
        |key, _| key == "Mode",
    )?;
    // A loyalty ability keeps its own timing (CR 606.3), which no row here changes.
    let Some(kind) = kind(word).filter(|kind| *kind == "Equip") else {
        return unmapped(format!("static:CastWithFlash-of-Activated.{word}"));
    };
    if p.get("Caster") != Some("You") {
        return unmapped("static:CastWithFlash-of-an-activation-for-another-player");
    }
    Ok(json!({"InstantActivation": {"kind": kind}}))
}

fn free(p: &Params, word: &str) -> Tr<Value> {
    check_keys(
        p,
        "Static.AlternativeCost",
        &["Mode", "ValidSA", "ValidPlayer", "Cost", "CostDesc"],
        |key, _| key == "Mode",
    )?;
    let Some(kind) = kind(word).filter(|kind| *kind != "Loyalty") else {
        return unmapped(format!("static:AlternativeCost-of-Activated.{word}"));
    };
    if p.get("ValidPlayer") != Some("You") {
        return unmapped("static:AlternativeCost-of-an-activation-for-another-player");
    }
    // Cycling's discard stays a part of its cost: "Discard this card" in place of the cycling cost and the discard is {0}.
    let zero = match (p.get("Cost"), p.get("CostDesc")) {
        (Some("0"), None) => true,
        (Some("Discard<1/CARDNAME>"), Some("0")) => kind == "Cycling",
        _ => false,
    };
    if !zero {
        return unmapped("static:AlternativeCost-of-an-activation-cost");
    }
    Ok(json!({"FreeActivation": {"kind": kind}}))
}
