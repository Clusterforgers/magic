//! `Effect::CopyCard`: a card in exile is copied into a token card the player may cast (CR 707.12).

use std::sync::Arc;

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::copies::CardCopy;
use crate::events::Event;
use crate::object::{ObjectKind, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each card the effect names that still lies face up in exile is copied once, and the copies are bound as the objects
/// they become: token cards in exile, owned by the player `who` names (the controller by default). The copy is not
/// cast; the step that follows (`CastFromZone`) asks whether to cast it, and a copy that is not cast ceases to exist at
/// the next state-based check (CR 704.5e). A card that left exile, a card that lies face down (its owner may look at it,
/// the others may not) and a token are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CopyCard { who, what, bind } = effect else {
        return Err(wrong_effect("CopyCard"));
    };
    let state = cx.state();
    let Some(owner) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let cards: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|card| {
            card.zone() == Zone::Exile
                && card.kind() == ObjectKind::Card
                && card.face_down().is_none()
        })
        .collect();
    let mut events = Vec::with_capacity(cards.len());
    let mut copies = Vec::with_capacity(cards.len());
    for (id, card) in (state.next_object().get()..).zip(cards) {
        let object = mtg_core::ids::ObjectId(id);
        events.push(Event::CardCopied(Box::new(CardCopy {
            object,
            of: card.reference(),
            owner,
            definition: Arc::new(card.card().copiable()),
        })));
        // A new object: its zone-change counter starts at 0.
        copies.push(Target::Object(ObjectRef {
            id: object,
            counter: 0,
        }));
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), copies)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
