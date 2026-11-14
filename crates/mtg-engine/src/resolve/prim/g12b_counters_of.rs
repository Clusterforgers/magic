//! `Effect::PutCountersOf`: counters of each kind another object or player holds are put on a holder ("put its counters
//! on target creature you control", CR 122.1); the object read may have left, and is then read as it last existed.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::l292_put_or_remove::Pick;
use super::put_counters::{holders, whole};
use super::wrong_effect;
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The kinds `of` names hold, read before any counter moves: for each kind, its number added up over what `of` names.
/// Each holder `on` names then gets `each` of every kind, or as many as were read. Nothing is put for a kind of none.
/// With `per_kind` the controller chooses for each kind, in turn, what is done instead (`l292_put_or_remove`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PutCountersOf {
        on,
        of,
        each,
        one_kind,
        per_kind,
        put,
        absent_only,
    } = effect
    else {
        return Err(wrong_effect("PutCountersOf"));
    };
    let mut kinds: Vec<(CounterKind, u32)> = Vec::new();
    if *put {
        // "The same number and kind of counters": what the triggering event put, whatever the holder has now.
        kinds = cx
            .scope
            .cause
            .map(|cause| cause.counters.clone())
            .ok_or_else(|| ResolveError::Invalid("there is no triggering event".into()))?;
    }
    for named in cx.scope.refs(of)?.into_iter().filter(|_| !*put) {
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
    let may_one = *one_kind && *per_kind == Some(mtg_cards::effect::PerKind::May);
    match r6102_one_kind(cx, kinds, *one_kind, may_one)? {
        Ok(picked) => kinds = picked,
        Err(asked) => return Ok(asked),
    }
    let each = match each {
        Some(amount) => Some(whole(cx.scope.amount(amount)?)),
        None => None,
    };
    let picks = match per_kind.as_ref().filter(|_| !may_one) {
        Some(choice) => match super::l292_put_or_remove::l292_picks(cx, &kinds, *choice)? {
            Ok(picks) => picks,
            Err(asked) => return Ok(asked),
        },
        None => vec![Pick::Same; kinds.len()],
    };
    let mut events = Vec::new();
    for holder in holders(cx, on)? {
        for ((kind, n), pick) in kinds.iter().zip(&picks) {
            if *absent_only && r6102_holds(cx, holder, kind) {
                continue;
            }
            let kind = match pick {
                Pick::Same => kind,
                Pick::Nothing => continue,
                Pick::PlusOne => &CounterKind::PlusOnePlusOne,
                Pick::Remove => {
                    events.push(Event::CountersChanged(CountersChanged {
                        on: holder,
                        counter: kind.clone(),
                        delta: -1,
                    }));
                    continue;
                }
            };
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

/// The kinds left once `one_kind` picked one, and for `PerKind::May` once the controller said whether to put it.
fn r6102_one_kind(
    cx: &Cx<'_>,
    mut kinds: Vec<(CounterKind, u32)>,
    one_kind: bool,
    may_one: bool,
) -> Result<Result<Vec<(CounterKind, u32)>, Run>, ResolveError> {
    if may_one {
        // "Choose a counter on it. You may put an additional counter of that kind": the kind first, then whether.
        let used = match super::ep22_one_kind::r6102_pick(cx, &kinds, true)? {
            Ok((picked, used)) => {
                kinds = picked;
                used
            }
            Err(asked) => return Ok(Err(asked)),
        };
        match r6102_whether(cx, &kinds, used)? {
            Ok(true) => {}
            Ok(false) => kinds.clear(),
            Err(asked) => return Ok(Err(asked)),
        }
    } else if one_kind {
        match super::ep22_one_kind::ep22_pick(cx, &kinds)? {
            Ok(picked) => kinds = picked,
            Err(asked) => return Ok(Err(asked)),
        }
    }
    Ok(Ok(kinds))
}

/// Whether the controller puts another counter of the one kind picked, the answer after the `used` ones of the pick, or
/// the question; nothing is asked when no kind was picked.
fn r6102_whether(
    cx: &Cx<'_>,
    kinds: &[(CounterKind, u32)],
    used: usize,
) -> Result<Result<bool, Run>, ResolveError> {
    let Some((kind, _)) = kinds.first() else {
        return Ok(Ok(false));
    };
    let player = cx.scope.controller;
    let mut answers = cx.journal().iter().skip(used);
    let put = match answers.next() {
        None => {
            let name = kind.name();
            return Ok(Err(Run::Ask(Decision {
                player,
                kind: DecisionKind::ChooseOption(OptionOptions {
                    can_cancel: false,
                    source: cx.scope.source.id,
                    options: vec![
                        ChoiceOption::Label(format!("Put another {name} counter")),
                        ChoiceOption::Label("Put nothing".to_owned()),
                    ],
                    text: format!("You may put another {name} counter on it."),
                }),
            })));
        }
        Some((who, Choice::Option(index))) if *who == player && *index < 2 => *index == 0,
        Some(other) => {
            return Err(ResolveError::Invalid(format!(
                "{other:?} is no answer to whether another counter is put"
            )));
        }
    };
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question about counters that was not asked".into(),
        ));
    }
    Ok(Ok(put))
}

/// Whether the holder has a counter of the kind now ("if it doesn't have a counter of that kind on it").
fn r6102_holds(cx: &Cx<'_>, holder: Target, kind: &CounterKind) -> bool {
    match holder {
        Target::Player(player) => cx
            .state()
            .player(player)
            .is_some_and(|p| p.counters().any(|(held, n)| held == kind && n > 0)),
        Target::Object(object) => cx
            .state()
            .object(object.id)
            .and_then(|o| o.permanent())
            .is_some_and(|p| p.counters_of(kind) > 0),
    }
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
