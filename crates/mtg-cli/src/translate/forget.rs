//! The `Forget*` keys of a line: they empty or trim the `Remembered` slot as the line acts, one table row per key.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::memory::Slot;
use crate::forge::script::Params;

/// What a key does to the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forgets {
    /// Everything the slot held goes before the line writes it.
    All,
    /// What the line moved leaves the slot: the engine drops a remembered object that moves (CR 400.7), so it needs no step.
    Moved,
}

/// The keys and what each does.
const KEYS: [(&str, Forgets); 4] = [
    ("ForgetOtherRemembered", Forgets::All),
    ("ForgetOtherTargets", Forgets::All),
    ("ForgetChanged", Forgets::Moved),
    ("ForgetPlayed", Forgets::Moved),
];

/// The line without its `Forget*` keys, and what they asked for.
pub fn split(p: &Params) -> Option<(Params, Vec<Forgets>)> {
    let found: Vec<(&str, Forgets)> = KEYS
        .into_iter()
        .filter(|(key, _)| p.get(key) == Some("True"))
        .collect();
    if found.is_empty() {
        return None;
    }
    let keys: Vec<&str> = found.iter().map(|(key, _)| *key).collect();
    Some((
        p.without(&keys),
        found.into_iter().map(|row| row.1).collect(),
    ))
}

/// The effects of the line with what its `Forget*` keys do added. A `Moved` key needs the slot to live on the source (a
/// name bound on the chain would still hold what moved), else the line stays a gap.
pub fn apply(
    ctx: &mut Ctx,
    asked: &[Forgets],
    before: &Option<Value>,
    mut effects: Vec<Value>,
) -> Tr<Vec<Value>> {
    let across = ctx.memory.crosses(Slot::Remembered);
    if asked.contains(&Forgets::Moved) && !across && before.is_some() {
        return unmapped("key:Forget-moved-from-a-bound-name");
    }
    if !asked.contains(&Forgets::All) {
        return Ok(effects);
    }
    if across {
        // The step goes before the line's own write, which is its last effect.
        let write = effects
            .iter()
            .rposition(|effect| effect.get("Remember").is_some());
        let step = json!({"Forget": {"slot": Slot::Remembered.name()}});
        match write {
            Some(at) => effects.insert(at, step),
            None => effects.push(step),
        }
    } else if ctx.remembered == *before {
        ctx.remembered = None;
    }
    Ok(effects)
}
