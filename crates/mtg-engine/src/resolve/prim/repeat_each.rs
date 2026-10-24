//! `Effect::RepeatEach`: the body is done once for each player or object the effect names, with the name bound to the one
//! the pass is for.

use mtg_cards::effect::{Effect, bound_names};

use super::wrong_effect;
use crate::choose::Target;
use crate::resolve::tree::is_next_pass;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The name under which the items still to come are kept; no card can write it, since a name has no `~`.
fn left_name(bind: &str) -> String {
    format!("{bind}~left")
}

/// Begins a pass, at the effect itself: the first time it lists what to repeat for, and at the start of each next pass it
/// takes the next of those it listed then. The pass binds the name to its item and clears what the body of the passes
/// before bound, so the body may bind the same names again; with no item left the repeat is done and the resolution goes on.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RepeatEach {
        over,
        bind,
        effects,
    } = effect
    else {
        return Err(wrong_effect("RepeatEach"));
    };
    let left = left_name(bind);
    let held = cx.resolving.bindings();
    let items = if is_next_pass(cx.resolving.position()) {
        held.get(&left).cloned().unwrap_or_default()
    } else {
        cx.scope.refs(over)?
    };
    let Some((first, rest)) = items.split_first() else {
        return Ok(Run::events(Vec::new()));
    };
    let mut bound: Vec<(String, Vec<Target>)> = Vec::new();
    let names = bound_names(effects)
        .into_iter()
        .chain([bind.clone(), left.clone()]);
    for name in names {
        if held.get(&name).is_some_and(|targets| !targets.is_empty()) {
            bound.push((name, Vec::new()));
        }
    }
    bound.push((bind.clone(), vec![*first]));
    bound.push((left, rest.to_vec()));
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Enter(0),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a repeat lists what it repeats for once, in the order the reference gives, binds the name to the first and keeps the rest for the passes to come; a pass clears the names the body bound before, a list with nothing in it or one that is used up goes on past the repeat, and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn effect(over: &str) -> Effect {
        serde_json::from_str(&format!(
            r#"{{"RepeatEach":{{"over":{over},"as":"each","effects":[{{"LookAtTop":{{"as":"inner"}}}}]}}}}"#
        ))
        .unwrap()
    }

    /// The names the first step binds, and where it goes on.
    fn first(game: &GameState, over: &str) -> (Vec<(String, Vec<Target>)>, Next) {
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        match run(&cx, &effect(over)).unwrap() {
            Run::Done {
                bound,
                next,
                events,
                ..
            } => {
                assert!(events.is_empty());
                (bound, next)
            }
            other => panic!("a repeat asks nothing: {other:?}"),
        }
    }

    #[rstest]
    #[case::two_seats(2, r#""EachPlayer""#, 0, 1)]
    #[case::four_seats(4, r#""EachPlayer""#, 0, 3)]
    #[case::opponents_leave_you_out(3, r#""Opponents""#, 1, 1)]
    fn the_first_item_is_bound_and_the_others_are_kept(
        #[case] seats: usize,
        #[case] over: &str,
        #[case] first_seat: u64,
        #[case] left: usize,
    ) {
        let (bound, next) = first(&game(seats), over);
        assert_eq!(next, Next::Enter(0));
        assert_eq!(
            bound,
            [
                (
                    "each".to_owned(),
                    vec![Target::Player(PlayerId(first_seat))]
                ),
                ("each~left".to_owned(), bound[1].1.clone()),
            ]
        );
        assert_eq!(bound[1].1.len(), left);
    }

    #[test]
    fn nothing_to_repeat_for_goes_on_past_the_repeat() {
        let game = game(2);
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let nobody = effect(r#"{"Each":[{"Named":"Nothing"}]}"#);
        let Run::Done {
            bound,
            next,
            events,
            ..
        } = run(&cx, &nobody).unwrap()
        else {
            panic!("a repeat asks nothing");
        };
        assert!(bound.is_empty() && events.is_empty());
        assert_eq!(next, Next::Sibling);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game(2);
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let error = run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).unwrap_err();
        assert!(error.to_string().contains("RepeatEach"), "{error}");
    }
}
