//! `Effect::AuraSwap`: "you may exchange this permanent with an Aura card in your hand" (CR 702.65).
//!
//! The controller may pick an Aura card from their hand that could enchant what the source is attached to as it enters
//! (CR 303.4f); the source goes to its owner's hand and the card enters the battlefield attached to that object or player
//! (CR 702.65b). With no such card, a source that is not on the battlefield attached to something, or one the controller
//! does not own, nothing moves: an exchange happens whole or not at all (CR 701.12a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::attach;
use crate::choose::{Questions, Stop};
use crate::events::{Event, ZoneCause};
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// What the source is attached to.
#[derive(Clone, Copy)]
enum Host {
    Object(mtg_core::ids::ObjectId),
    Player(mtg_core::ids::PlayerId),
}

/// Runs the exchange.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    if !matches!(effect, Effect::AuraSwap) {
        return Err(wrong_effect("AuraSwap"));
    }
    Stop::settle(swap(cx))
}

fn swap(cx: &Cx<'_>) -> Result<Run, Stop> {
    let state = cx.state();
    let player = cx.controller();
    let source = cx.scope.source;
    let Some(aura) = state.object(source.id).filter(|object| {
        object.counter() == source.counter
            && object.zone() == Zone::Battlefield
            && object.owner() == player
            && object.controller() == player
    }) else {
        return Ok(Run::events(Vec::new()));
    };
    let Some(held) = aura.permanent() else {
        return Ok(Run::events(Vec::new()));
    };
    let host = match (held.attached_to(), held.attached_player()) {
        (Some(object), _) => Host::Object(object.id),
        (None, Some(player)) => Host::Player(player),
        (None, None) => return Ok(Run::events(Vec::new())),
    };
    let fits = |card: &GameObject| match host {
        Host::Object(id) => {
            attach::hosts_when_entering(state, card, player).is_some_and(|hosts| hosts.contains(&id))
        }
        Host::Player(enchanted) => {
            attach::player_hosts_when_entering(state, card, player).contains(&enchanted)
        }
    };
    let cards: Vec<mtg_core::ids::ObjectId> = state
        .zones
        .get(Zone::Hand(player))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|card| card.card().subtypes.iter().any(|subtype| subtype.0 == "Aura"))
        .filter(|card| fits(card))
        .map(GameObject::id)
        .collect();
    if cards.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let mut questions = Questions::new(cx);
    let text = format!("You may exchange {} with an Aura card in your hand.", aura.card().name);
    let taken = questions.cards(player, cards, (0, 1), text)?;
    questions.end()?;
    let Some(card) = taken.first().copied() else {
        return Ok(Run::events(Vec::new()));
    };
    let mut events = vec![
        Event::ZoneChanged {
            object: aura.id(),
            from: Zone::Battlefield,
            to: Zone::Hand(aura.owner()),
            cause: ZoneCause::Moved,
        },
        Event::ZoneChanged {
            object: card,
            from: Zone::Hand(player),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
    ];
    events.push(match host {
        Host::Object(to) => Event::Attached { object: card, to },
        Host::Player(enchanted) => Event::AttachedToPlayer {
            object: card,
            player: enchanted,
        },
    });
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: snapshot(state, aura.id()).into_iter().collect(),
        next: crate::resolve::Next::Sibling,
    })
}
