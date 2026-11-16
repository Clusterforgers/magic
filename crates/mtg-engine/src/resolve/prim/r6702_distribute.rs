//! `Effect::R6702Distribute`: the controller distributes counters among any number of permanents as the effect resolves
//! ("distribute three stun counters among any number of tapped creatures your opponents control").

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::{CardsOptions, Choice, NumberOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The controller chooses between one and as many permanents as there are counters among those the reference names on
/// the battlefield, then how many each of them but the last gets (at least one each, leaving at least one for each
/// after it); the last takes the rest. Every permanent gets its share at once. No counter, or no permanent, is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6702Distribute {
        among,
        counter,
        count,
    } = effect
    else {
        return Err(wrong_effect("R6702Distribute"));
    };
    let total = whole(cx.scope.amount(count)?);
    let from: Vec<ObjectId> = cx
        .scope
        .live_objects(among)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(|object| object.id())
        .collect();
    let total = u32::try_from(total).unwrap_or(0);
    if total == 0 || from.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let player = cx.controller();
    let most = usize::try_from(total).unwrap_or(usize::MAX).min(from.len());
    let mut answers = cx.journal().iter();
    // A lone permanent takes every counter unasked.
    let chosen: Vec<ObjectId> = if from.len() == 1 {
        from.clone()
    } else {
        match answers.next() {
            Some((who, Choice::Cards(cards)))
                if *who == player
                    && (1..=most).contains(&cards.len())
                    && cards.iter().all(|card| from.contains(card))
                    && cards
                        .iter()
                        .enumerate()
                        .all(|(at, card)| !cards[..at].contains(card)) =>
            {
                cards.clone()
            }
            Some(_) => {
                return Err(ResolveError::Invalid(
                    "not a choice of permanents to distribute among".into(),
                ));
            }
            None => {
                return Ok(Run::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseCards(CardsOptions {
                        one_per_name: false,
                        source: cx.scope.source.id,
                        from,
                        min: 1,
                        max: u32::try_from(most).unwrap_or(u32::MAX),
                        text: format!(
                            "Choose the permanents to distribute {total} counters among."
                        ),
                        can_cancel: false,
                        total: None,
                        groups: Vec::new(),
                    }),
                }));
            }
        }
    };
    let mut numbers = answers;
    let mut shares: Vec<u32> = Vec::with_capacity(chosen.len());
    let mut left = total;
    for (at, object) in chosen.iter().enumerate() {
        let after = u32::try_from(chosen.len() - at - 1).unwrap_or(u32::MAX);
        if after == 0 {
            shares.push(left);
            break;
        }
        let max = left.saturating_sub(after);
        let share = if max <= 1 {
            1
        } else {
            match numbers.next() {
                Some((who, Choice::Number(share)))
                    if *who == player && (1..=max).contains(share) =>
                {
                    *share
                }
                Some(_) => return Err(ResolveError::Invalid("not a share of the counters".into())),
                None => {
                    let name = cx
                        .state()
                        .object(*object)
                        .map_or_else(String::new, |o| o.card().name.clone());
                    return Ok(Run::Ask(Decision {
                        player,
                        kind: DecisionKind::ChooseNumber(NumberOptions {
                            source: cx.scope.source.id,
                            min: 1,
                            max: Some(max),
                            except: Vec::new(),
                            text: format!("How many of the {left} counters left go on {name}?"),
                            can_cancel: false,
                            cost: None,
                            per_x: 0,
                            secret: false,
                        }),
                    }));
                }
            }
        };
        shares.push(share);
        left -= share;
    }
    let events = chosen
        .iter()
        .zip(shares)
        .filter_map(|(object, share)| {
            let reference = cx.state().object(*object)?.reference();
            Some(Event::CountersChanged(CountersChanged {
                on: Target::Object(reference),
                counter: counter.clone(),
                delta: i32::try_from(share).ok()?,
            }))
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: counters are distributed as the effect resolves: the controller picks the permanents (none asked for a lone
    //! one), then the share of each but the last, at least one each; the last takes the rest, and nothing is put when
    //! nothing can hold them.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const DISTRIBUTE: &str = r#"{"R6702Distribute":{"among":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"counter":{"Other":"stun"},"count":3}}"#;

    fn game(theirs: usize) -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![creature("Source", 1, 1)],
                (0..theirs)
                    .map(|at| creature(&format!("Foe {at}"), 2, 2))
                    .collect(),
            ],
        )
    }

    fn play(game: &GameState, journal: Vec<(PlayerId, Choice)>) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(DISTRIBUTE).unwrap())
    }

    fn shares(run: Run) -> Vec<i32> {
        let Run::Done { events, .. } = run else {
            panic!("the distribution is done");
        };
        events
            .into_iter()
            .map(|event| match event {
                Event::CountersChanged(changed) => changed.delta,
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_lone_permanent_takes_every_counter_unasked() {
        assert_eq!(shares(play(&game(1), Vec::new()).unwrap()), [3]);
    }

    #[test]
    fn nothing_to_hold_them_is_no_event() {
        assert!(shares(play(&game(0), Vec::new()).unwrap()).is_empty());
    }

    #[rstest]
    #[case::two_then_one(2, Some(2), &[2, 1])]
    #[case::one_then_two(2, Some(1), &[1, 2])]
    #[case::one_each_unasked(3, None, &[1, 1, 1])]
    fn the_shares_are_asked_for_all_but_the_last(
        #[case] chosen: usize,
        #[case] first: Option<u32>,
        #[case] want: &[i32],
    ) {
        let game = game(3);
        let foes: Vec<ObjectId> = game.zone(Zone::Battlefield)[1..=chosen].to_vec();
        let mut journal = vec![(PlayerId(0), Choice::Cards(foes))];
        let asked = play(&game, journal.clone()).unwrap();
        match first {
            Some(share) => {
                assert!(matches!(asked, Run::Ask(_)), "the first share is asked");
                journal.push((PlayerId(0), Choice::Number(share)));
                assert_eq!(shares(play(&game, journal).unwrap()), want);
            }
            None => assert_eq!(shares(asked), want),
        }
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game(1);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        assert!(run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).is_err());
    }
}
