//! `Effect::R111OrderGraveyard`: a player reorders their graveyard as they choose (CR 404.2).

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The words of the question.
const R111_ORDER_GRAVEYARD: &str =
    "Put the cards of your graveyard in any order, the top card first.";

/// Each player the effect names who is still in the game, in turn order, puts the cards of their graveyard in an order
/// they choose (top card first), logged as `R111GraveyardOrdered`; a graveyard of fewer than two cards asks nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R111OrderGraveyard { who } = effect else {
        return Err(wrong_effect("R111OrderGraveyard"));
    };
    let state = cx.state();
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(r111_orders(cx, &players).map(Run::events))
}

/// The new order of each graveyard, asking in turn.
fn r111_orders(cx: &Cx<'_>, players: &[PlayerId]) -> Result<Vec<Event>, Stop> {
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    for player in players {
        let cards = cx.state().zone(Zone::Graveyard(*player)).to_vec();
        if cards.len() < 2 {
            continue;
        }
        // The zone lies bottom to top; the player is shown and answers the top card first.
        let mut order = questions.order(
            *player,
            cards.into_iter().rev().collect(),
            R111_ORDER_GRAVEYARD.to_owned(),
        )?;
        order.reverse();
        events.push(Event::R111GraveyardOrdered {
            player: *player,
            order,
        });
    }
    questions.end()?;
    Ok(events)
}
