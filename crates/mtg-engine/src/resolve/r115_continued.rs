//! The steps of a replacement effect that reached a spell cast in the middle of a resolution (CR 608.2g, 614.1): "instead
//! exile cards until ..., you may cast it without paying its mana cost, then put the rest on the bottom". The steps before
//! the cast run inline as the replaced event; the cast and the steps after it go on as part of the resolving spell or
//! ability (`Event::R115Continued`), once the step of the replaced event is done and before its next effect. They read
//! the names the steps before them bound and their own source; the event they replaced is gone.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::{Cx, Progress, ResolveError, Resolving, Run, Scope, tree};
use crate::choose::Target;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::snapshot::Snapshot;
use crate::state::GameState;

/// The steps of a replacement effect that go on as part of the resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct R115Continuation {
    /// The object with the replacement effect.
    source: ObjectRef,
    /// Who controls the replacement effect.
    controller: PlayerId,
    /// The effects of the replacement.
    effects: Vec<Effect>,
    /// Where the steps stand and what they bound.
    run: Resolving,
    /// The step of the resolution that hung them on it is done, so the next finished step is theirs.
    armed: bool,
}

/// Hangs the steps an `Event::R115Continued` names on the resolution: they run once the step whose batch holds the event
/// is done.
pub(crate) fn r115_hang(
    resolving: &mut Resolving,
    (source, controller): (ObjectRef, PlayerId),
    effects: &[Effect],
    position: &[usize],
    bound: &[(String, Vec<Target>)],
    snapshots: &[Snapshot],
) -> Result<(), String> {
    let mut run = Resolving::begin(resolving.object);
    crate::fold::step_done(&mut run, position, bound, snapshots)?;
    resolving.r115_continued.push(R115Continuation {
        source,
        controller,
        effects: effects.to_vec(),
        run,
        armed: false,
    });
    Ok(())
}

/// Folds a finished step into the first steps hung on the resolution, when there are such that run now; whether they
/// took it. Steps that reach their end are done. Steps hung in the batch of this step run from the next one.
pub(crate) fn r115_step_done(
    resolving: &mut Resolving,
    position: &[usize],
    bound: &[(String, Vec<Target>)],
    snapshots: &[Snapshot],
) -> Result<bool, String> {
    let taken = match resolving.r115_continued.iter().position(|hung| hung.armed) {
        Some(at) => {
            let hung = &mut resolving.r115_continued[at];
            crate::fold::step_done(&mut hung.run, position, bound, snapshots)?;
            if tree::effect_at(&hung.effects, &[], position).is_none() {
                resolving.r115_continued.remove(at);
            }
            resolving.journal.clear();
            true
        }
        None => false,
    };
    for hung in &mut resolving.r115_continued {
        hung.armed = true;
    }
    Ok(taken)
}

impl Resolving {
    /// Where the step that runs next stands: in the steps of a replacement hung on the resolution while there are such.
    pub(crate) fn r115_step_position(&self) -> &[usize] {
        self.r115_continued
            .iter()
            .find(|hung| hung.armed)
            .map_or(&self.position, |hung| &hung.run.position)
    }

    /// Whether steps of a replacement run before the resolution's next effect.
    pub(crate) fn r115_continuing(&self) -> bool {
        self.r115_continued.iter().any(|hung| hung.armed)
    }
}

/// The next step of the steps hung on the resolution, when there are such: its question, its cast, or its events ending
/// with the `StepDone` that moves them on.
pub(super) fn r115_step(
    state: &GameState,
    resolving: &Resolving,
    run: &impl Fn(&Cx<'_>, &Effect) -> Result<Run, ResolveError>,
) -> Option<Result<Progress, ResolveError>> {
    let hung = resolving.r115_continued.iter().find(|hung| hung.armed)?;
    let at = Resolving {
        journal: resolving.journal.clone(),
        ..hung.run.clone()
    };
    let Some(effect) = tree::effect_at(&hung.effects, &[], &at.position) else {
        return Some(Ok(Progress::Done(vec![Event::StepDone {
            position: at.position.clone(),
            bound: Vec::new(),
            snapshots: Vec::new(),
        }])));
    };
    let mut scope = Scope::announcing(state, hung.controller, hung.source);
    scope.bindings = &at.bindings;
    scope.departed = &at.snapshots;
    scope.snapshots.extend(at.snapshots.iter());
    let cx = Cx {
        scope,
        resolving: &at,
    };
    Some(run(&cx, effect).map(|ran| match ran {
        Run::Ask(decision) => Progress::Ask(decision),
        Run::Cast(cast) => Progress::Cast(cast),
        Run::Done {
            mut events,
            bound,
            snapshots,
            next,
        } => {
            let position = tree::after(&hung.effects, &[], &at.position, next);
            events.push(Event::StepDone {
                position,
                bound,
                snapshots,
            });
            Progress::Done(events)
        }
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: steps hung on a resolution wait for the step of their batch, then take the finished steps until their end,
    //! while the resolution's own position stays where it was.
    use mtg_cards::effect::Effect;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::{Resolving, r115_hang, r115_step_done};
    use crate::object::ObjectRef;

    fn draws(count: usize) -> Vec<Effect> {
        (0..count)
            .map(|_| serde_json::from_str(r#"{"Draw":{}}"#).unwrap())
            .collect()
    }

    #[rstest]
    #[case::one_step_left(2, 1)]
    #[case::two_steps_left(3, 1)]
    #[case::from_the_first(3, 0)]
    fn hung_steps_run_after_the_step_of_their_batch(#[case] count: usize, #[case] from: usize) {
        let mut resolving = Resolving::begin(ObjectId(7));
        let source = ObjectRef {
            id: ObjectId(3),
            counter: 0,
        };
        r115_hang(
            &mut resolving,
            (source, PlayerId(0)),
            &draws(count),
            &[from],
            &[],
            &[],
        )
        .unwrap();
        assert!(!resolving.r115_continuing());
        // The step of the batch is the resolution's own, and the fold moves it on.
        assert!(!r115_step_done(&mut resolving, &[1], &[], &[]).unwrap());
        crate::fold::step_done(&mut resolving, &[1], &[], &[]).unwrap();
        assert_eq!(resolving.position, [1]);
        for step in from + 1..=count {
            assert!(resolving.r115_continuing());
            assert_eq!(resolving.r115_step_position(), [step - 1]);
            assert!(r115_step_done(&mut resolving, &[step], &[], &[]).unwrap());
            assert_eq!(resolving.position, [1]);
        }
        assert_eq!(resolving.r115_continued, []);
    }
}
