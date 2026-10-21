//! `Effect::PutCounters`: counters are put on permanents and players (CR 122); `RemoveCounters` and `MoveCounters` share
//! the choice of what can hold them.

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent on the battlefield and each player in the game the effect names gets the counters, all together and in
/// the order the reference lists them. An object that has changed zones since it was named (CR 400.7), a card in
/// another zone, a player who has left and a name that repeats are passed over; a count of nothing is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PutCounters { on, counter, count } = effect else {
        return Err(wrong_effect("PutCounters"));
    };
    let count = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    if count > 0 {
        for holder in holders(cx, on)? {
            events.push(Event::CountersChanged(CountersChanged {
                on: holder,
                counter: counter.clone(),
                delta: count,
            }));
        }
    }
    Ok(Run::events(events))
}

/// An amount as a number of counters: none for a negative amount, and no more than a change of counters can hold.
pub(super) fn whole(amount: i64) -> i32 {
    i32::try_from(amount.max(0)).unwrap_or(i32::MAX)
}

/// The players and permanents `reference` names that can hold counters now, each once: a player still in the game, a
/// permanent on the battlefield that is still the object that was named.
pub(super) fn holders(cx: &Cx<'_>, reference: &Ref) -> Result<Vec<Target>, ResolveError> {
    let mut found: Vec<Target> = Vec::new();
    for named in cx.scope.refs(reference)? {
        let holds = match named {
            Target::Player(player) => cx.state().is_alive(player),
            Target::Object(object) => cx.state().object(object.id).is_some_and(|live| {
                live.counter() == object.counter && live.zone() == Zone::Battlefield
            }),
        };
        if holds && !found.contains(&named) {
            found.push(named);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    //! Goal: putting counters gives each permanent and player the effect names the counters, all together and in the order the reference lists them, whatever names them (the source, a chosen target, a name, a selector over the board, a player); the count is worked out when the effect runs (a number, a count of the board, the counters on something) and nothing is done for a count of nothing or less; objects that are not permanents, that changed zones since they were named, players who left and names that repeat are passed over; it binds nothing and goes on to the next effect; and it refuses another effect by name.
    use mtg_cards::value::CounterKind;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::object::ObjectRef;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls the Source, a Wolf with two +1/+1 counters and holds a Spell; seat 1 controls a Bear and an Ogre.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        );
        game.players[0].battlefield[1]
            .counters
            .push((CounterKind::PlusOnePlusOne, 2));
        GameState::from_setup(&game).unwrap()
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
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
        let Run::Done { events, next, .. } = run(&cx, &effect(json)).unwrap() else {
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

    fn plus(holder: &str, delta: i32) -> (String, CounterKind, i32) {
        (holder.to_owned(), CounterKind::PlusOnePlusOne, delta)
    }

    #[rstest]
    #[case::the_source(r#"{"PutCounters":{"on":"This","counter":"+1/+1"}}"#, vec![plus("Source", 1)])]
    #[case::several(r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":3}}"#, vec![plus("Source", 3)])]
    #[case::each_creature_of_a_seat(
        r#"{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"counter":"+1/+1","count":2}}"#,
        vec![plus("Bear", 2), plus("Ogre", 2)]
    )]
    #[case::every_creature(
        r#"{"PutCounters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}"#,
        vec![plus("Source", 1), plus("Wolf", 1), plus("Bear", 1), plus("Ogre", 1)]
    )]
    #[case::the_count_of_the_board(
        r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
        vec![plus("Source", 2)]
    )]
    #[case::the_counters_on_another_object(
        r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Counters":{"on":{"Each":[{"Named":"Wolf"}]},"counter":"+1/+1"}}}}"#,
        vec![plus("Source", 2)]
    )]
    #[case::a_player(
        r#"{"PutCounters":{"on":"You","counter":"poison","count":3}}"#,
        vec![("seat 0".to_owned(), CounterKind::Poison, 3)]
    )]
    #[case::every_opponent(
        r#"{"PutCounters":{"on":"Opponents","counter":{"Other":"energy"},"count":2}}"#,
        vec![("seat 1".to_owned(), CounterKind::Other("energy".into()), 2)]
    )]
    #[case::each_player_and_nothing_more(
        r#"{"PutCounters":{"on":"EachPlayer","counter":"charge"}}"#,
        vec![
            ("seat 0".to_owned(), CounterKind::Charge, 1),
            ("seat 1".to_owned(), CounterKind::Charge, 1),
        ]
    )]
    #[case::a_count_of_nothing(r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":0}}"#, vec![])]
    #[case::a_negative_count(r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":-2}}"#, vec![])]
    #[case::a_card_in_a_hand(
        r#"{"PutCounters":{"on":{"Each":[{"In":"Hand"}]},"counter":"+1/+1"}}"#,
        vec![]
    )]
    #[case::nobody(
        r#"{"PutCounters":{"on":{"Each":[{"Named":"Nobody"}]},"counter":"+1/+1"}}"#,
        vec![]
    )]
    fn it_puts_counters_on_what_it_names(
        #[case] json: &str,
        #[case] expected: Vec<(String, CounterKind, i32)>,
    ) {
        assert_eq!(proposed(&game(), json), expected);
    }

    #[test]
    fn a_player_who_left_and_an_object_that_changed_zones_are_passed_over() {
        let mut game = game();
        let wolf = named(&game, "Wolf");
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(
            proposed(
                &game,
                r#"{"PutCounters":{"on":"Opponents","counter":"poison"}}"#
            ),
            vec![],
            "no opponent is left to take it"
        );
        crate::fold::apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: wolf.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(ME),
                cause: crate::events::ZoneCause::Destroyed,
            },
        )
        .unwrap();
        assert_eq!(
            proposed(
                &game,
                r#"{"PutCounters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}"#
            ),
            vec![plus("Source", 1), plus("Bear", 1), plus("Ogre", 1)],
            "the Wolf is in a graveyard now"
        );
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
        let error = run(&cx, &effect(r#"{"Draw":{}}"#)).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the PutCounters primitive was given another effect".into())
        );
    }
}
