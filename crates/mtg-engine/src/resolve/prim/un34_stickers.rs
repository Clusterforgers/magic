//! `Effect::PutSticker` and `Effect::ClaimPrize`: the first player named who is in the game puts a sticker on a
//! permanent they own (CR 123), or claims the prize of the Attraction that is the source.

use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, Selector};
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, LET, Next, ResolveError, Run};
use crate::un34_stickers::{StickerAt, VOWELS, affordable, put_events, sticker_text};
use crate::zones::Zone;
use mtg_cards::ability::StickerKind;

/// Puts a sticker on or claims the prize.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let (who, put) = match effect {
        Effect::PutSticker {
            who,
            on,
            kind,
            up_to,
        } => (who, Some((on, *kind, *up_to))),
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
    match put {
        Some((on, kind, up_to)) => Stop::settle(put_stickers(cx, player, on, kind, up_to)),
        None => Ok(Run::events(vec![Event::PrizeClaimed {
            player,
            attraction: cx.scope.source.id,
        }])),
    }
}

/// The player puts stickers on, one at a time: one that must be put when `up_to` is 0, else up to that many, each
/// optional. Each goes on an object of `on` they own (a permanent, or a card in the zone the selector names) and is an
/// unused sticker of the kind they can pay for; with nothing to choose nothing more happens. The unique vowels of the
/// last name sticker put on are kept for `Amount::StickerVowels`.
fn put_stickers(
    cx: &Cx<'_>,
    player: PlayerId,
    on: &Selector,
    kind: Option<StickerKind>,
    up_to: u32,
) -> Result<Run, Stop> {
    let state = cx.state();
    let objects: Vec<ObjectRef> = cx
        .scope
        .live_objects(&Ref::Each(on.clone()))?
        .into_iter()
        .filter(|object| object.owner() == player && object.zone() != Zone::Stack)
        .map(GameObject::reference)
        .collect();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    let mut taken: Vec<StickerAt> = Vec::new();
    let mut spent = 0u32;
    let mut vowels = 0u32;
    for round in 0..up_to.max(1) {
        let stickers: Vec<_> = affordable(state, player)
            .into_iter()
            .filter(|(at, sticker)| {
                !taken.contains(at)
                    && kind.is_none_or(|kind| sticker.kind() == kind)
                    && sticker.tickets() + spent <= tickets(state, player)
            })
            .collect();
        if objects.is_empty() || stickers.is_empty() {
            break;
        }
        if up_to > 0 {
            let text = if round == 0 {
                "Put a sticker on?".to_owned()
            } else {
                "Put another sticker on?".to_owned()
            };
            if !questions.yes_no(player, text)? {
                break;
            }
        }
        let chosen = if let [only] = objects.as_slice() {
            *only
        } else {
            let from = objects.iter().map(|object| object.id).collect();
            let text = "Choose what you own to put a sticker on.".to_owned();
            let picked = questions.cards(player, from, (1, 1), text)?;
            picked
                .first()
                .and_then(|id| objects.iter().find(|object| object.id == *id))
                .copied()
                .ok_or_else(|| ResolveError::Invalid("nothing was chosen".into()))?
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
        let (at, sticker) = stickers
            .get(picked)
            .ok_or_else(|| ResolveError::Invalid("no sticker was chosen".into()))?;
        if sticker.kind() == StickerKind::Name {
            vowels = sticker.unique_vowels();
        }
        spent += sticker.tickets();
        taken.push(*at);
        events.extend(put_events(state, *at, chosen));
    }
    questions.end()?;
    Ok(Run::Done {
        events,
        bound: vec![(format!("{LET}{vowels}:{VOWELS}"), Vec::new())],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The tickets the player holds.
fn tickets(state: &crate::state::GameState, player: PlayerId) -> u32 {
    state
        .player(player)
        .map_or(0, |seat| seat.counters_of(&crate::un34_stickers::ticket()))
}
