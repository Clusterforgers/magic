//! The writer table: each Forge API's `Remember*`/`Imprint*` keys and what they write. A key that names what the verb moved
//! binds the verb's `as` name, which `Defined$ Remembered` reads afterwards; a key that names what the verb acted on binds
//! the line's own object; an `Imprint*` key on a moving verb is the verb's remember key kept in the imprint slot.

use serde_json::{Value, json};

use super::ctx::Ctx;
use crate::forge::script::Params;

/// What a writer key puts in the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writes {
    /// The objects the verb moved or changed, bound by the verb (`as`).
    Moved,
    /// The object the line acts on (its target or `Defined$`), for a verb that cannot fail to act on it.
    ActedOn,
    /// What the verb moved, kept in the imprint slot: the verb reads the key as the named remember key.
    ImprintAs(&'static str),
}

/// One row: the Forge API (`ChangeZone.Hidden` is a move from a hidden zone), the key and what it writes.
pub struct Writer {
    /// The Forge API.
    pub api: &'static str,
    /// The key, set to `True`.
    pub key: &'static str,
    /// What it writes.
    pub writes: Writes,
}

const fn w(api: &'static str, key: &'static str, writes: Writes) -> Writer {
    Writer { api, key, writes }
}

/// The writer table.
pub const WRITERS: &[Writer] = &[
    w("Amass", "RememberAmass", Writes::Moved),
    w("Tap", "RememberTapped", Writes::Moved),
    w("TapAll", "RememberTapped", Writes::Moved),
    // The target is remembered whether or not it is destroyed (a regenerated creature is still "that creature").
    w("Destroy", "RememberDestroyed", Writes::Moved),
    w("Destroy", "RememberLKI", Writes::Moved),
    w("Destroy", "RememberTargets", Writes::Moved),
    w("DestroyAll", "RememberDestroyed", Writes::Moved),
    w("Counter", "RememberCountered", Writes::Moved),
    w("Sacrifice", "RememberSacrificed", Writes::Moved),
    w("SacrificeAll", "RememberSacrificed", Writes::Moved),
    w("Attach", "RememberAttached", Writes::Moved),
    w("Mill", "RememberMilled", Writes::Moved),
    w("ChangeZone", "RememberChanged", Writes::Moved),
    w("ChangeZone", "RememberLKI", Writes::Moved),
    w("ChangeZone", "RememberTargets", Writes::Moved),
    w("ChangeZone.Hidden", "RememberChanged", Writes::Moved),
    w("ChangeZoneAll", "RememberChanged", Writes::Moved),
    w("DealDamage", "RememberDamaged", Writes::ActedOn),
    w("PutCounter", "RememberPut", Writes::ActedOn),
    w("PutCounter", "RememberCards", Writes::ActedOn),
    w("Tap", "RememberTapped", Writes::ActedOn),
    w("GainControl", "RememberControlled", Writes::ActedOn),
    w("Pump", "RememberPumped", Writes::ActedOn),
    w("Goad", "RememberGoaded", Writes::ActedOn),
    w("Animate", "RememberAnimated", Writes::ActedOn),
    w("Phases", "RememberAffected", Writes::ActedOn),
    w(
        "Token",
        "ImprintTokens",
        Writes::ImprintAs("RememberTokens"),
    ),
    w(
        "CopyPermanent",
        "ImprintTokens",
        Writes::ImprintAs("RememberTokens"),
    ),
    w(
        "ChooseCard",
        "ImprintChosen",
        Writes::ImprintAs("RememberChosen"),
    ),
    w(
        "ChangeZone",
        "Imprint",
        Writes::ImprintAs("RememberChanged"),
    ),
    w(
        "ChangeZoneAll",
        "Imprint",
        Writes::ImprintAs("RememberChanged"),
    ),
    w("Dig", "Imprint", Writes::ImprintAs("RememberChanged")),
    w(
        "DigUntil",
        "ImprintFound",
        Writes::ImprintAs("RememberFound"),
    ),
    w(
        "DigUntil",
        "ImprintRevealed",
        Writes::ImprintAs("RememberRevealed"),
    ),
    w(
        "PeekAndReveal",
        "ImprintRevealed",
        Writes::ImprintAs("RememberRevealed"),
    ),
    w(
        "Reveal",
        "ImprintRevealed",
        Writes::ImprintAs("RememberRevealed"),
    ),
    w("Mill", "Imprint", Writes::ImprintAs("RememberMilled")),
];

/// The remember key the verb of `api` reads an `Imprint*` key as, when the table has a row for it.
pub fn imprint_as(api: &str, key: &str) -> Option<&'static str> {
    WRITERS.iter().find_map(|row| match row.writes {
        Writes::ImprintAs(own) if row.api == api && row.key == key => Some(own),
        _ => None,
    })
}

/// Whether the key is an acted-on writer of the API.
pub fn acts_on(api: &str, key: &str) -> bool {
    WRITERS
        .iter()
        .any(|row| row.api == api && row.key == key && row.writes == Writes::ActedOn)
}

/// Whether the step asks to remember what the verb of `api` moved, by any of its `Moved` keys set to `True`.
pub fn wanted(p: &Params, api: &str) -> bool {
    WRITERS
        .iter()
        .filter(|row| row.api == api && row.writes == Writes::Moved)
        .any(|row| p.get(row.key) == Some("True"))
}

/// Names the objects the step `verb` moved (`as`) and makes them what `Remembered` stands for in the lines after it.
pub fn bind_moved(ctx: &mut Ctx, effect: &mut Value, verb: &str, stem: &str) {
    let name = ctx.fresh(stem);
    ctx.remembered = Some(json!({ "Bound": name }));
    if matches!(
        verb,
        "Mill"
            | "Draw"
            | "Sacrifice"
            | "SacrificeUpTo"
            | "Destroy"
            | "Move"
            | "Exile"
            | "ExileHidden"
    ) {
        ctx.counted.insert(name.clone());
    }
    // A destroy can fail on an indestructible permanent, so what it named is not what it moved.
    if verb != "Destroy"
        && let Some(what) = effect[verb]
            .get("what")
            .filter(|what| what.get("Target").is_some())
            .cloned()
    {
        ctx.moved_from.insert(name.clone(), what);
    }
    effect[verb]["as"] = json!(name);
}
