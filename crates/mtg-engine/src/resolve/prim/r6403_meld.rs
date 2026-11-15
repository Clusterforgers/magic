//! `Effect::Meld`: "if you both own and control ~ and a creature named N, exile them, then meld them into M" (CR 701.42).
//!
//! The first run checks that the controller owns and controls the source and a partner the filter names (one of their
//! choice when several do), exiles both and binds them as the objects they become in exile; the rerun (`Next::Again`)
//! melds them when both are still those cards in exile: the source returns to the battlefield as the melded permanent and
//! the partner joins it (`Event::Melded`). A token, a copy or a card that did not reach exile cannot be melded, and what
//! was exiled stays there (CR 701.42b, 701.42c).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;

use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::merged::Melded;
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Runs the effect: the exile, then the meld.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Meld { with, into } = effect else {
        return Err(wrong_effect("Meld"));
    };
    Stop::settle(meld(cx, with, into))
}

/// The name the exiled pair of this step is bound to: the step is told apart by its position.
fn marker(cx: &Cx<'_>) -> String {
    format!("melding {:?}", cx.resolving.position)
}

fn meld(cx: &Cx<'_>, with: &Selector, into: &str) -> Result<Run, Stop> {
    let marker = marker(cx);
    if let Some(exiled) = cx.resolving.bindings.get(&marker) {
        return Ok(Run::events(melded(cx, exiled, with, into)));
    }
    let state = cx.state();
    let player = cx.controller();
    let source = cx.scope.source;
    let held = |object: &GameObject| {
        object.zone() == Zone::Battlefield
            && object.owner() == player
            && object.controller() == player
    };
    let Some(first) = state
        .object(source.id)
        .filter(|object| object.counter() == source.counter && held(object))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let mut filter = with.0.clone();
    filter.extend([
        Pred::OwnedBy(Ref::You),
        Pred::ControlledBy(Ref::You),
        Pred::Other,
    ]);
    let partners: Vec<ObjectRef> = cx
        .scope
        .live_objects(&Ref::Each(Selector(filter)))?
        .into_iter()
        .filter(|object| held(object))
        .map(GameObject::reference)
        .collect();
    let mut questions = Questions::new(cx);
    let partner = match partners.as_slice() {
        [] => return Ok(Run::events(Vec::new())),
        [only] => *only,
        _ => {
            let from = partners.iter().map(|partner| partner.id).collect();
            let text = format!("Choose the card to meld with {}.", first.card().name);
            let taken = questions.cards(player, from, (1, 1), text)?;
            taken
                .first()
                .and_then(|id| partners.iter().find(|partner| partner.id == *id))
                .copied()
                .ok_or_else(|| ResolveError::Invalid("no card to meld was chosen".into()))?
        }
    };
    questions.end()?;
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    let mut exiled = Vec::new();
    for reference in [first.reference(), partner] {
        snapshots.extend(snapshot(state, reference.id));
        events.push(Event::ZoneChanged {
            object: reference.id,
            from: Zone::Battlefield,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        });
        exiled.push(Target::Object(ObjectRef {
            id: reference.id,
            counter: reference.counter + 1,
        }));
    }
    Ok(Run::Done {
        events,
        bound: vec![(marker, exiled)],
        snapshots,
        next: Next::Again,
    })
}

/// The events of the meld itself: none unless both cards are the pair the step exiled, still in exile, and the game
/// knows the melded card.
fn melded(cx: &Cx<'_>, exiled: &[Target], with: &Selector, into: &str) -> Vec<Event> {
    let state = cx.state();
    let Some(definition) = state.conjurable.get(into) else {
        return Vec::new();
    };
    let card = |target: Option<&Target>| match target {
        Some(Target::Object(reference)) => state.object(reference.id).filter(|object| {
            object.counter() == reference.counter
                && object.zone() == Zone::Exile
                && object.kind() == ObjectKind::Card
                && !object.is_card_copy()
        }),
        _ => None,
    };
    let (Some(first), Some(partner)) = (card(exiled.first()), card(exiled.get(1))) else {
        return Vec::new();
    };
    // The partner is the card of the pair the ability names (CR 701.42b).
    let named = with.0.iter().all(|pred| match pred {
        Pred::Named(name) => partner.card().name == *name,
        _ => true,
    });
    if !named || first.owner() != partner.owner() {
        return Vec::new();
    }
    let mut events = vec![
        Event::ZoneChanged {
            object: first.id(),
            from: Zone::Exile,
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
        Event::Melded(Box::new(Melded {
            object: first.id(),
            with: partner.id(),
            name: definition.name.clone(),
            definition: Arc::clone(definition),
        })),
    ];
    // A melded planeswalker enters with the loyalty of the melded card (CR 306.5b).
    if definition.type_set().contains(CardType::Planeswalker)
        && let Some(loyalty) = definition.loyalty.filter(|loyalty| *loyalty > 0)
        && let Ok(delta) = i32::try_from(loyalty)
    {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(ObjectRef {
                id: first.id(),
                counter: first.counter() + 1,
            }),
            counter: CounterKind::Loyalty,
            delta,
        }));
    }
    events
}
