//! The legend rule (CR 704.5j): a player who controls two or more legendary permanents with the same name chooses one
//! and puts the rest into their owners' graveyards. The choice is a decision of the state-based action pass.

use std::collections::BTreeMap;

use mtg_core::card::Supertype;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::controller_of_object;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The legendary permanents one player controls that share a name, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    /// The player who controls them and chooses.
    pub(crate) player: PlayerId,
    /// The name they share.
    pub(crate) name: String,
    /// The permanents, by id.
    pub(crate) permanents: Vec<ObjectId>,
}

/// Every group the rule applies to: by player in turn order from the active player, then by name.
pub(crate) fn groups(state: &GameState) -> Vec<Group> {
    if !state.possible.legends {
        return Vec::new();
    }
    let mut found: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if !object.card().supertypes.contains(&Supertype::Legendary) {
            continue;
        }
        let controller = controller_of_object(state, object);
        if state.is_alive(controller) {
            found
                .entry((controller, object.card().name.clone()))
                .or_default()
                .push(id);
        }
    }
    let order = state.turn_order_from(state.turn().active());
    let place = |player: PlayerId| order.iter().position(|seat| *seat == player);
    let mut groups: Vec<Group> = found
        .into_iter()
        .filter(|(_, permanents)| permanents.len() > 1)
        .map(|((player, name), mut permanents)| {
            permanents.sort_unstable();
            Group {
                player,
                name,
                permanents,
            }
        })
        .collect();
    groups.sort_by_key(|group| place(group.player));
    groups
}

/// The question the rule asks now: the first group's controller chooses the one to keep. `None` when no player controls two
/// legendary permanents with one name.
pub(crate) fn decision(state: &GameState) -> Option<Decision> {
    let group = groups(state).into_iter().next()?;
    let source = *group.permanents.first()?;
    Some(Decision {
        player: group.player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            source,
            from: group.permanents,
            min: 1,
            max: 1,
            text: format!("Legend rule: choose the {} to keep.", group.name),
        }),
    })
}

/// The answer to that question: the permanents not chosen are put into their owners' graveyards, all together. They are
/// not destroyed.
pub(crate) fn answer(
    state: &GameState,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let events = options
        .from
        .iter()
        .filter(|id| !choice.0.contains(id))
        .filter_map(|id| state.object(*id))
        .map(|object| Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::StateBased,
        })
        .collect();
    Ok(Outcome::Events(events))
}
