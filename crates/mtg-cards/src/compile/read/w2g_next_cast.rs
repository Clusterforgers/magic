//! The next spell cast: "When you next cast an instant or sorcery spell this turn, copy that spell. You may choose new
//! targets for the copy." is a `DelayedTrigger` on that cast, once and for this turn, whose effect copies the spell.

use super::copy_spells;
use super::tokens::Reader;
use super::triggers::{pattern_text, w2g_next_spell};
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// "when you next cast <spell> [this turn], copy that spell ...": a delayed trigger on the next such cast of yours.
pub fn w2g_next_cast(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("when you next cast")?;
    let what = w2g_next_spell(r)?;
    let this_turn = r.words("this turn");
    r.punct(',').then_some(())?;
    // The copy names the spell cast, as it does after a trigger on a cast.
    let outer = (r.cast_spell, r.cast_itself);
    (r.cast_spell, r.cast_itself) = (true, false);
    let effects = copy_spells::copy(r);
    (r.cast_spell, r.cast_itself) = outer;
    Some(vec![Effect::DelayedTrigger {
        when: TriggerPattern::SpellCast { what, by: Ref::You },
        once: true,
        this_turn,
        effects: effects?,
    }])
}

/// The words of [`w2g_next_cast`] without the first capital and the period, from the copy's own words.
pub(super) fn w2g_next_cast_text(effect: &Effect) -> Option<String> {
    let Effect::DelayedTrigger {
        when: when @ TriggerPattern::SpellCast { by: Ref::You, .. },
        once: true,
        this_turn,
        effects,
    } = effect
    else {
        return None;
    };
    let [copy] = effects.as_slice() else {
        return None;
    };
    if !matches!(
        copy,
        Effect::CopySpell {
            what: Ref::Triggering,
            ..
        }
    ) {
        return None;
    }
    let spell = pattern_text(when)?
        .strip_prefix("Whenever you cast ")?
        .to_owned();
    let turn = if *this_turn { " this turn" } else { "" };
    Some(format!(
        "when you next cast {spell}{turn}, {}",
        copy_spells::cast_text(copy)?
    ))
}
