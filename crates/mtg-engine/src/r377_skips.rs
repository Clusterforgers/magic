//! Rules that last for the next turn of the player they name ("Target player skips all combat phases of their next
//! turn", CR 614.10): `Duration::TheirNextTurn` applies from a later turn on and ends with the first turn of that player.

use mtg_cards::modify::Duration;

use crate::events::Event;
use crate::prohibit::covers;
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;

/// The expiries of the rules whose player's next turn is the turn that is ending.
pub(crate) fn r377_their_turn_over(state: &GameState) -> Vec<Event> {
    let turn = state.turn();
    let active = turn.active();
    state
        .rule_effects()
        .iter()
        .filter(|effect| effect.duration == Duration::TheirNextTurn && turn.number() > effect.turn)
        .filter(|effect| match &effect.rule {
            Rule::Cant { who, .. } => covers(who, Some(active), None),
            Rule::MustAttackAt { turn_of, .. } => turn_of.contains(&active),
            _ => true,
        })
        .map(|effect| Event::RuleExpired { id: effect.id })
        .collect()
}

/// Whether the rule waits for a later turn: one for the next turn of its player, begun this turn, and a prohibition for
/// that turn during the turns of other players before it ("can't cast instant or sorcery spells during that player's next
/// turn").
pub(crate) fn r377_waits(state: &GameState, effect: &RuleEffect) -> bool {
    effect.duration == Duration::TheirNextTurn
        && (state.turn().number() <= effect.turn
            || match &effect.rule {
                Rule::Cant { who, .. } => !covers(who, Some(state.turn().active()), None),
                _ => false,
            })
}
