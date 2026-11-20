//! "You may plot nonland cards from the top of your library." (`GameRule::R111PlotFromLibraryTop`): the top card of a
//! player's library is plotted as a card of their hand is (CR 702.170a) while a permanent they control has such a rule
//! whose selector picks it.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The top card of `player`'s library when a permanent they control lets them plot it.
pub(crate) fn r111_plottable_top(state: &GameState, player: PlayerId) -> Option<ObjectId> {
    let top = *crate::library::top_cards(state, player, 1).first()?;
    r111_may_plot_from_top(state, player, top).then_some(top)
}

/// Whether `player` may plot the card `id` from their library: it is the top card and a rule of a permanent they control
/// picks it.
pub(crate) fn r111_may_plot_from_top(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    if card.zone() != Zone::Library(player)
        || crate::library::top_cards(state, player, 1).first() != Some(&id)
    {
        return false;
    }
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|source| controller_of(state, **source) == Some(player))
        .any(|source| {
            abilities_of(state, *source)
                .iter()
                .any(|(_, ability)| match &**ability {
                    Ability::Rule {
                        rule: GameRule::R111PlotFromLibraryTop(what),
                        condition: None,
                        from: ZoneKind::Battlefield,
                    } => {
                        let mut what: Selector = what.clone();
                        what.0.push(Pred::In(ZoneKind::Library));
                        state.object(*source).is_some_and(|object| {
                            Scope::announcing(state, player, object.reference())
                                .selects(&what, card.reference())
                                .unwrap_or(false)
                        })
                    }
                    _ => false,
                })
        })
}
