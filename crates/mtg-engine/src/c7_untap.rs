//! The end of a rule that a permanent doesn't untap during its controller's next untap step (CR 502.3, 611.2b).

use mtg_cards::modify::Duration;
use mtg_core::ids::PlayerId;

use crate::abilities::Grant;
use crate::characteristics::controller_of;
use crate::effects::Modification;
use crate::events::Event;
use crate::rules::Rule;
use crate::state::GameState;
use crate::zones::Zone;

/// The `EffectExpired` events of the untap step of the `actives` (several on a shared turn, CR 805.3): each effect
/// that lasts through the next untap step and holds down a permanent one of them controls is used up by this step,
/// whether that permanent is tapped or not, and expires once however many of them it holds down. The untap of the
/// step reads the effects before these events remove them. The exerts of each of them end with them (`ExertEnded`,
/// `w11a_exert`).
pub(crate) fn c7_untap_spent(state: &GameState, actives: &[PlayerId]) -> Vec<Event> {
    let mut spent: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id).is_some_and(|c| actives.contains(&c)))
        .filter_map(|id| state.layers().get(*id))
        .flat_map(|held| &held.applied)
        .filter(|applied| matches!(applied.modification, Modification::DoesntUntap))
        .filter_map(|applied| match applied.origin {
            Grant::Effect(id) | Grant::R6701Nested { effect: id, .. } => Some(id),
            Grant::Static { .. }
            | Grant::Borrowed { .. }
            | Grant::R111Lent { .. }
            | Grant::Ring { .. }
            | Grant::R114Sticker(_) => None,
        })
        .filter(|id| {
            state
                .effects()
                .iter()
                .any(|e| e.id == *id && e.duration == Duration::NextUntapStep)
        })
        .collect();
    spent.sort();
    spent.dedup();
    // An untap limit through the next untap step of a player it names ends with that step.
    let limits = state
        .rule_effects()
        .iter()
        .filter(|effect| {
            effect.duration == Duration::NextUntapStep
                && matches!(&effect.rule, Rule::UntapLimit { who, .. } if actives.iter().any(|a| who.contains(a)))
        })
        .map(|effect| Event::RuleExpired { id: effect.id });
    spent
        .into_iter()
        .map(|id| Event::EffectExpired { id })
        .chain(limits)
        .chain(
            actives
                .iter()
                .flat_map(|active| crate::w11a_exert::w11a_spent(state, *active)),
        )
        .collect()
}
