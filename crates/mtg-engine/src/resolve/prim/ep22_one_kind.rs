//! "Choose a counter on target permanent. Put an additional counter of that kind on that permanent": the controller picks
//! one of the kinds held (`PutCountersOf.one_kind`), with one `ChooseOption` asked only when two or more kinds are held.

use mtg_cards::value::CounterKind;

use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, ResolveError, Run};

/// The one kind of `kinds` the controller picks, as a list of it alone (empty when none is held), or the question.
pub(super) fn ep22_pick(
    cx: &Cx<'_>,
    kinds: &[(CounterKind, u32)],
) -> Result<Result<Vec<(CounterKind, u32)>, Run>, ResolveError> {
    let mut answers = cx.journal().iter();
    if kinds.len() <= 1 {
        if answers.next().is_some() {
            return Err(ResolveError::Invalid(
                "an answer was given to a choice of a kind of counter that was not asked".into(),
            ));
        }
        return Ok(Ok(kinds.to_vec()));
    }
    let player = cx.scope.controller;
    let index = match answers.next() {
        None => return Ok(Err(Run::Ask(question(cx, kinds)))),
        Some((who, Choice::Option(index))) if *who == player && *index < kinds.len() => *index,
        Some(other) => {
            return Err(ResolveError::Invalid(format!(
                "{other:?} is no answer to the choice of a kind of counter"
            )));
        }
    };
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a kind of counter that was not asked".into(),
        ));
    }
    Ok(Ok(kinds.get(index).cloned().into_iter().collect()))
}

/// "Choose a counter.": the kinds held.
fn question(cx: &Cx<'_>, kinds: &[(CounterKind, u32)]) -> Decision {
    Decision {
        player: cx.scope.controller,
        kind: DecisionKind::ChooseOption(OptionOptions {
            can_cancel: false,
            source: cx.scope.source.id,
            options: kinds
                .iter()
                .map(|(kind, _)| ChoiceOption::Label(format!("{} counter", kind.name())))
                .collect(),
            text: "Choose a counter; another of its kind is put.".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: putting another counter of one kind asks the controller which kind only when two or more are held, puts one
    //! counter of the kind picked (or of the only kind) on the holder, nothing when none is held, and refuses an answer
    //! that names no kind or was not asked.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mtg_cards::effect::Effect;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Target;
    use crate::events::Event;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Each (holder, kind, delta) proposed.
    type Changes = Vec<(String, String, i32)>;

    /// Seat 0 controls the Source, a Wolf with two +1/+1 counters and one charge counter, and a Cat with one shield
    /// counter; seat 1 controls a Bear with none.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    creature("Cat", 1, 1),
                ],
                vec![creature("Bear", 2, 2)],
            ],
        );
        game.players[0].battlefield[1].counters =
            vec![(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)];
        game.players[0].battlefield[2].counters = vec![(CounterKind::Other("shield".into()), 1)];
        GameState::from_setup(&game).unwrap()
    }

    fn effect(name: &str) -> String {
        format!(
            r#"{{"PutCountersOf":{{"on":{{"Each":[{{"Named":"{name}"}}]}},"of":{{"Each":[{{"Named":"{name}"}}]}},"each":1,"one_kind":true}}}}"#
        )
    }

    /// `Ok(None)` when a question is asked, else each (holder, kind, delta) proposed.
    fn run_with(json: &str, answers: &[usize]) -> Result<Option<Changes>, ResolveError> {
        let game = game();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal: answers
                .iter()
                .map(|index| (ME, Choice::Option(*index)))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        Ok(match super::super::g12b_counters_of::run(&cx, &effect)? {
            Run::Ask(_) => None,
            Run::Done { events, .. } => Some(
                events
                    .into_iter()
                    .map(|event| {
                        let Event::CountersChanged(changed) = event else {
                            panic!("{event:?}");
                        };
                        let Target::Object(on) = changed.on else {
                            panic!("objects only");
                        };
                        let holder = game.object(on.id).unwrap().card().name.clone();
                        (holder, changed.counter.name().to_owned(), changed.delta)
                    })
                    .collect(),
            ),
            Run::Cast(_) => panic!("no cast"),
        })
    }

    fn put(holder: &str, kind: &str) -> Changes {
        vec![(holder.to_owned(), kind.to_owned(), 1)]
    }

    #[rstest]
    #[case::two_kinds_ask("Wolf", &[], None)]
    #[case::the_first_kind("Wolf", &[0], Some(put("Wolf", "+1/+1")))]
    #[case::the_second_kind("Wolf", &[1], Some(put("Wolf", "charge")))]
    #[case::one_kind_held("Cat", &[], Some(put("Cat", "shield")))]
    #[case::none_held("Bear", &[], Some(vec![]))]
    fn one_kind_gets_another(
        #[case] holder: &str,
        #[case] answers: &[usize],
        #[case] expected: Option<Changes>,
    ) {
        assert_eq!(run_with(&effect(holder), answers).unwrap(), expected);
    }

    #[rstest]
    #[case::no_such_kind("Wolf", &[2])]
    #[case::an_answer_too_many("Wolf", &[0, 1])]
    #[case::not_asked_of_one_kind("Cat", &[0])]
    fn a_wrong_answer_is_refused(#[case] holder: &str, #[case] answers: &[usize]) {
        assert!(run_with(&effect(holder), answers).is_err());
    }
}
