//! Counters the controller picks as they are removed: "Remove three counters from target permanent" (any kinds) and
//! "Remove up to three counters" (the controller may stop early).
//!
//! Each counter is one `ChooseOption` among the kinds the holder still has (and "Stop" for "up to"); a question with
//! one option is not asked. The answers are read back from the journal in order, so a step with k questions runs k + 1
//! times.

use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;

use super::put_counters::holders;
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::{CountersChanged, kinds_held};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The option that ends an "up to" removal early.
const W2F_STOP: &str = "Stop";

/// The controller removes up to `count` counters of `counter` (of any kind when `None`) from each holder `from` names,
/// one at a time; without `up_to` they remove as many as there are up to the count.
pub(super) fn w2f_run(
    cx: &Cx<'_>,
    from: &Ref,
    counter: Option<&CounterKind>,
    count: u32,
    up_to: bool,
) -> Result<Run, ResolveError> {
    let mut answers = cx.journal().iter();
    let player = cx.scope.controller;
    let mut events = Vec::new();
    for holder in holders(cx, from)? {
        let mut held: Vec<(CounterKind, u32)> = kinds_held(cx.state(), holder)
            .into_iter()
            .filter(|(kind, _)| counter.is_none_or(|wanted| wanted == kind))
            .collect();
        let mut taken: Vec<(CounterKind, i32)> = Vec::new();
        for _ in 0..count {
            let kinds: Vec<CounterKind> = held
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(kind, _)| kind.clone())
                .collect();
            if kinds.is_empty() {
                break;
            }
            let offered = kinds.len() + usize::from(up_to);
            let index = if offered == 1 {
                0
            } else {
                match answers.next() {
                    None => return Ok(Run::Ask(w2f_question(cx, holder, &kinds, up_to))),
                    Some((who, Choice::Option(index))) if *who == player && *index < offered => *index,
                    Some(other) => {
                        return Err(ResolveError::Invalid(format!(
                            "{other:?} is no answer to the choice of a counter to remove"
                        )));
                    }
                }
            };
            let Some(kind) = kinds.get(index) else { break };
            w2f_take(&mut held, &mut taken, kind);
        }
        events.extend(taken.into_iter().map(|(counter, n)| {
            Event::CountersChanged(CountersChanged {
                on: holder,
                counter,
                delta: -n,
            })
        }));
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a counter that was not asked".into(),
        ));
    }
    Ok(Run::events(events))
}

/// One counter of `kind` leaves what is `held` and joins what is `taken`.
fn w2f_take(held: &mut [(CounterKind, u32)], taken: &mut Vec<(CounterKind, i32)>, kind: &CounterKind) {
    if let Some((_, n)) = held.iter_mut().find(|(held, _)| held == kind) {
        *n = n.saturating_sub(1);
    }
    match taken.iter_mut().find(|(took, _)| took == kind) {
        Some((_, n)) => *n += 1,
        None => taken.push((kind.clone(), 1)),
    }
}

/// "Choose a counter to remove.": the kinds still held, then "Stop" for "up to".
fn w2f_question(cx: &Cx<'_>, holder: Target, kinds: &[CounterKind], up_to: bool) -> Decision {
    let mut options: Vec<ChoiceOption> = kinds
        .iter()
        .map(|kind| ChoiceOption::Label(format!("{} counter", kind.name())))
        .collect();
    if up_to {
        options.push(ChoiceOption::Label(W2F_STOP.into()));
    }
    let whose = match holder {
        Target::Object(object) => cx
            .state()
            .object(object.id)
            .map_or_else(String::new, |object| format!(" from {}", object.card().name)),
        Target::Player(_) => String::new(),
    };
    Decision {
        player: cx.scope.controller,
        kind: DecisionKind::ChooseOption(OptionOptions {
            source: cx.scope.source.id,
            options,
            text: format!("Choose a counter to remove{whose}."),
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: removing counters of any kind (or "up to" some of a kind) asks the controller for each counter among the kinds
    //! still held, with "Stop" for "up to", takes without asking when one option is left, never takes more than is held or
    //! than the count, groups what it takes by kind, and refuses answers that were not asked or name no option.
    use mtg_cards::effect::Effect;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    type Outcome = Result<Vec<(String, i32)>, Vec<String>>;

    /// Seat 0 controls the Source; seat 1 controls a Bear with two +1/+1, one shield and one stun counter and a Wolf
    /// with three charge counters.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![
                vec![creature("Source", 1, 1)],
                vec![creature("Bear", 2, 2), creature("Wolf", 2, 2)],
            ],
        );
        game.players[1].battlefield[0].counters = vec![
            (CounterKind::PlusOnePlusOne, 2),
            (CounterKind::Other("shield".into()), 1),
            (CounterKind::Other("stun".into()), 1),
        ];
        game.players[1].battlefield[1].counters = vec![(CounterKind::Charge, 3)];
        GameState::from_setup(&game).unwrap()
    }

    /// Runs the effect with `answers` (option indices of seat 0) in the journal.
    fn run_with(game: &GameState, json: &str, answers: &[usize]) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal: answers
                .iter()
                .map(|index| (ME, Choice::Option(*index)))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        super::super::remove_counters::run(&cx, &effect)
    }

    /// The labels of the question asked, or the (kind, delta) removed when done.
    fn outcome(run: Run) -> Outcome {
        match run {
            Run::Ask(decision) => {
                let DecisionKind::ChooseOption(options) = decision.kind else {
                    panic!("a choice of option");
                };
                assert_eq!(decision.player, ME);
                Err(options
                    .options
                    .into_iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label,
                        other => panic!("{other:?}"),
                    })
                    .collect())
            }
            Run::Done { events, .. } => Ok(events
                .into_iter()
                .map(|event| {
                    let Event::CountersChanged(changed) = event else {
                        panic!("{event:?}");
                    };
                    (changed.counter.name().to_owned(), changed.delta)
                })
                .collect()),
            Run::Cast(_) => panic!("no cast"),
        }
    }

    fn asked(labels: &[&str]) -> Outcome {
        Err(labels.iter().map(|label| (*label).to_owned()).collect())
    }

    fn removed(taken: &[(&str, i32)]) -> Outcome {
        Ok(taken.iter().map(|(kind, n)| ((*kind).to_owned(), *n)).collect())
    }

    #[rstest]
    #[case::any_kind_asks_first("Bear", "", 3, &[], asked(&["+1/+1 counter", "shield counter", "stun counter"]))]
    #[case::any_kind_takes_each_answer("Bear", "", 3, &[0, 1, 0], removed(&[("+1/+1", -2), ("shield", -1)]))]
    #[case::a_kind_used_up_is_no_longer_offered("Bear", "", 3, &[0, 0], asked(&["shield counter", "stun counter"]))]
    #[case::up_to_offers_stop("Bear", r#","up_to":true"#, 3, &[], asked(&["+1/+1 counter", "shield counter", "stun counter", "Stop"]))]
    #[case::stop_ends_early("Bear", r#","up_to":true"#, 3, &[2, 2], removed(&[("stun", -1)]))]
    #[case::stop_at_once_removes_nothing("Bear", r#","up_to":true"#, 3, &[3], removed(&[]))]
    #[case::one_kind_is_not_asked("Wolf", "", 2, &[], removed(&[("charge", -2)]))]
    #[case::never_more_than_held("Wolf", "", 5, &[], removed(&[("charge", -3)]))]
    #[case::up_to_of_a_named_kind_asks_kind_or_stop("Wolf", r#","counter":"charge","up_to":true"#, 2, &[0], asked(&["charge counter", "Stop"]))]
    #[case::up_to_of_a_named_kind_ignores_others("Bear", r#","counter":"+1/+1","up_to":true"#, 3, &[0, 0], removed(&[("+1/+1", -2)]))]
    #[case::nothing_held_nothing_asked("Source", "", 3, &[], removed(&[]))]
    #[case::a_count_of_nothing("Bear", "", 0, &[], removed(&[]))]
    fn counters_are_picked_one_at_a_time(
        #[case] holder: &str,
        #[case] more: &str,
        #[case] count: i32,
        #[case] answers: &[usize],
        #[case] expected: Outcome,
    ) {
        let game = game();
        let json = format!(
            r#"{{"RemoveCounters":{{"from":{{"Each":[{{"Named":"{holder}"}}]}}{more},"count":{count}}}}}"#
        );
        assert_eq!(outcome(run_with(&game, &json, answers).unwrap()), expected);
    }

    #[rstest]
    #[case::an_index_past_the_options(&[3])]
    #[case::an_answer_never_asked(&[0, 0, 1, 0])]
    fn wrong_answers_are_refused(#[case] answers: &[usize]) {
        let game = game();
        let json = r#"{"RemoveCounters":{"from":{"Each":[{"Named":"Bear"}]},"count":3}}"#;
        assert!(run_with(&game, json, answers).is_err());
    }
}
