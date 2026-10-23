//! `Effect::MultiplyCounters`: the counters an object or player holds are multiplied, "double" being the factor two
//! (CR 122.1).

use mtg_cards::effect::Effect;

use super::put_counters::holders;
use super::wrong_effect;
use crate::counters::{CountersChanged, kinds_held};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each permanent and player the effect names that can hold counters gets as many more of the kind as it holds times the
/// factor less one, or of each kind it holds when the effect names none, in the order of the kinds. A holder with none of
/// the kind gets nothing (no event), and a number too large for a change of counters is held at the most a change can be.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::MultiplyCounters {
        on,
        counter,
        factor,
    } = effect
    else {
        return Err(wrong_effect("MultiplyCounters"));
    };
    let mut events = Vec::new();
    for holder in holders(cx, on)? {
        for (kind, held) in kinds_held(cx.state(), holder) {
            if counter.as_ref().is_some_and(|only| *only != kind) {
                continue;
            }
            let more = u64::from(held).saturating_mul(u64::from(factor.saturating_sub(1)));
            let delta = i32::try_from(more).unwrap_or(i32::MAX);
            if delta > 0 {
                events.push(Event::CountersChanged(CountersChanged {
                    on: holder,
                    counter: kind,
                    delta,
                }));
            }
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: multiplying counters puts as many more of a kind on each holder the effect names as it holds times the factor less one (doubling puts the same number again, tripling twice that), of one kind or of each kind it holds in the order of the kinds; a holder with none of the kind, a card in another zone, an object that changed zones since it was named and a name that repeats get nothing; players hold counters too; it binds nothing and goes on to the next effect; and it refuses another effect by name.
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

    /// Seat 0 controls the Source, a Wolf with three +1/+1 counters and two charge counters, a Cat with one -1/-1 counter,
    /// holds a Spell and has two poison counters; seat 1 controls a Bear with four +1/+1 counters and an Ogre with none.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    creature("Cat", 2, 2),
                ],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        );
        game.players[0].battlefield[1].counters = vec![
            (CounterKind::PlusOnePlusOne, 3),
            (CounterKind::Other("charge".into()), 2),
        ];
        game.players[0].battlefield[2].counters = vec![(CounterKind::MinusOneMinusOne, 1)];
        game.players[1].battlefield[0].counters = vec![(CounterKind::PlusOnePlusOne, 4)];
        game.players[0].counters = vec![(CounterKind::Poison, 2)];
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    /// What the effect proposes when it runs as the Source's, as (holder, kind, change).
    fn proposed(game: &GameState, json: &str) -> Vec<(String, CounterKind, i32)> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = run(&cx, &effect).unwrap()
        else {
            panic!("the effect asks nothing");
        };
        assert_eq!(bound.len(), 0);
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

    fn plus(holder: &str, delta: i32) -> (String, CounterKind, i32) {
        (holder.to_owned(), CounterKind::PlusOnePlusOne, delta)
    }

    fn each(name: &str) -> String {
        format!(r#"{{"Each":[{{"Named":"{name}"}}]}}"#)
    }

    #[rstest]
    #[case::doubling_a_kind(
        format!(r#"{{"MultiplyCounters":{{"on":{},"counter":"+1/+1"}}}}"#, each("Wolf")),
        vec![plus("Wolf", 3)]
    )]
    #[case::tripling_a_kind(
        format!(r#"{{"MultiplyCounters":{{"on":{},"counter":"+1/+1","factor":3}}}}"#, each("Wolf")),
        vec![plus("Wolf", 6)]
    )]
    #[case::each_kind_in_the_order_of_the_kinds(
        format!(r#"{{"MultiplyCounters":{{"on":{}}}}}"#, each("Wolf")),
        vec![plus("Wolf", 3), ("Wolf".into(), CounterKind::Other("charge".into()), 2)]
    )]
    #[case::a_kind_it_has_none_of(
        format!(r#"{{"MultiplyCounters":{{"on":{},"counter":"-1/-1"}}}}"#, each("Wolf")),
        vec![]
    )]
    #[case::a_permanent_with_no_counters(
        format!(r#"{{"MultiplyCounters":{{"on":{}}}}}"#, each("Ogre")),
        vec![]
    )]
    #[case::each_creature_of_a_seat(
        r#"{"MultiplyCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}"#.to_owned(),
        vec![plus("Wolf", 3)]
    )]
    #[case::each_creature_of_every_seat(
        r#"{"MultiplyCounters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}"#.to_owned(),
        vec![plus("Wolf", 3), plus("Bear", 4)]
    )]
    #[case::a_player_s_counters(
        r#"{"MultiplyCounters":{"on":"You"}}"#.to_owned(),
        vec![("seat 0".into(), CounterKind::Poison, 2)]
    )]
    #[case::a_card_in_a_hand(
        r#"{"MultiplyCounters":{"on":{"Each":[{"Named":"Spell"},{"In":"Hand"}]}}}"#.to_owned(),
        vec![]
    )]
    #[case::nothing_named(
        format!(r#"{{"MultiplyCounters":{{"on":{}}}}}"#, each("Nobody")),
        vec![]
    )]
    fn it_multiplies_the_counters_a_holder_has(
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
            ResolveError::Invalid("the MultiplyCounters primitive was given another effect".into())
        );
    }
}
