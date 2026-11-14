//! "Plotting cards from your hand costs {2} less." (`GameRule::R6018PlotLess`): the plot cost of a card in a player's
//! hand loses that much generic mana for each such rule of a permanent the player controls (CR 601.2f, 702.170a).

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of;
use crate::state::GameState;
use crate::zones::Zone;

/// The generic mana the plot costs of `player`'s cards lose: the total of the plot rules of the permanents they control.
fn r6018_less(state: &GameState, player: PlayerId) -> u32 {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(player))
        .flat_map(|id| abilities_of(state, *id))
        .filter_map(|(_, ability)| match &*ability {
            Ability::Rule {
                rule: GameRule::R6018PlotLess(less),
                condition: None,
                from: ZoneKind::Battlefield,
            } => Some(*less),
            _ => None,
        })
        .fold(0, u32::saturating_add)
}

/// `cost`, the plot cost of the card `id`, less the generic mana the rules of the player whose hand holds it take off.
pub(crate) fn r6018_plot_cost(state: &GameState, id: ObjectId, cost: ManaCost) -> ManaCost {
    let Some(Zone::Hand(player)) = state.object(id).map(crate::object::GameObject::zone) else {
        return cost;
    };
    match r6018_less(state, player) {
        0 => cost,
        less => crate::announce::r6018_reduced(
            &cost,
            &ManaCost::from_symbols([ManaSymbol::Generic(less)]),
        ),
    }
}
