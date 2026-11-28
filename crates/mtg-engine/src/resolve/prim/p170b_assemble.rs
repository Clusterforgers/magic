//! `Effect::P170bAssemble`: the first player named who is in the game assembles Contraptions (Unstable), as many as
//! the count times every replacement that applies to the object assembling (Steamflogger Boss).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop};
use crate::p170b_contraptions::{SPROCKETS, assemble_events, tops};
use crate::resolve::{Cx, ResolveError, Run};

/// Assembles: for each of the top cards of the player's Contraption deck, top first, the player assigns it to one of
/// their sprockets, then each is put onto the battlefield on its sprocket. A player with no Contraption deck, or one
/// that runs out, assembles what is left: nothing at all for an empty deck.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::P170bAssemble { who, count } = effect else {
        return Err(wrong_effect("P170bAssemble"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let count = u32::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
    let times = crate::replace::p170b_assemble::p170b_times(state, cx.scope.source)?;
    let count = usize::try_from(count.saturating_mul(times)).unwrap_or(usize::MAX);
    let cards = tops(state, player, count);
    let mut questions = Questions::new(cx);
    let mut chosen = Vec::new();
    let walked: Result<(), Stop> = cards.iter().try_for_each(|card| {
        let name = state
            .object(*card)
            .map_or_else(String::new, |object| object.card().name.clone());
        let options = (1..=SPROCKETS)
            .map(|sprocket| ChoiceOption::Label(format!("Sprocket {sprocket}")))
            .collect();
        let at = questions.w9b_option(player, options, format!("Assign {name} to a sprocket."))?;
        chosen.push((*card, u8::try_from(at).unwrap_or(0).saturating_add(1)));
        Ok(())
    });
    if let Err(stop) = walked {
        return Stop::settle(Err(stop));
    }
    questions.end()?;
    Ok(Run::events(
        chosen
            .into_iter()
            .flat_map(|(card, sprocket)| assemble_events(player, card, sprocket))
            .collect(),
    ))
}
