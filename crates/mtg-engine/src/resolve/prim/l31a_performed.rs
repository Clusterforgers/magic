//! `Effect::Performed`: the log of a keyword action the steps before it performed (investigate, discover, manifest
//! dread, a gift), which "whenever you investigate" and the like trigger on; nothing else changes.

use mtg_cards::did::Action;
use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// One event for each player the effect names who is still in the game: `Investigated` once for each time,
/// `Discovered` with the value and the card, `DreadManifested` with the card put into the graveyard, `GiftGiven` to
/// each player the gift went to who is still in the game. A card that is gone is no card.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Performed {
        action,
        who,
        times,
        card,
        value,
        to,
    } = effect
    else {
        return Err(wrong_effect("Performed"));
    };
    let state = cx.state();
    let card = match card {
        Some(card) => cx
            .scope
            .objects(card)
            .ok()
            .and_then(|found| found.first().map(|found| found.id))
            .filter(|id| state.object(*id).is_some()),
        None => None,
    };
    let value = match value {
        Some(value) => u32::try_from(cx.scope.amount(value)?.max(0)).unwrap_or(u32::MAX),
        None => 0,
    };
    let times = usize::try_from(cx.scope.amount(times)?.max(0)).unwrap_or(0);
    let given = match to {
        Some(to) => cx.scope.players(to)?,
        None => Vec::new(),
    };
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        match action {
            Action::Investigated => {
                events.extend((0..times).map(|_| Event::Investigated { player }));
            }
            Action::Discovered => events.push(Event::Discovered {
                player,
                value,
                card,
            }),
            Action::ManifestedDread => events.push(Event::DreadManifested { player, card }),
            Action::GaveGift => events.extend(
                given
                    .iter()
                    .filter(|to| state.is_alive(**to))
                    .map(|to| Event::GiftGiven { player, to: *to }),
            ),
            _ => return Err(ResolveError::Unsupported("Effect.Performed")),
        }
    }
    Ok(Run::events(events))
}
