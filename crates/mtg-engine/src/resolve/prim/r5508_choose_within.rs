//! `Effect::ChooseWithin`: players choose any number of objects whose powers or mana values add up to at most a total.

use mtg_cards::effect::Effect;
use mtg_cards::refs::TargetSet;
use mtg_core::ids::{ObjectId, PlayerId};

use super::choose::candidates_of;
use super::wrong_effect;
use crate::choose::{CardsTotal, Picked, Selection, Target, pick_selections_within};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::state::GameState;

/// Each player `who` names, in the order named, chooses any number of their candidates (all of what `from` names, or
/// their own with `Ref::Chooser`) whose values add up to at most the total; a negative power subtracts (the Slaughter
/// the Strong ruling). What they chose is bound together to `as`, the other candidates to `rest_as`.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChooseWithin {
        who,
        from,
        within,
        bind,
        rest_as,
    } = effect
    else {
        return Err(wrong_effect("ChooseWithin"));
    };
    let (most, power) = match within {
        TargetSet::TotalPowerAtMost(most) => (most, true),
        TargetSet::TotalManaValueAtMost(most) => (most, false),
        _ => {
            return Err(ResolveError::Invalid(
                "a choice within a total adds up powers or mana values".into(),
            ));
        }
    };
    let most = i32::try_from(cx.scope.amount(most)?).unwrap_or(i32::MAX);
    let choosers: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .collect();
    let (candidates, own) = candidates_of(cx, from, &choosers)?;
    let state = cx.state();
    let stat = if power { "power" } else { "mana value" };
    let mut selections = Vec::with_capacity(choosers.len());
    let mut totals = Vec::with_capacity(choosers.len());
    for (player, mine) in choosers.iter().zip(&own) {
        let ids: Vec<ObjectId> = mine
            .iter()
            .filter_map(|t| t.object())
            .map(|o| o.id)
            .collect();
        let values: Vec<i32> = ids
            .iter()
            .map(|id| r5508_value(state, *id, power))
            .collect();
        totals.push(Some(CardsTotal {
            most,
            values,
            stat: stat.to_owned(),
        }));
        selections.push(Selection {
            one_per_name: false,
            groups: Vec::new(),
            player: *player,
            max: ids.len(),
            min: 0,
            from: ids,
            text: format!("Choose any number with total {stat} {most} or less."),
        });
    }
    let chosen: Vec<Target> = match pick_selections_within(cx, &selections, &totals)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(taken) => taken
            .into_iter()
            .flat_map(|(_, cards)| cards)
            .filter_map(|id| {
                candidates
                    .iter()
                    .find(|target| target.object().is_some_and(|o| o.id == id))
                    .copied()
            })
            .collect(),
    };
    let mut bound = vec![(bind.clone(), chosen.clone())];
    if let Some(rest) = rest_as {
        let others = candidates
            .into_iter()
            .filter(|target| target.object().is_some() && !chosen.contains(target))
            .collect();
        bound.push((rest.clone(), others));
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The power of the object as it is now, negative kept, 0 without one; or its mana value.
fn r5508_value(state: &GameState, id: ObjectId, power: bool) -> i32 {
    if power {
        crate::characteristics::characteristics(state, id)
            .and_then(|chars| chars.power)
            .unwrap_or(0)
    } else {
        state.object(id).map_or(0, |object| {
            i32::try_from(object.mana_value()).unwrap_or(i32::MAX)
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each chooser is asked for any number of their own candidates within the total, with each candidate's value (a negative power kept, a mana value), an answer over the total is refused, the choice and the rest are bound, and another effect is refused.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choice, check_cards};
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);
    const WITHIN: &str = r#"{"ChooseWithin":{"who":"EachPlayer","from":{"Each":[{"Type":"Creature"},{"ControlledBy":"Chooser"}]},"within":{"TotalPowerAtMost":4},"as":"kept","rest_as":"rest"}}"#;

    /// Seat 0 controls the Source (1/1), a 3/3 and a 2/2; seat 1 a 4/4 and a 0/1.
    fn game() -> GameState {
        GameState::from_setup(&setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Three", 3, 3),
                    creature("Two", 2, 2),
                ],
                vec![creature("Four", 4, 4), creature("Zero", 0, 1)],
            ],
        ))
        .unwrap()
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects().find(|o| o.card().name == name).unwrap().id()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[test]
    fn the_first_chooser_is_asked_with_the_values_of_their_own_creatures() {
        let game = game();
        let Run::Ask(decision) = run_with(&game, Vec::new(), WITHIN).unwrap() else {
            panic!("asked");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards");
        };
        assert_eq!((options.min, options.max), (0, 3));
        let total = options.total.clone().unwrap();
        assert_eq!((total.most, total.values), (4, vec![1, 3, 2]));
        assert!(check_cards(&options, &[id(&game, "Three"), id(&game, "Source")]).is_ok());
        assert!(check_cards(&options, &[id(&game, "Three"), id(&game, "Two")]).is_err());
    }

    #[rstest]
    #[case::three_and_one(&["Three", "Source"], &["Four"], &["Two", "Zero"])]
    #[case::nothing_kept(&[], &["Four", "Zero"], &["Source", "Three", "Two"])]
    fn the_choices_are_bound_together_and_the_rest_apart(
        #[case] mine: &[&str],
        #[case] theirs: &[&str],
        #[case] rest: &[&str],
    ) {
        let game = game();
        let pick = |names: &[&str]| names.iter().map(|name| id(&game, name)).collect::<Vec<_>>();
        let journal = vec![
            (ME, Choice::Cards(pick(mine))),
            (FOE, Choice::Cards(pick(theirs))),
        ];
        let Run::Done { bound, .. } = run_with(&game, journal, WITHIN).unwrap() else {
            panic!("done");
        };
        let names = |targets: &[Target]| {
            let mut names: Vec<String> = targets
                .iter()
                .filter_map(|t| t.object())
                .map(|o| game.object(o.id).unwrap().card().name.clone())
                .collect();
            names.sort();
            names
        };
        let mut kept: Vec<String> = mine.iter().chain(theirs).map(|n| (*n).to_owned()).collect();
        kept.sort();
        let mut others: Vec<String> = rest.iter().map(|n| (*n).to_owned()).collect();
        others.sort();
        assert_eq!(names(&bound[0].1), kept);
        assert_eq!(names(&bound[1].1), others);
    }

    #[test]
    fn mana_values_are_the_values_of_a_mana_value_total() {
        let game = game();
        let json = WITHIN.replace(r#"{"TotalPowerAtMost":4}"#, r#"{"TotalManaValueAtMost":1}"#);
        let Run::Ask(decision) = run_with(&game, Vec::new(), &json).unwrap() else {
            panic!("asked");
        };
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards");
        };
        let total = options.total.unwrap();
        assert_eq!(total.most, 1);
        assert_eq!(total.values.len(), 3);
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game();
        assert!(run_with(&game, Vec::new(), r#"{"Draw":{}}"#).is_err());
    }
}
