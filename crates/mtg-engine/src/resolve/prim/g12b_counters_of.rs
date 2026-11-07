//! `Effect::PutCountersOf`: counters of each kind another object or player holds are put on a holder ("put its counters
//! on target creature you control", CR 122.1); the object read may have left, and is then read as it last existed.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::put_counters::{holders, whole};
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The kinds `of` names hold, read before any counter moves: for each kind, its number added up over what `of` names.
/// Each holder `on` names then gets `each` of every kind, or as many as were read. Nothing is put for a kind of none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PutCountersOf { on, of, each } = effect else {
        return Err(wrong_effect("PutCountersOf"));
    };
    let mut kinds: Vec<(CounterKind, u32)> = Vec::new();
    for named in cx.scope.refs(of)? {
        let held = match named {
            Target::Player(player) => cx
                .state()
                .player(player)
                .map(|p| p.counters().map(|(kind, n)| (kind.clone(), n)).collect())
                .unwrap_or_default(),
            Target::Object(object) => cx
                .scope
                .seen(object)
                .map(|seen| seen.counters.g12b_kinds())
                .unwrap_or_default(),
        };
        for (kind, n) in held {
            match kinds.iter_mut().find(|(found, _)| *found == kind) {
                Some((_, total)) => *total = total.saturating_add(n),
                None => kinds.push((kind, n)),
            }
        }
    }
    kinds.retain(|(_, n)| *n > 0);
    let each = match each {
        Some(amount) => Some(whole(cx.scope.amount(amount)?)),
        None => None,
    };
    let mut events = Vec::new();
    for holder in holders(cx, on)? {
        for (kind, n) in &kinds {
            let delta = each.unwrap_or_else(|| i32::try_from(*n).unwrap_or(i32::MAX));
            if delta > 0 {
                events.push(Event::CountersChanged(CountersChanged {
                    on: holder,
                    counter: kind.clone(),
                    delta,
                }));
            }
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: putting the counters of another puts on each holder as many of each kind as the object or player read holds, or the number the effect gives of each kind it holds; an object read that has left is read as it last existed, one never seen gives nothing; a holder that is not on the battlefield gets nothing; players are read and get counters too; it binds nothing and goes on to the next effect; and it refuses another effect by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::object::ObjectRef;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls the Source, a Wolf with three +1/+1 counters and two charge counters and a Cat with none, holds a
    /// Spell and has two poison counters; seat 1 controls a Bear with one -1/-1 counter.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    creature("Cat", 2, 2),
                ],
                vec![creature("Bear", 2, 2)],
            ],
        );
        game.players[0].battlefield[1].counters = vec![
            (CounterKind::PlusOnePlusOne, 3),
            (CounterKind::Other("charge".into()), 2),
        ];
        game.players[1].battlefield[0].counters = vec![(CounterKind::MinusOneMinusOne, 1)];
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

    fn each(name: &str) -> String {
        format!(r#"{{"Each":[{{"Named":"{name}"}}]}}"#)
    }

    fn put(on: &str, of: &str, each: &str) -> String {
        format!(r#"{{"PutCountersOf":{{"on":{on},"of":{of}{each}}}}}"#)
    }

    fn row(holder: &str, kind: CounterKind, delta: i32) -> (String, CounterKind, i32) {
        (holder.to_owned(), kind, delta)
    }

    fn charge() -> CounterKind {
        CounterKind::Other("charge".into())
    }

    #[rstest]
    #[case::as_many_of_each_kind(put(&each("Cat"), &each("Wolf"), ""),
        vec![row("Cat", CounterKind::PlusOnePlusOne, 3), row("Cat", charge(), 2)])]
    #[case::one_of_each_kind_it_holds(put(&each("Wolf"), &each("Wolf"), r#","each":1"#),
        vec![row("Wolf", CounterKind::PlusOnePlusOne, 1), row("Wolf", charge(), 1)])]
    #[case::onto_another_seat_s_creature(put(&each("Bear"), &each("Wolf"), ""),
        vec![row("Bear", CounterKind::PlusOnePlusOne, 3), row("Bear", charge(), 2)])]
    #[case::of_one_with_none(put(&each("Wolf"), &each("Cat"), ""), vec![])]
    #[case::of_a_player(put(&each("Cat"), r#""You""#, ""), vec![row("Cat", CounterKind::Poison, 2)])]
    #[case::onto_a_player(put(r#""You""#, &each("Bear"), r#","each":1"#),
        vec![row("seat 0", CounterKind::MinusOneMinusOne, 1)])]
    #[case::onto_a_card_in_a_hand(put(r#"{"Each":[{"Named":"Spell"},{"In":"Hand"}]}"#, &each("Wolf"), ""), vec![])]
    #[case::of_nothing_named(put(&each("Cat"), &each("Nobody"), ""), vec![])]
    #[case::each_of_none(put(&each("Wolf"), &each("Wolf"), r#","each":0"#), vec![])]
    fn it_puts_the_counters_of_another(
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
            ResolveError::Invalid("the PutCountersOf primitive was given another effect".into())
        );
    }
}
