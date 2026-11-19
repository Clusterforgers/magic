//! "You may have an opponent gain 3 life" as a cost (`CostItem::OpponentGainsLife`, CR 118.3, 601.2h): the payer
//! chooses one of their opponents as the cost is paid, kept in `CostTaken::players`, and that player gains the life
//! with the other parts of the payment; a player who can't gain life gains none, and the cost is still paid.

use mtg_core::ids::PlayerId;

use super::Part;
use crate::choose::Choices;
use crate::events::Event;
use crate::state::GameState;

/// The opponents of `payer` still in the game, in turn order: those the cost may choose.
pub(crate) fn r109_opponents(state: &GameState, payer: PlayerId) -> Vec<PlayerId> {
    state
        .opponents_of(payer)
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect()
}

/// The life the opponents chosen in `choices` gain, one for each part of `parts` that has an opponent gain life, in
/// the order the cost prints them.
pub(crate) fn r109_opponent_gains(
    state: &GameState,
    parts: &[Part],
    choices: &Choices,
) -> Vec<Event> {
    let mut chosen = choices.cost_players().iter();
    let mut events = Vec::new();
    for part in parts {
        let Part::OpponentGainsLife(amount) = part else {
            continue;
        };
        let Some(&player) = chosen.next() else {
            continue;
        };
        if *amount > 0 && !crate::rules::cant_gain_life(state, player) {
            events.push(Event::LifeGained {
                player,
                amount: *amount,
            });
        }
    }
    events
}
