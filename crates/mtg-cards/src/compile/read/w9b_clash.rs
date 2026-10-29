//! Clash: "Clash with an opponent. If you win, S." is a `Clash` with the win branch, and "You lose 2 life and draw two
//! cards, then clash with an opponent. If you win, repeat this process." a clash until a loss whose win branch repeats
//! the steps before it.

use super::conditional::body;
use super::effects::player_effect;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The words of the clash itself.
pub(super) const W9B_CLASH: &str = "clash with an opponent";

/// The words of the branch that repeats the steps before the clash.
pub(super) const W9B_REPEAT: &str = "repeat this process";

/// ". If you win, S": the effects of the win branch, none for "repeat this process".
fn w9b_branch(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.punct('.').then_some(())?;
    r.need("if you win")?;
    r.punct(',').then_some(())?;
    if r.words(W9B_REPEAT) {
        return Some(Vec::new());
    }
    if let Some(damage) = r.attempt(w9b_controller_damage) {
        return Some(vec![damage]);
    }
    let effects = body(r)?;
    (!effects.is_empty()).then_some(effects)
}

/// "~ deals 3 damage to that creature's controller": the controller of the one target of the sentences before.
fn w9b_controller_damage(r: &mut Reader<'_>) -> Option<Effect> {
    (r.targets.len() == 1).then_some(())?;
    r.source().then_some(())?;
    r.need("deals")?;
    let amount = r.number()?;
    r.need("damage to that creature's controller")?;
    Some(Effect::DealDamage {
        source: None,
        to: Ref::ControllerOf(Box::new(Ref::Target(0))),
        amount: Amount::Fixed(i32::try_from(amount).ok()?),
    })
}

/// "Clash with an opponent." with an optional win branch, or the steps of you, ", then" the clash and "If you win,
/// repeat this process."
pub(super) fn w9b_clash(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let lead = if r.words(W9B_CLASH) {
        Vec::new()
    } else {
        let lead = player_effect(r)?;
        r.punct(',').then_some(())?;
        r.need("then")?;
        r.need(W9B_CLASH)?;
        lead
    };
    let branch = r.attempt(w9b_branch);
    // Steps before the clash go only with "repeat this process", and a win branch only with no steps.
    let (win, until) = match branch {
        None => (Vec::new(), false),
        Some(effects) if effects.is_empty() && !lead.is_empty() => (lead.clone(), true),
        Some(effects) if !effects.is_empty() && lead.is_empty() => (effects, false),
        Some(_) => return None,
    };
    let mut effects = lead;
    effects.push(Effect::Clash {
        who: Ref::You,
        win,
        lose: Vec::new(),
        until,
        with: None,
    });
    Some(effects)
}
