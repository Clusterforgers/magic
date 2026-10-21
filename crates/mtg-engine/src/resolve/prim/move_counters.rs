//! `Effect::MoveCounters`: counters are moved from one permanent onto another (CR 122.5).

use mtg_cards::effect::Effect;

use super::put_counters::{holders, whole};
use super::wrong_effect;
use crate::counters::{CountersChanged, held_by};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The first permanent the effect names to give them up loses up to the count of the kind, no more than it has, and the
/// first it names to take them gets that many, in one step. Nothing moves when either is missing (not on the battlefield,
/// changed zones since it was named, not an object), when they are the same permanent, or when the first holds none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::MoveCounters {
        from,
        to,
        counter,
        count,
    } = effect
    else {
        return Err(wrong_effect("MoveCounters"));
    };
    let count = whole(cx.scope.amount(count)?);
    let (Some(source), Some(target)) = (
        holders(cx, from)?
            .into_iter()
            .find(|t| t.object().is_some()),
        holders(cx, to)?.into_iter().find(|t| t.object().is_some()),
    ) else {
        return Ok(Run::events(Vec::new()));
    };
    let moved = held_by(cx.state(), source, counter).min(count.unsigned_abs());
    let mut events = Vec::new();
    if source != target
        && let Ok(moved) = i32::try_from(moved)
        && moved > 0
    {
        for (on, delta) in [(source, -moved), (target, moved)] {
            events.push(Event::CountersChanged(CountersChanged {
                on,
                counter: counter.clone(),
                delta,
            }));
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: moving counters takes up to the count of the kind off the first permanent and puts as many on the second in one step, never more than the first has and never of another kind; nothing moves when the first holds none, when either is missing (a card elsewhere, an object that changed zones since it was named, a player), or when both are one permanent; it binds nothing and goes on to the next effect; and it refuses another effect by name.
    use mtg_cards::value::CounterKind;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Target;
    use crate::fixtures::{creature, setup};
    use crate::object::ObjectRef;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls the Source and a Wolf with three +1/+1 counters and holds a Spell; seat 1 controls a Bear.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2)],
            ],
        );
        game.players[0].battlefield[1].counters = vec![(CounterKind::PlusOnePlusOne, 3)];
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    fn proposed(game: &GameState, json: &str) -> Vec<(String, CounterKind, i32)> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, next, .. } = run(&cx, &effect).unwrap() else {
            panic!("the effect asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
            .into_iter()
            .map(|event| {
                let Event::CountersChanged(changed) = event else {
                    panic!("{event:?}");
                };
                let Target::Object(object) = changed.on else {
                    panic!("counters move between objects");
                };
                let holder = game.object(object.id).unwrap().card().name.clone();
                (holder, changed.counter, changed.delta)
            })
            .collect()
    }

    fn plus(holder: &str, delta: i32) -> (String, CounterKind, i32) {
        (holder.to_owned(), CounterKind::PlusOnePlusOne, delta)
    }

    fn moving(from: &str, to: &str, extra: &str) -> String {
        format!(
            r#"{{"MoveCounters":{{"from":{{"Each":[{{"Named":"{from}"}}]}},"to":{{"Each":[{{"Named":"{to}"}}]}},"counter":"+1/+1"{extra}}}}}"#
        )
    }

    #[rstest]
    #[case::one(moving("Wolf", "Bear", ""), vec![plus("Wolf", -1), plus("Bear", 1)])]
    #[case::two(moving("Wolf", "Source", r#","count":2"#), vec![plus("Wolf", -2), plus("Source", 2)])]
    #[case::more_than_there_are(moving("Wolf", "Bear", r#","count":9"#), vec![plus("Wolf", -3), plus("Bear", 3)])]
    #[case::from_a_permanent_with_none(moving("Bear", "Wolf", ""), vec![])]
    #[case::onto_itself(moving("Wolf", "Wolf", ""), vec![])]
    #[case::a_count_of_nothing(moving("Wolf", "Bear", r#","count":0"#), vec![])]
    #[case::from_nothing(moving("Nobody", "Bear", ""), vec![])]
    #[case::onto_nothing(moving("Wolf", "Nobody", ""), vec![])]
    #[case::onto_a_card_in_a_hand(
        r#"{"MoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"to":{"Each":[{"Named":"Spell"},{"In":"Hand"}]},"counter":"+1/+1"}}"#.to_owned(),
        vec![]
    )]
    #[case::the_first_object_of_several_takes_them(
        r#"{"MoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"to":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}"#.to_owned(),
        vec![plus("Wolf", -1), plus("Source", 1)]
    )]
    #[case::the_first_object_of_several_gives_them(
        r#"{"MoveCounters":{"from":{"Each":[{"Type":"Creature"}]},"to":{"Each":[{"Named":"Bear"}]},"counter":"+1/+1"}}"#.to_owned(),
        vec![]
    )]
    fn it_moves_counters_between_permanents(
        #[case] json: String,
        #[case] expected: Vec<(String, CounterKind, i32)>,
    ) {
        assert_eq!(proposed(&game(), &json), expected);
    }

    #[test]
    fn it_refuses_another_effect_by_name() {
        let game = game();
        let source = named(&game, "Source");
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(r#"{"Draw":{}}"#).unwrap();
        assert_eq!(
            run(&cx, &effect).unwrap_err(),
            ResolveError::Invalid("the MoveCounters primitive was given another effect".into())
        );
    }
}
