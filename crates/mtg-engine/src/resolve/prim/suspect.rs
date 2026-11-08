//! `Effect::Suspect` and `Effect::Unsuspect`: creatures become suspected or stop being so (CR 701.60); `Effect::Solve`:
//! Cases become solved (CR 719.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield becomes suspected (a creature that is not yet) or
/// stops being so (one that is), in the order the reference lists them. Anything else is passed over: a player, an
/// object in another zone or that changed zones since it was named, and a name that repeats.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    if let Effect::Solve { what } = effect {
        return solve(cx, what);
    }
    let (what, suspect) = match effect {
        Effect::Suspect { what } => (what, true),
        Effect::Unsuspect { what } => (what, false),
        _ => return Err(wrong_effect("Suspect")),
    };
    let state = cx.scope.state;
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object
                    .permanent()
                    .is_some_and(|held| held.suspected() != suspect)
                && (!suspect
                    || characteristics(state, object.id()).is_some_and(|chars| chars.is_creature()))
        })
        .map(|object| {
            let object = object.id();
            if suspect {
                Event::Suspected { object }
            } else {
                Event::Unsuspected { object }
            }
        })
        .collect();
    Ok(Run::events(events))
}

/// Each permanent the reference names that is still on the battlefield and not solved yet becomes solved (CR 719.3);
/// anything else is passed over.
fn solve(cx: &Cx<'_>, what: &mtg_cards::refs::Ref) -> Result<Run, ResolveError> {
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.permanent().is_some_and(|held| !held.solved())
        })
        .map(|object| Event::Solved {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a suspected permanent reads as suspected while it is there and, once it has left, as its snapshot holds it; the fold refuses a permanent suspected twice or unsuspected while it is not.
    use mtg_cards::refs::{Pred, Selector};
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::Scope;
    use crate::snapshot::snapshot;
    use crate::zones::Zone;

    #[rstest]
    fn the_sacrificed_creature_reads_as_it_was(#[values(true, false)] suspected: bool) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let bear = game.zone(Zone::Battlefield)[0];
        if suspected {
            apply_event(&mut game, &Event::Suspected { object: bear }).unwrap();
        }
        let named = game.object(bear).unwrap().reference();
        let is = Selector(vec![Pred::Suspected]);
        let live = Scope::announcing(&game, PlayerId(0), named);
        assert_eq!(live.selects(&is, named).unwrap(), suspected);
        let shot = snapshot(&game, bear).unwrap();
        let gone = game.objects.get_mut(&bear).unwrap();
        gone.counter += 1;
        gone.zone = Zone::Graveyard(PlayerId(0));
        let mut scope = Scope::announcing(&game, PlayerId(0), named);
        scope.snapshots = vec![&shot];
        assert_eq!(scope.selects(&is, named).unwrap(), suspected);
    }

    #[test]
    fn the_fold_refuses_what_cannot_change() {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let bear = game.zone(Zone::Battlefield)[0];
        assert!(apply_event(&mut game, &Event::Unsuspected { object: bear }).is_err());
        apply_event(&mut game, &Event::Suspected { object: bear }).unwrap();
        assert!(apply_event(&mut game, &Event::Suspected { object: bear }).is_err());
        apply_event(&mut game, &Event::Unsuspected { object: bear }).unwrap();
        assert!(!game.object(bear).unwrap().permanent().unwrap().suspected());
    }
}
