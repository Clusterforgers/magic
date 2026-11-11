//! `Effect::PutSticker` and `Effect::ClaimPrize`: the first player named who is in the game puts a sticker on a
//! permanent they own (CR 123), or claims the prize of the Attraction that is the source.

use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, Selector};
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, ResolveError, Run};
use crate::un34_stickers::{affordable, put_events, sticker_text};
use crate::zones::Zone;

/// Puts a sticker on or claims the prize.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let (who, on) = match effect {
        Effect::PutSticker { who, on } => (who, Some(on)),
        Effect::ClaimPrize { who } => (who, None),
        _ => return Err(wrong_effect("PutSticker")),
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
    match on {
        Some(on) => Stop::settle(put(cx, player, on)),
        None => Ok(Run::events(vec![Event::PrizeClaimed {
            player,
            attraction: cx.scope.source.id,
        }])),
    }
}

/// The player chooses a permanent of `on` they own and an unused sticker they can pay for, pays its tickets and puts
/// it on; with neither to choose nothing happens.
fn put(cx: &Cx<'_>, player: PlayerId, on: &Selector) -> Result<Run, Stop> {
    let state = cx.state();
    let permanents: Vec<ObjectRef> = cx
        .scope
        .live_objects(&Ref::Each(on.clone()))?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield && object.owner() == player)
        .map(GameObject::reference)
        .collect();
    let stickers = affordable(state, player);
    if permanents.is_empty() || stickers.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let mut questions = Questions::new(cx);
    let chosen = if let [only] = permanents.as_slice() {
        *only
    } else {
        let from = permanents.iter().map(|object| object.id).collect();
        let text = "Choose a permanent you own to put a sticker on.".to_owned();
        let taken = questions.cards(player, from, (1, 1), text)?;
        taken
            .first()
            .and_then(|id| permanents.iter().find(|object| object.id == *id))
            .copied()
            .ok_or_else(|| ResolveError::Invalid("no permanent was chosen".into()))?
    };
    let options = stickers
        .iter()
        .map(|(at, sticker)| {
            let sheet = state
                .sticker_sheets(player)
                .into_iter()
                .find(|(index, _)| *index == at.sheet)
                .map_or_else(String::new, |(_, card)| card.name.clone());
            ChoiceOption::Label(format!("{sheet}: {}", sticker_text(sticker)))
        })
        .collect();
    let picked = questions.w9b_option(player, options, "Choose a sticker.".to_owned())?;
    questions.end()?;
    let (at, _) = stickers
        .get(picked)
        .ok_or_else(|| ResolveError::Invalid("no sticker was chosen".into()))?;
    Ok(Run::events(put_events(state, *at, chosen)))
}
