//! `Effect::Monstrosity`: the source becomes monstrous (CR 701.37).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;

use super::explore::actors;
use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::{GameObject, PermanentState};
use crate::resolve::{Cx, ResolveError, Run};

/// A source that is on the battlefield and not monstrous gets the counters, as a proposed `CountersChanged` that a
/// replacement effect sees, and becomes monstrous: one `Monstrous` follows the counters, also when there are none
/// (a count of 0 or less, or counters a replacement effect took away). A source that has left the battlefield or is
/// monstrous already does nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Monstrosity { count } = effect else {
        return Err(wrong_effect("Monstrosity"));
    };
    let delta = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    for source in actors(cx, &Ref::This)? {
        let Some(holder) = source.permanent else {
            continue;
        };
        let monstrous = cx
            .state()
            .object(holder.id)
            .and_then(GameObject::permanent)
            .is_none_or(PermanentState::monstrous);
        if monstrous {
            continue;
        }
        if delta > 0 {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            }));
        }
        events.push(Event::Monstrous {
            object: source.object,
            player: source.player,
        });
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a monstrosity on a source that is on the battlefield and not monstrous proposes the counters (none for a count of 0 or less) and one `Monstrous` after them; on a monstrous source or one that has left the battlefield it does nothing; and it refuses another effect.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// The Source is on the battlefield of seat 0, or in its hand.
    fn game(on_board: bool) -> GameState {
        let source = vec![creature("Source", 1, 1)];
        let (hand, board) = if on_board {
            (vec![], source)
        } else {
            (source, vec![])
        };
        GameState::from_setup(&setup(vec![hand, vec![]], vec![board, vec![]])).unwrap()
    }

    fn run_with(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game
            .objects()
            .find(|o| o.card().name == "Source")
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn events(result: Result<Run, ResolveError>) -> Vec<Event> {
        match result.unwrap() {
            Run::Done { events, .. } => events,
            _ => panic!("done"),
        }
    }

    #[rstest]
    #[case::three_counters(3, 2)]
    #[case::no_counters(0, 1)]
    #[case::a_negative_count(-2, 1)]
    fn a_source_that_is_not_monstrous_becomes_monstrous_after_the_counters(
        #[case] count: i32,
        #[case] expected: usize,
    ) {
        let game = game(true);
        let found = events(run_with(
            &game,
            &format!(r#"{{"Monstrosity":{{"count":{count}}}}}"#),
        ));
        assert_eq!(found.len(), expected);
        assert!(matches!(found.last(), Some(Event::Monstrous { player, .. }) if *player == ME));
        if expected == 2 {
            assert!(matches!(
                &found[0],
                Event::CountersChanged(change) if change.delta == count
            ));
        }
    }

    #[test]
    fn a_monstrous_source_does_nothing() {
        let mut game = game(true);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let (object, player) = (source.id(), ME);
        apply_event(&mut game, &Event::Monstrous { object, player }).unwrap();
        assert!(events(run_with(&game, r#"{"Monstrosity":{"count":3}}"#)).is_empty());
    }

    #[test]
    fn a_source_that_has_left_the_battlefield_does_nothing() {
        let game = game(false);
        assert!(events(run_with(&game, r#"{"Monstrosity":{"count":3}}"#)).is_empty());
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(true);
        assert!(run_with(&game, r#"{"Adapt":{"count":3}}"#).is_err());
    }
}
