//! Rules that last until the step or combat phase they make a player skip has been skipped once ("skips their next untap
//! step", "skips their next combat phase", CR 614.10): the skip uses the rule up.

use mtg_cards::modify::{Duration, Prohibited};
use mtg_core::ids::{EffectId, PlayerId};

use crate::events::Event;
use crate::prohibit::covers;
use crate::rules::Rule;
use crate::state::GameState;
use crate::triggers::step_matches;
use crate::turn::{Phase, Step};

/// The events that end the rules the steps `skipped` of `player`'s turn use up: for each kind of skip that one of the
/// steps answers, the oldest such rule in force. With `this_turn` false the rules that end with the turn anyway are left
/// out.
pub(crate) fn rmt3_used(
    state: &GameState,
    player: PlayerId,
    skipped: &[Step],
    this_turn: bool,
) -> Vec<Event> {
    let mut oldest: Vec<(Prohibited, EffectId)> = Vec::new();
    for effect in state.rule_effects() {
        let lasting = match effect.duration {
            Duration::UntilSkipped => true,
            Duration::UntilSkippedThisTurn => this_turn,
            _ => false,
        };
        let Rule::Cant { who, action } = &effect.rule else {
            continue;
        };
        if !lasting || !effect.is_active(state) || !covers(who, Some(player), None) {
            continue;
        }
        let answered = match **action {
            Prohibited::SkipCombat => skipped.iter().any(|step| step.phase() == Phase::Combat),
            Prohibited::SkipStep(named) => skipped.iter().any(|step| step_matches(named, *step)),
            _ => false,
        };
        if !answered {
            continue;
        }
        match oldest.iter_mut().find(|(kind, _)| kind == &**action) {
            Some(slot) => slot.1 = slot.1.min(effect.id),
            None => oldest.push(((**action).clone(), effect.id)),
        }
    }
    oldest
        .into_iter()
        .map(|(_, id)| Event::RuleExpired { id })
        .collect()
}
