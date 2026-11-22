//! `Effect::R116Assimilate`: "Put it onto the battlefield under your control with a +1/+1 counter. It's a Borg artifact
//! creature and loses all other creature types." for a card, and "Gain control of it and put a +1/+1 counter on it. It's a
//! Borg artifact creature and loses all other creature types." for a permanent.
//!
//! A card goes onto the battlefield under the controller's control with the counter as it enters (`Effect::Move`); a
//! permanent changes control for good (`Modification::SetController`) and then gets the counter. Either way an effect that
//! lasts for good then makes it an artifact creature whose only creature type is Borg (CR 205.1a, layer 4).

use mtg_cards::effect::{Destination, Effect};
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::{CardType, Subtype};

use super::{continuous, move_to, moved_as, put_counters, wrong_effect};
use crate::choose::Target;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// The names this step binds its cards and its permanents to: the step is told apart by its position.
fn r116_markers(cx: &Cx<'_>) -> (String, String) {
    let at = format!("{:?}", cx.resolving.position);
    (
        format!("assimilated cards {at}"),
        format!("assimilated permanents {at}"),
    )
}

/// "It's a Borg artifact creature and loses all other creature types."
fn r116_borg(affected: Ref) -> Effect {
    Effect::Continuous {
        affected,
        mods: vec![
            Modification::SetTypes(vec![CardType::Artifact, CardType::Creature]),
            Modification::SetCreatureTypes(vec![Subtype("Borg".to_owned())]),
        ],
        duration: Duration::Forever,
    }
}

/// Declared ahead of the engine: the two passes below do not move or take the object yet in a game
/// (`r116_assimilate.rs` of the testkit, ignored), so the effect refuses until they do.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R116Assimilate { .. } = effect else {
        return Err(wrong_effect("R116Assimilate"));
    };
    let _ = cx;
    Err(ResolveError::Unsupported("Effect.R116Assimilate"))
}

/// The first pass puts the cards onto the battlefield (or takes control of the permanents) and binds them; the second
/// pass makes them Borg artifact creatures (and puts the counter on the permanents).
#[allow(dead_code)]
fn r116_two_passes(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R116Assimilate { what } = effect else {
        return Err(wrong_effect("R116Assimilate"));
    };
    let (cards, permanents) = r116_markers(cx);
    if cx.resolving.bindings.contains_key(&cards) {
        let mut run = continuous::run(cx, &r116_borg(Ref::Bound(cards)))?;
        if cx.resolving.bindings.contains_key(&permanents)
            && let (Run::Done { events, .. }, Run::Done { events: more, .. }) = (
                &mut run,
                put_counters::run(
                    cx,
                    &Effect::PutCounters {
                        on: Ref::Bound(permanents),
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::one(),
                    },
                )?,
            )
        {
            events.extend(more);
        }
        return Ok(run);
    }
    let state = cx.state();
    let mut on_battlefield = Vec::new();
    for named in cx.scope.objects(what)? {
        if state.object(named.id).is_some_and(|object| {
            object.counter() == named.counter && object.zone() == Zone::Battlefield
        }) {
            on_battlefield.push(Target::Object(named));
        }
    }
    let moved = Effect::Move {
        what: what.clone(),
        to: Destination::Battlefield {
            tapped: false,
            under: Some(Ref::You),
            transformed: false,
            counters: vec![(CounterKind::PlusOnePlusOne, 1)],
        },
        bind: Some(cards.clone()),
    };
    let mut run = if on_battlefield.is_empty() {
        moved_as::bind(cx, &moved, move_to::run(cx, &moved)?)
    } else {
        let mut taken = continuous::run(
            cx,
            &Effect::Continuous {
                affected: what.clone(),
                mods: vec![Modification::SetController(Ref::You)],
                duration: Duration::Forever,
            },
        )?;
        if let Run::Done { bound, .. } = &mut taken {
            bound.push((cards.clone(), on_battlefield.clone()));
            bound.push((permanents, on_battlefield));
        }
        taken
    };
    match &mut run {
        Run::Done { bound, next, .. } => {
            if !bound.iter().any(|(name, _)| *name == cards) {
                bound.push((cards, Vec::new()));
            }
            *next = Next::Again;
        }
        Run::Ask(_) | Run::Cast(_) => {}
    }
    Ok(run)
}
