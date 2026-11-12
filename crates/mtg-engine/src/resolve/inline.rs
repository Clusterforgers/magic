//! Effects that run in the middle of something else, without the stack: what a mana ability does besides adding mana,
//! and what a replacement effect does instead of the event it replaces.
//!
//! The effects run step by step over the effect tree as a resolution does (`tree::after`), each step's names bound for
//! the steps after it, but the events are not folded between steps: they are proposed together.

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::prim;
use super::{Cx, Next, ResolveError, Resolving, Run, START, Scope, tree};
use crate::abilities::Cause;
use crate::choose::Choice;
use crate::decision::Decision;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;

/// What effects run in the middle of an event came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inline {
    /// The events of the effects.
    Events(Vec<Event>),
    /// The effects are a single "may" its player declined: the event happens as it is ("you may ... instead").
    Declined,
    /// A player has to answer first; the answers so far were used in order.
    Ask(Decision),
}

/// The answers given so far to the questions of effects run inline, in the order they were asked.
pub(crate) type Answers<'a> = &'a mut dyn FnMut() -> Option<(PlayerId, Choice)>;

/// The events of `effects` run now for `player` as the controller of `source`, each as the effect proposes it.
///
/// An effect that needs an answer is an error: a mana ability asks nobody anything (CR 605.3b).
pub(crate) fn run_inline(
    state: &GameState,
    player: PlayerId,
    source: &GameObject,
    effects: &[Effect],
) -> Result<Vec<Event>, ResolveError> {
    let base = Base::Mana {
        state,
        player,
        source: source.reference(),
    };
    match drive(&base, source.id, effects, &mut || None, &[])? {
        Inline::Events(events) => Ok(events),
        Inline::Declined => Ok(Vec::new()),
        Inline::Ask(_) => Err(ResolveError::Invalid(
            "an effect run in the middle of something else asked a question".into(),
        )),
    }
}

/// The events of `effects` that a replacement effect of `source` runs in place of the event `cause` describes: the
/// effects read its player or object (`Triggering`), its amount and the source of its damage (`Dealer`).
///
/// A question is asked of the player the effect names, the controller of the replacement by default (CR 614.1, 616):
/// `answer` gives the answers so far in order, and the first question left without one comes back as `Inline::Ask`.
pub(crate) fn w5d_run_instead(
    state: &GameState,
    player: PlayerId,
    source: ObjectRef,
    cause: &Cause,
    effects: &[Effect],
    answer: Answers<'_>,
) -> Result<Inline, ResolveError> {
    let base = Base::Instead {
        state,
        player,
        source,
        cause,
    };
    drive(&base, source.id, effects, answer, &[])
}

/// `w5d_run_instead` with names bound before the first step: the targets a damage shield fixed as it began.
pub(crate) fn r370_run_instead_bound(
    state: &GameState,
    (player, source): (PlayerId, ObjectRef),
    cause: &Cause,
    effects: &[Effect],
    bound: &[(String, Vec<crate::choose::Target>)],
    answer: Answers<'_>,
) -> Result<Inline, ResolveError> {
    let base = Base::Instead {
        state,
        player,
        source,
        cause,
    };
    drive(&base, source.id, effects, answer, bound)
}

/// Runs the effect tree from its first effect to its end: a question takes the next answer and runs the step again, and
/// a finished step folds its names into the run (`fold::step_done`). An effect that needs its own events folded first
/// (`Next::Again`) or casts a spell is an error.
///
/// A step reads the state the steps before it leave ("draw a card, then discard a card" offers the card drawn): their
/// events are folded onto a copy of the state, which the real fold repeats in the same order once the batch is applied.
fn drive(
    base: &Base<'_>,
    object: ObjectId,
    effects: &[Effect],
    answer: Answers<'_>,
    bound: &[(String, Vec<crate::choose::Target>)],
) -> Result<Inline, ResolveError> {
    let mut resolving = Resolving::begin(object);
    resolving.bindings.extend(bound.iter().cloned());
    let mut events = Vec::new();
    let mut steps = 0_usize;
    let mut moved = std::collections::BTreeSet::new();
    let mut after: Option<GameState> = None;
    while let Some(effect) = tree::effect_at(effects, &[], &resolving.position) {
        steps += 1;
        if steps > 10_000 {
            return Err(ResolveError::Invalid(
                "effects run in the middle of an event went on without end".into(),
            ));
        }
        match step(base, after.as_ref(), &resolving, effect)? {
            Run::Ask(decision) => match answer() {
                Some(given) => resolving.journal.push(given),
                None => return Ok(Inline::Ask(decision)),
            },
            Run::Cast(_) => {
                return Err(ResolveError::Invalid(
                    "an effect run in the middle of something else casts no spell".into(),
                ));
            }
            Run::Done {
                events: made,
                bound,
                snapshots,
                next,
            } => {
                if next == Next::Again {
                    return Err(ResolveError::Invalid(
                        "an effect run in the middle of something else cannot wait for its own events"
                            .into(),
                    ));
                }
                if resolving.position == START
                    && next == Next::Sibling
                    && matches!(effects, [Effect::May { .. }])
                {
                    return Ok(Inline::Declined);
                }
                // An object an earlier step moved is a new object (CR 400.7): a later step's move of it finds nothing.
                // A step that read the folded state found it as the new object it is, which it may move.
                let made: Vec<Event> = if after.is_some() {
                    made
                } else {
                    made.into_iter()
                        .filter_map(|event| still_there(event, &moved))
                        .collect()
                };
                moved.extend(made.iter().filter_map(moved_object));
                let position = tree::after(effects, &[], &resolving.position, next);
                if !made.is_empty() && tree::effect_at(effects, &[], &position).is_some() {
                    after = folded(after.as_ref().unwrap_or_else(|| base.state()), &made);
                }
                events.extend(made);
                crate::fold::step_done(&mut resolving, &position, &bound, &snapshots)
                    .map_err(ResolveError::Invalid)?;
            }
        }
    }
    Ok(Inline::Events(events))
}

/// Where effects run inline read their scope from.
enum Base<'s> {
    /// A mana ability's effects besides its mana, announced by `player` for `source`.
    Mana {
        state: &'s GameState,
        player: PlayerId,
        source: ObjectRef,
    },
    /// A replacement effect of `source`, controlled by `player`, in place of the event `cause` describes.
    Instead {
        state: &'s GameState,
        player: PlayerId,
        source: ObjectRef,
        cause: &'s Cause,
    },
}

/// `state` with `events` folded onto a copy of it, or `None` when one of them does not fold there: the steps after then
/// read the state the run began in, as they did before any step was folded.
fn folded(state: &GameState, events: &[Event]) -> Option<GameState> {
    let mut copy = state.clone();
    events
        .iter()
        .all(|event| crate::fold::apply_event(&mut copy, event).is_ok())
        .then_some(copy)
}

impl Base<'_> {
    /// The state the run began in.
    const fn state(&self) -> &GameState {
        match self {
            Base::Mana { state, .. } | Base::Instead { state, .. } => state,
        }
    }

    /// The scope of a step: the base read in `after` (the state the steps before it left) when given, with the names and
    /// objects the steps before it bound and moved away.
    fn scope<'a>(&'a self, after: Option<&'a GameState>, resolving: &'a Resolving) -> Scope<'a> {
        let mut scope = match self {
            Base::Mana {
                state,
                player,
                source,
            } => Scope::announcing(after.unwrap_or(state), *player, *source),
            Base::Instead {
                state,
                player,
                source,
                cause,
            } => Scope::triggered(after.unwrap_or(state), *player, *source, cause),
        };
        scope.bindings = &resolving.bindings;
        scope.inline = Some(resolving);
        scope.departed = &resolving.snapshots;
        scope.snapshots.extend(resolving.snapshots.iter());
        scope
    }
}

/// One step of the run at the position `resolving` stands at.
fn step(
    base: &Base<'_>,
    after: Option<&GameState>,
    resolving: &Resolving,
    effect: &Effect,
) -> Result<Run, ResolveError> {
    let cx = Cx {
        scope: base.scope(after, resolving),
        resolving,
    };
    prim::run(&cx, effect)
}

/// The object a zone change moves.
const fn moved_object(event: &Event) -> Option<ObjectId> {
    match event {
        Event::ZoneChanged { object, .. } => Some(*object),
        _ => None,
    }
}

/// The event with the objects in `moved` taken out of it: a move of one of them is no event, and a library bottomed
/// keeps the other cards.
fn still_there(event: Event, moved: &std::collections::BTreeSet<ObjectId>) -> Option<Event> {
    match event {
        Event::ZoneChanged { object, .. } if moved.contains(&object) => None,
        Event::LibraryBottomed {
            player,
            cards,
            random,
        } => {
            let cards: Vec<ObjectId> = cards
                .into_iter()
                .filter(|card| !moved.contains(card))
                .collect();
            (!cards.is_empty()).then_some(Event::LibraryBottomed {
                player,
                cards,
                random,
            })
        }
        other => Some(other),
    }
}
