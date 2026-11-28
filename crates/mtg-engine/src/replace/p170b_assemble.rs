//! Replacements of an assembly (Steamflogger Boss: "If a Rigger you control would assemble a Contraption, it assembles
//! two Contraptions instead"): each that applies to the object whose ability assembles multiplies how many it
//! assembles, once; the order they apply in does not change the product (CR 616.1).

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::gather;
use crate::object::ObjectRef;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// How many times as many Contraptions `assembler` assembles: the product of the replacements that apply to it now.
pub(crate) fn p170b_times(state: &GameState, assembler: ObjectRef) -> Result<u32, ResolveError> {
    let mut times: u32 = 1;
    for source in gather(state) {
        let Ability::Replacement {
            when: ReplaceWhen::P170bAssembles { what },
            by: ReplaceBy::Multiply(by),
            condition,
            ..
        } = &*source.ability
        else {
            continue;
        };
        let scope = Scope::announcing(state, source.controller, source.object);
        if !scope.selects(what, assembler)? {
            continue;
        }
        if let Some(condition) = condition
            && !scope.holds(condition)?
        {
            continue;
        }
        times = times.saturating_mul(*by);
    }
    Ok(times)
}
