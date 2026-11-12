//! What the controller of a `PutCountersOf` chooses for each kind of counter it reads (`per_kind`), one `ChooseOption`
//! for each kind in turn: "for each kind of counter on target permanent, put another counter of that kind on it or remove
//! one from it" and "for each kind of counter on permanents you control, you may put your choice of a +1/+1 counter or a
//! counter of that kind on ~".

use mtg_cards::effect::PerKind;
use mtg_cards::value::CounterKind;

use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::resolve::{Cx, ResolveError, Run};

/// What is done for one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pick {
    /// Counters of that kind are put.
    Same,
    /// One counter of that kind is removed.
    Remove,
    /// A +1/+1 counter is put instead.
    PlusOne,
    /// Nothing is done.
    Nothing,
}

/// The options of one kind and what each does.
fn options(kind: &CounterKind, choice: PerKind) -> (Vec<(String, Pick)>, String) {
    let name = kind.name();
    match choice {
        PerKind::PutOrRemove => (
            vec![
                (format!("Put another {name} counter"), Pick::Same),
                (format!("Remove a {name} counter"), Pick::Remove),
            ],
            format!("Put another {name} counter on it or remove one from it."),
        ),
        PerKind::MayPlusOneInstead => (
            vec![
                (format!("Put a {name} counter"), Pick::Same),
                ("Put a +1/+1 counter".to_owned(), Pick::PlusOne),
                ("Put nothing".to_owned(), Pick::Nothing),
            ],
            format!("You may put a +1/+1 counter or a {name} counter on it."),
        ),
    }
}

/// For each of `kinds`, what the controller picked, or the question.
pub(super) fn l292_picks(
    cx: &Cx<'_>,
    kinds: &[(CounterKind, u32)],
    choice: PerKind,
) -> Result<Result<Vec<Pick>, Run>, ResolveError> {
    let mut picks = Vec::with_capacity(kinds.len());
    let mut choosers = Vec::with_capacity(kinds.len());
    for (kind, _) in kinds {
        let (offered, text) = options(kind, choice);
        choosers.push(OptionChooser {
            player: cx.scope.controller,
            options: offered
                .iter()
                .map(|(label, _)| ChoiceOption::Label(label.clone()))
                .collect(),
            text,
        });
        picks.push(
            offered
                .into_iter()
                .map(|(_, pick)| pick)
                .collect::<Vec<_>>(),
        );
    }
    match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => Ok(Err(Run::Ask(decision))),
        Optioned::Done(taken) => Ok(Ok(taken
            .iter()
            .zip(&picks)
            .map(|((_, picked), offered)| offered.get(*picked).copied().unwrap_or(Pick::Nothing))
            .collect())),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a choice for each kind asks the controller once for each kind held, in turn, and the answers decide for each kind whether a counter of it is put, one is removed, a +1/+1 counter is put instead or nothing is; a permanent with no counters asks nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use crate::choose::Choice;
    use crate::counters::CountersChanged;
    use crate::decision::DecisionKind;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Cx, Resolving, Run, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;
    use mtg_cards::value::CounterKind;

    const EFFECT: &str =
        r#"{"PutCountersOf":{"on":"This","of":"This","each":1,"per_kind":"PutOrRemove"}}"#;

    /// A creature with the counters of `kinds`, two of each.
    fn game(kinds: &[CounterKind]) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let id = game.zone(Zone::Battlefield)[0];
        let on = crate::choose::Target::Object(game.object(id).unwrap().reference());
        for kind in kinds {
            crate::fold::apply_event(
                &mut game,
                &Event::CountersChanged(CountersChanged {
                    on,
                    counter: kind.clone(),
                    delta: 2,
                }),
            )
            .unwrap();
        }
        game
    }

    const PLUS: &str =
        r#"{"PutCountersOf":{"on":"This","of":"This","each":1,"per_kind":"MayPlusOneInstead"}}"#;

    fn play(game: &GameState, picks: &[usize]) -> Run {
        play_with(EFFECT, game, picks)
    }

    fn play_with(json: &str, game: &GameState, picks: &[usize]) -> Run {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal: picks
                .iter()
                .map(|pick| (PlayerId(0), Choice::Option(*pick)))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        super::super::run(&cx, &serde_json::from_str(json).unwrap()).unwrap()
    }

    #[rstest]
    #[case::the_kind(&[0], &[("loyalty", 1)])]
    #[case::a_plus_one_instead(&[1], &[("+1/+1", 1)])]
    #[case::nothing(&[2], &[])]
    fn a_plus_one_counter_may_stand_for_a_kind(
        #[case] picks: &[usize],
        #[case] expected: &[(&str, i32)],
    ) {
        let found = deltas(play_with(PLUS, &game(&[CounterKind::Loyalty]), picks));
        let expected: Vec<(String, i32)> = expected
            .iter()
            .map(|(kind, delta)| ((*kind).to_owned(), *delta))
            .collect();
        assert_eq!(found, expected);
    }

    fn deltas(run: Run) -> Vec<(String, i32)> {
        let Run::Done { events, .. } = run else {
            panic!("the step is done");
        };
        events
            .into_iter()
            .map(|event| match event {
                Event::CountersChanged(changed) => {
                    (changed.counter.name().to_owned(), changed.delta)
                }
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::none(&[], &[], &[])]
    #[case::put_one_kind(&[CounterKind::PlusOnePlusOne], &[0], &[("+1/+1", 1)])]
    #[case::remove_one_kind(&[CounterKind::PlusOnePlusOne], &[1], &[("+1/+1", -1)])]
    #[case::two_kinds_each_their_own(
        &[CounterKind::PlusOnePlusOne, CounterKind::Loyalty],
        &[1, 0],
        &[("+1/+1", -1), ("loyalty", 1)]
    )]
    fn each_kind_is_put_or_removed_as_answered(
        #[case] kinds: &[CounterKind],
        #[case] picks: &[usize],
        #[case] expected: &[(&str, i32)],
    ) {
        let found = deltas(play(&game(kinds), picks));
        let expected: Vec<(String, i32)> = expected
            .iter()
            .map(|(kind, delta)| ((*kind).to_owned(), *delta))
            .collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn each_kind_is_asked_in_turn() {
        let game = game(&[CounterKind::PlusOnePlusOne, CounterKind::Loyalty]);
        for answered in 0..2 {
            let Run::Ask(decision) = play(&game, &vec![0; answered]) else {
                panic!("kind {answered} is asked");
            };
            assert!(matches!(decision.kind, DecisionKind::ChooseOption(_)));
            assert_eq!(decision.player, PlayerId(0));
        }
    }
}
