//! `Effect::R112NoteManaSpent`: the source notes the type and amount of the mana spent to pay the activation cost of
//! the ability resolving ("Note the type of mana spent to pay this activation cost"), in place of what it noted before;
//! `Produce::R112Noted` adds it back. The note is source memory (`memory.rs`), so it ends when the source leaves.

use mtg_cards::effect::Effect;
use mtg_core::mana::ManaType;

use super::wrong_effect;
use crate::events::Event;
use crate::memory::{Held, Remembered};
use crate::resolve::{Cx, ResolveError, Run};

use crate::memory::R112_NOTED_SLOT;

/// Replaces the source's note with the mana the activation spent.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    if !matches!(effect, Effect::R112NoteManaSpent) {
        return Err(wrong_effect("R112NoteManaSpent"));
    }
    let values = cx
        .scope
        .mana_spent()
        .into_iter()
        .map(|mana| Held::Number(i64::try_from(mana.index()).unwrap_or(0)))
        .collect();
    Ok(Run::events(vec![Event::Remembered(Box::new(Remembered {
        source: cx.scope.source,
        slot: R112_NOTED_SLOT.to_owned(),
        objects: Vec::new(),
        values,
        replace: true,
    }))]))
}

/// The mana `source` last noted, in the order it was spent.
pub(crate) fn r112_noted(
    state: &crate::state::GameState,
    source: crate::object::ObjectRef,
) -> Vec<ManaType> {
    state
        .memory
        .get(&(source, R112_NOTED_SLOT.to_owned()))
        .map(|held| {
            held.iter()
                .filter_map(|value| match value {
                    Held::Number(at) => usize::try_from(*at)
                        .ok()
                        .and_then(|at| ManaType::ALL.get(at).copied()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
