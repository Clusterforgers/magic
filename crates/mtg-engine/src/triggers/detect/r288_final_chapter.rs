//! The final chapter ability of a Saga triggers or resolves (CR 714.2c): what "Whenever the final chapter ability of a
//! Saga you control triggers" and "... resolves" look for. The Saga is the object of the event.

use mtg_cards::trigger::TriggerPattern;

use super::{Happened, cause};
use crate::abilities::Cause;
use crate::choose::Target;
use crate::events::{Event, ResolutionOutcome};
use crate::object::GameObject;
use crate::resolve::Scope;
use crate::state::GameState;

/// The final chapter ability that triggered or resolved in this event, judged after it (the Saga is sacrificed later).
pub(super) fn r288_happened(_pre: &GameState, event: &Event, post: &GameState) -> Option<Happened> {
    match event {
        Event::AbilityTriggered(triggered) if triggered.delayed.is_none() => {
            let pending = post
                .pending_triggers()
                .iter()
                .find(|pending| pending.trigger() == &**triggered)?;
            let number = crate::saga::r288_chapter_number(pending.rules())?;
            let saga = triggered.source.id;
            (crate::saga::final_chapter(post, saga) == Some(number)).then_some(
                Happened::FinalChapter {
                    saga,
                    resolved: false,
                },
            )
        }
        Event::ResolutionEnded {
            object,
            outcome: ResolutionOutcome::Resolved,
        } => {
            let (ability, source, number) = post.r288_resolved_chapter?;
            (ability == *object && crate::saga::final_chapter(post, source.id) == Some(number))
                .then_some(Happened::FinalChapter {
                    saga: source.id,
                    resolved: true,
                })
        }
        _ => None,
    }
}

/// The cause of a final chapter pattern the happening matches: the Saga, when the selector picks it.
pub(super) fn r288_matching(
    pattern: &TriggerPattern,
    happened: &Happened,
    scope: &Scope<'_>,
) -> Option<Cause> {
    let (
        TriggerPattern::FinalChapter { what, resolves },
        Happened::FinalChapter { saga, resolved },
    ) = (pattern, happened)
    else {
        return None;
    };
    if resolves != resolved {
        return None;
    }
    let saga = scope.state.object(*saga).map(GameObject::reference)?;
    scope
        .selects(what, saga)
        .ok()?
        .then(|| cause(scope.state, vec![Target::Object(saga)]))
}
