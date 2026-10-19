//! Resolving a spell or ability: the effect list runs as a journal of steps, each a pure function of state.
//!
//! [`step`] reads the state and the journal and never changes either. It returns the events of one effect
//! (ending with `StepDone`, which moves the position on and clears the journal) or the decision the effect
//! needs; the answer is logged as `ChoiceMade`, joins the journal and the same step runs again to use it, so a
//! step with k decisions runs k + 1 times. State stays the fold of the log, and an engine rebuilt from the log
//! stands at the same step (`docs/cards-dsl.md`, Execution).

mod prim;
mod tree;
mod value;

use mtg_cards::ability::Ability;
use mtg_core::ids::{ObjectId, PlayerId};

pub use value::{Bindings, ResolveError, Scope};

use crate::abilities::AbilityObject;
use crate::choose::{Choice, Target};
use crate::decision::Decision;
use crate::error::EngineError;
use crate::events::{Event, ResolutionOutcome};
use crate::flow;
use crate::object::{GameObject, ObjectKind};
use crate::replace::{self, Processed};
use crate::snapshot::Snapshot;
use crate::state::GameState;

/// The position of the first effect: a path of indices into the effect tree.
const START: [usize; 1] = [0];

/// A spell or ability that is resolving: how far it is, what it bound and the answers waiting to be used.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resolving {
    pub(crate) object: ObjectId,
    pub(crate) position: Vec<usize>,
    pub(crate) bindings: Bindings,
    pub(crate) snapshots: Vec<Snapshot>,
    pub(crate) journal: Vec<(PlayerId, Choice)>,
}

impl Resolving {
    pub(crate) fn begin(object: ObjectId) -> Self {
        Resolving {
            object,
            position: START.to_vec(),
            bindings: Bindings::new(),
            snapshots: Vec::new(),
            journal: Vec::new(),
        }
    }

    /// The spell or ability.
    pub const fn object(&self) -> ObjectId {
        self.object
    }

    /// The path of the effect that runs next: indices into the effect tree.
    pub fn position(&self) -> &[usize] {
        &self.position
    }

    /// What the finished steps bound to names.
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Objects the finished steps moved away, as they were.
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// The answers given to the step that is running and not yet used by it.
    pub fn journal(&self) -> &[(PlayerId, Choice)] {
        &self.journal
    }
}

/// What one step of a resolution came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// The events of the step, ending with `StepDone`, or the end of the resolution.
    Done(Vec<Event>),
    /// The step needs an answer first.
    Ask(Decision),
}

/// Where a step goes after its effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// On to the next effect.
    Sibling,
    /// Into the effects of this branch of the effect (`then`, `paid`, a mode), or past it when it is empty.
    Enter(usize),
}

/// What running one effect came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// The effect is done: these events are proposed, these names are bound and these objects were moved away.
    Done {
        /// The events the effect proposes; the replacement step sees them before the fold does.
        events: Vec<Event>,
        /// Names the effect binds.
        bound: Vec<(String, Vec<Target>)>,
        /// Objects the effect moved away, as they were.
        snapshots: Vec<Snapshot>,
        /// Where the resolution goes next.
        next: Next,
    },
    /// The effect needs an answer first.
    Ask(Decision),
}

impl Run {
    /// An effect that is done and binds nothing.
    pub const fn events(events: Vec<Event>) -> Run {
        Run::Done {
            events,
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Sibling,
        }
    }
}

/// What an effect may read while it runs.
#[derive(Debug, Clone)]
pub struct Cx<'a> {
    pub(crate) scope: Scope<'a>,
    pub(crate) resolving: &'a Resolving,
}

impl Cx<'_> {
    /// The game.
    pub fn state(&self) -> &GameState {
        self.scope.state
    }

    /// The player who controls the spell or ability.
    pub const fn controller(&self) -> PlayerId {
        self.scope.controller
    }

    /// The answers given to this step so far.
    pub fn journal(&self) -> &[(PlayerId, Choice)] {
        &self.resolving.journal
    }
}

/// The `Spell` ability of a card: what it does as an instant or sorcery.
pub(crate) fn spell_ability(object: &GameObject) -> Option<&Ability> {
    object
        .card()
        .rules
        .iter()
        .map(|rules| &**rules)
        .find(|rules| matches!(rules, Ability::Spell { .. }))
}

/// The rules a stack object runs: those of the ability it is, or the `Spell` ability of its card.
fn rules_of(object: &GameObject) -> Option<&Ability> {
    object
        .ability()
        .map(AbilityObject::rules)
        .or_else(|| spell_ability(object))
}

/// Whether resolving the object runs effects; a permanent spell, or a card whose text is left to the table, does not.
pub(crate) fn runs_effects(object: &GameObject) -> bool {
    rules_of(object).is_some()
}

fn scope_of<'a>(
    state: &'a GameState,
    object: &'a GameObject,
    resolving: &'a Resolving,
    rules: &'a Ability,
) -> Scope<'a> {
    let ability = object.ability();
    let mut snapshots: Vec<&Snapshot> = resolving.snapshots.iter().collect();
    if let Some(ability) = ability {
        snapshots.push(ability.snapshot());
        snapshots.extend(ability.cause().iter().flat_map(|c| c.snapshots.iter()));
    }
    Scope {
        state,
        controller: object.controller(),
        source: ability.map_or_else(|| object.reference(), AbilityObject::source),
        choices: object.choices(),
        specs: rules.targets(),
        bindings: &resolving.bindings,
        snapshots,
        cause: ability.and_then(|a| a.cause()),
    }
}

/// The events that end a resolution that did what it says.
fn finish(object: &GameObject) -> Vec<Event> {
    let mut events = vec![Event::ResolutionEnded {
        object: object.id(),
        outcome: ResolutionOutcome::Resolved,
    }];
    if object.kind() != ObjectKind::Ability {
        events.push(Event::SpellResolved {
            object: object.id(),
        });
    }
    events
}

/// Runs the next step of the resolution in progress.
pub fn step(state: &GameState) -> Result<Progress, EngineError> {
    let resolving = state
        .resolving()
        .ok_or_else(|| EngineError::Resolution("nothing is resolving".into()))?;
    let object = state.object(resolving.object).ok_or_else(|| {
        EngineError::Resolution(format!("{:?} is not in the game", resolving.object))
    })?;
    if !state.is_alive(object.controller()) {
        return Ok(Progress::Done(vec![Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Countered,
        }]));
    }
    let Some(rules) = rules_of(object) else {
        return Ok(Progress::Done(finish(object)));
    };
    let scope = scope_of(state, object, resolving, rules);
    // CR 608.2b: a spell or ability whose targets have all become illegal does nothing at all.
    if resolving.position == START && scope.all_targets_illegal() {
        return Ok(Progress::Done(vec![Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Fizzled,
        }]));
    }
    let effects = rules.effects();
    let Some(effect) = tree::effect_at(effects, &resolving.position) else {
        return Ok(Progress::Done(finish(object)));
    };
    let cx = Cx { scope, resolving };
    match prim::run(&cx, effect)? {
        Run::Ask(decision) => Ok(Progress::Ask(decision)),
        Run::Done {
            events,
            bound,
            snapshots,
            next,
        } => {
            let mut events = match replace::process(state, events) {
                Processed::Events(events) => events,
                Processed::Ask(decision) => return Ok(Progress::Ask(decision)),
            };
            events.push(Event::StepDone {
                position: tree::after(effects, &resolving.position, next),
                bound,
                snapshots,
            });
            Ok(Progress::Done(events))
        }
    }
}

/// The next move of the engine while something resolves.
pub(crate) fn next(state: &GameState) -> flow::Progress {
    match step(state) {
        Ok(Progress::Done(events)) => flow::Progress::Events(events),
        Ok(Progress::Ask(decision)) => flow::Progress::Decide(decision),
        Err(error) => flow::Progress::Fail(error),
    }
}
