//! `Effect::RemoveCounters`: counters are taken off permanents and players (CR 122).

use mtg_cards::effect::Effect;

use super::put_counters::{holders, whole};
use super::wrong_effect;
use crate::counters::{CountersChanged, held_by};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each permanent and player the effect names loses the counters of the kind, up to the count and no more than they have,
/// all together and in the order the reference lists them. What holds none, what cannot hold any (a card in another
/// zone, an object that has changed zones since it was named, a player who has left) and a count of nothing give no
/// event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RemoveCounters {
        from,
        counter,
        count,
        up_to,
    } = effect
    else {
        return Err(wrong_effect("RemoveCounters"));
    };
    let count = whole(cx.scope.amount(count)?);
    let counter = match counter {
        Some(counter) if !*up_to => counter,
        _ if count > 0 => {
            let count = count.unsigned_abs();
            return super::w2f_any_counters::w2f_run(cx, from, counter.as_ref(), count, *up_to);
        }
        _ => return Ok(Run::events(Vec::new())),
    };
    let mut events = Vec::new();
    if count > 0 {
        for holder in holders(cx, from)? {
            let held = held_by(cx.state(), holder, counter);
            let taken = held.min(count.unsigned_abs());
            if let Ok(taken) = i32::try_from(taken)
                && taken > 0
            {
                events.push(Event::CountersChanged(CountersChanged {
                    on: holder,
                    counter: counter.clone(),
                    delta: -taken,
                }));
            }
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: removing counters takes at most the count of the kind from each permanent and player the effect names, never more than they hold and never of another kind, all together and in the order the reference lists them; "all of them" is a count of the counters there are; what holds none, cards in other zones, players who left and counts of nothing give no event; it binds nothing and goes on to the next effect; and it refuses another effect by name.
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

    /// Seat 0 controls the Source and a Wolf with three +1/+1 and one -1/-1 counter and has two poison counters; seat 1
    /// controls a Bear with a charge counter and has an energy counter.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2)],
            ],
        );
        game.players[0].battlefield[1].counters = vec![
            (CounterKind::PlusOnePlusOne, 3),
            (CounterKind::MinusOneMinusOne, 1),
        ];
        game.players[1].battlefield[0].counters = vec![(CounterKind::Charge, 1)];
        game.players[0].counters = vec![(CounterKind::Poison, 2)];
        game.players[1].counters = vec![(CounterKind::Other("energy".into()), 1)];
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
                let holder = match changed.on {
                    Target::Player(player) => format!("seat {}", player.index()),
                    Target::Object(object) => game.object(object.id).unwrap().card().name.clone(),
                };
                (holder, changed.counter, changed.delta)
            })
            .collect()
    }

    fn on(holder: &str, counter: CounterKind, delta: i32) -> (String, CounterKind, i32) {
        (holder.to_owned(), counter, delta)
    }

    #[rstest]
    #[case::one(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1"}}"#,
        vec![on("Wolf", CounterKind::PlusOnePlusOne, -1)]
    )]
    #[case::two(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1","count":2}}"#,
        vec![on("Wolf", CounterKind::PlusOnePlusOne, -2)]
    )]
    #[case::more_than_there_are(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"-1/-1","count":5}}"#,
        vec![on("Wolf", CounterKind::MinusOneMinusOne, -1)]
    )]
    #[case::all_of_them(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1","count":{"Counters":{"on":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1"}}}}"#,
        vec![on("Wolf", CounterKind::PlusOnePlusOne, -3)]
    )]
    #[case::a_kind_it_does_not_hold(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"charge"}}"#,
        vec![]
    )]
    #[case::the_source_holds_none(r#"{"RemoveCounters":{"from":"This","counter":"+1/+1"}}"#, vec![])]
    #[case::a_player(
        r#"{"RemoveCounters":{"from":"You","counter":"poison"}}"#,
        vec![on("seat 0", CounterKind::Poison, -1)]
    )]
    #[case::every_player_that_holds_some(
        r#"{"RemoveCounters":{"from":"EachPlayer","counter":{"Other":"energy"},"count":4}}"#,
        vec![on("seat 1", CounterKind::Other("energy".into()), -1)]
    )]
    #[case::each_permanent_of_the_board(
        r#"{"RemoveCounters":{"from":{"Each":[{"Type":"Creature"}]},"counter":"charge","count":2}}"#,
        vec![on("Bear", CounterKind::Charge, -1)]
    )]
    #[case::a_count_of_nothing(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1","count":0}}"#,
        vec![]
    )]
    #[case::a_negative_count(
        r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1","count":-1}}"#,
        vec![]
    )]
    fn it_takes_counters_off_what_it_names(
        #[case] json: &str,
        #[case] expected: Vec<(String, CounterKind, i32)>,
    ) {
        assert_eq!(proposed(&game(), json), expected);
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
            ResolveError::Invalid("the RemoveCounters primitive was given another effect".into())
        );
    }
}
