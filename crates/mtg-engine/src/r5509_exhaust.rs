//! "During your turn, as long as you haven't activated an exhaust ability this turn, you may activate exhaust abilities
//! as though they haven't been activated." (`GameRule::ExhaustAgain`): an exhaust ability spent for the game is free
//! again for the controller of its source while the rule holds for them.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::ZoneKind;

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of;
use crate::object::ObjectRef;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// Whether the exhaust abilities of `source` may be activated as though they haven't been: it is its controller's turn,
/// they control a permanent with the rule, and they have activated no exhaust ability this turn.
pub(crate) fn r5509_again(state: &GameState, source: ObjectRef) -> bool {
    let Some(player) = controller_of(state, source.id) else {
        return false;
    };
    state.turn().active() == player
        && state
            .tallies
            .players
            .get(&(player, Tally::R5509Exhausted))
            .is_none_or(|count| *count == 0)
        && state.zone(Zone::Battlefield).iter().any(|id| {
            controller_of(state, *id) == Some(player)
                && abilities_of(state, *id).iter().any(|(_, ability)| {
                    matches!(
                        &**ability,
                        Ability::Rule {
                            rule: GameRule::ExhaustAgain,
                            condition: None,
                            from: ZoneKind::Battlefield,
                        }
                    )
                })
        })
}
