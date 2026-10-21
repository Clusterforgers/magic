//! How an amount is divided among the targets (CR 601.2d): "3 damage divided as you choose among up to three targets".
//!
//! A specification with a `divide` amount asks once its targets are chosen. One target gets the whole amount without a
//! question, none gets nothing, and an amount smaller than the number of targets (each needs at least one) rewinds.

use mtg_cards::effect::Effect;
use mtg_cards::value::Amount;

use super::{Ctx, Next, Work, targets};
use crate::choose::{Choices, DivideOptions};
use crate::decision::DecisionKind;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let specs = targets::specs(ctx, &work.choices.modes);
    while work.done.divided < specs.len() {
        let at = work.done.divided;
        let Some(divide) = &specs[at].divide else {
            work.done.divided += 1;
            continue;
        };
        let Some(total) = total_of(ctx, work.choices, divide) else {
            return Some(Next::Drop);
        };
        let among = work.choices.targets.get(at).cloned().unwrap_or_default();
        match among.as_slice() {
            [] => {}
            [only] => work.choices.division.push((*only, total)),
            many => {
                if usize::try_from(total).map_or(true, |total| total < many.len()) {
                    return Some(Next::Drop);
                }
                return Some(ctx.ask(DecisionKind::DivideAmount(DivideOptions {
                    source: ctx.source,
                    total,
                    among: among.clone(),
                    text: wording(ctx, total),
                    can_cancel: ctx.can_cancel(),
                })));
            }
        }
        work.done.divided += 1;
    }
    None
}

/// The amount to divide, worked out with what has been announced so far (X may be part of it).
fn total_of(ctx: &Ctx<'_>, choices: &Choices, divide: &Amount) -> Option<u32> {
    let origin = ctx.state.object(ctx.source).map_or(
        ObjectRef {
            id: ctx.source,
            counter: 0,
        },
        GameObject::reference,
    );
    let mut scope = Scope::announcing(ctx.state, ctx.player, origin);
    scope.choices = choices;
    u32::try_from(scope.amount(divide).ok()?).ok()
}

/// What is divided, in the words of a card.
fn wording(ctx: &Ctx<'_>, total: u32) -> String {
    let damage = ctx.ability.is_some_and(|ability| {
        ability
            .effects()
            .iter()
            .any(|effect| matches!(effect, Effect::DealDamage { .. }))
    });
    let what = if damage { "damage" } else { "counters" };
    format!("Divide {total} {what} among the targets.")
}
