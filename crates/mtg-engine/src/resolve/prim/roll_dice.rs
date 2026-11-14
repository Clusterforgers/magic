//! `Effect::RollDice`: a player rolls a die (CR 706) and the resolution goes on with the row of the table the total is in.

use mtg_cards::effect::{DieRow, Effect};

use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;

/// Whether the total is in the range of the row.
fn holds(row: &DieRow, total: i64) -> bool {
    total >= i64::from(row.from) && row.to.is_none_or(|to| total <= i64::from(to))
}

/// The different numbers the dice rolled, in the order rolled.
fn c11e_distinct(rolled: &[u32]) -> Vec<u32> {
    let mut distinct: Vec<u32> = Vec::with_capacity(rolled.len());
    for result in rolled {
        if !distinct.contains(result) {
            distinct.push(*result);
        }
    }
    distinct
}

/// The roller chooses one of the numbers rolled ("choose one result"); dice that all rolled the same ask nothing.
fn c11e_chosen_last(
    cx: &Cx<'_>,
    player: PlayerId,
    rolled: &[u32],
) -> Result<Optioned, ResolveError> {
    let options = c11e_distinct(rolled)
        .into_iter()
        .map(|result| ChoiceOption::Label(result.to_string()))
        .collect();
    pick_options(
        cx,
        &[OptionChooser {
            player,
            options,
            text: "Choose one result.".to_owned(),
        }],
    )
}

/// "Roll that many dice plus one and ignore the lowest roll" (`GameRule::ExtraDie`), once for each such rule: `dice` plus
/// `extra` dice come one after another from the next substream of the game's random stream, the `extra` lowest are
/// ignored and the others are kept in the order rolled. Only the kept dice are logged, as the rolls whose results count;
/// they spend the substream the extra dice came from.
fn ep29_with_extra(state: &crate::state::GameState, sides: u32, dice: u32, extra: u32) -> Vec<u32> {
    let mut rng = GameRng::substream(state.seed(), state.rng_index());
    let all: Vec<u32> = (0..dice.saturating_add(extra))
        .map(|_| u32::try_from(rng.below(u64::from(sides))).unwrap_or(0) + 1)
        .collect();
    let mut order: Vec<usize> = (0..all.len()).collect();
    // The lowest first, the earlier of two equal rolls first: those are the ones ignored.
    order.sort_by_key(|at| (all[*at], *at));
    let ignored: Vec<usize> = order
        .into_iter()
        .take(usize::try_from(extra).unwrap_or(0))
        .collect();
    all.iter()
        .enumerate()
        .filter(|(at, _)| !ignored.contains(at))
        .map(|(_, result)| *result)
        .collect()
}

/// "Ignore all but the highest roll": the highest number rolled (the earliest of equal ones) moves last, where the total
/// is read.
fn r5518_highest_last(rolled: &mut Vec<u32>) {
    let at = rolled
        .iter()
        .enumerate()
        .max_by_key(|(at, result)| (**result, std::cmp::Reverse(*at)))
        .map(|(at, _)| at);
    if let Some(at) = at {
        let kept = rolled.remove(at);
        rolled.push(kept);
    }
}

/// The dice the player rolls, in the order rolled: `dice` dice of `sides` faces, one substream of the game's random
/// stream each, or with the extra dice of `GameRule::ExtraDie` and the lowest ignored.
pub(crate) fn rolls(
    state: &crate::state::GameState,
    player: PlayerId,
    sides: u32,
    dice: u32,
) -> Vec<u32> {
    let extra = crate::rules::ep29_extra_dice(state, player);
    if extra == 0 {
        (0..dice)
            .map(|at| {
                let below = GameRng::substream(state.seed(), state.rng_index() + u64::from(at))
                    .below(u64::from(sides));
                u32::try_from(below).unwrap_or(0) + 1
            })
            .collect()
    } else {
        ep29_with_extra(state, sides, dice, extra)
    }
}

/// The name a number of a roll of several dice is kept under (`DieResult::Odd`, `Even`, `Total`, `Times`).
pub(crate) fn r5507_kept(which: mtg_cards::value::DieResult) -> String {
    use mtg_cards::value::DieResult;
    match which {
        DieResult::Odd => "~dice:odd".to_owned(),
        DieResult::Even => "~dice:even".to_owned(),
        DieResult::Total => "~dice:total".to_owned(),
        DieResult::Times(result) => format!("~dice:times:{result}"),
        DieResult::Only | DieResult::Chosen | DieResult::Other | DieResult::Distinct => {
            "~dice:one".to_owned()
        }
    }
}

/// "Roll X six-sided dice": the first player the effect names who is still in the game rolls the number of dice the
/// amount works out to, each logged as `DieRolled`; how many came up odd and even, their total and how many rolled each
/// number are kept for the effects after it, fresh for this roll (zero when no die is rolled).
fn r5507_roll_many(
    cx: &Cx<'_>,
    who: &mtg_cards::refs::Ref,
    sides: u32,
    count: &mtg_cards::value::Amount,
) -> Result<Run, ResolveError> {
    use mtg_cards::value::DieResult;
    let state = cx.state();
    let player = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player));
    let dice = u32::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
    let rolled = player.map_or_else(Vec::new, |player| rolls(state, player, sides, dice));
    let odd = rolled.iter().filter(|result| *result % 2 == 1).count();
    let total: u64 = rolled.iter().map(|result| u64::from(*result)).sum();
    let mut kept = vec![
        (DieResult::Odd, i64::try_from(odd).unwrap_or(0)),
        (
            DieResult::Even,
            i64::try_from(rolled.len() - odd).unwrap_or(0),
        ),
        (DieResult::Total, i64::try_from(total).unwrap_or(0)),
    ];
    for face in 1..=sides {
        let times = rolled.iter().filter(|result| **result == face).count();
        kept.push((DieResult::Times(face), i64::try_from(times).unwrap_or(0)));
    }
    let bound = kept
        .into_iter()
        .map(|(which, value)| {
            (
                format!("{}{value}:{}", crate::resolve::LET, r5507_kept(which)),
                Vec::new(),
            )
        })
        .collect();
    Ok(Run::Done {
        events: player.map_or_else(Vec::new, |player| {
            rolled
                .iter()
                .map(|result| Event::DieRolled {
                    player,
                    sides,
                    result: *result,
                })
                .collect()
        }),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The first player the effect names who is still in the game rolls the die: the next substream of the game's random
/// stream gives a number from 1 to the sides, logged as `DieRolled`, and the step goes on into the first row that holds
/// that number plus `plus`, past the effect when no row does.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RollDice {
        who,
        sides,
        dice,
        choose,
        plus,
        highest,
        results,
        count,
    } = effect
    else {
        return Err(wrong_effect("RollDice"));
    };
    if let Some(count) = count {
        return r5507_roll_many(cx, who, *sides, count);
    }
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let dice = match highest {
        Some(count) => u32::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0),
        None => *dice,
    };
    let mut rolled = rolls(state, player, *sides, dice);
    if highest.is_some() {
        if rolled.is_empty() {
            return Ok(Run::events(Vec::new()));
        }
        r5518_highest_last(&mut rolled);
    }
    if *choose {
        match c11e_chosen_last(cx, player, &rolled)? {
            Optioned::Done(taken) => {
                let distinct = c11e_distinct(&rolled);
                let chosen = taken
                    .first()
                    .and_then(|(_, at)| distinct.get(*at))
                    .copied()
                    .unwrap_or(0);
                if let Some(at) = rolled.iter().position(|result| *result == chosen) {
                    let kept = rolled.remove(at);
                    rolled.push(kept);
                }
            }
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        }
    }
    let result = rolled.last().copied().unwrap_or(0);
    let total = i64::from(result) + cx.scope.amount(plus)?;
    Ok(Run::Done {
        events: rolled
            .iter()
            .map(|result| Event::DieRolled {
                player,
                sides: *sides,
                result: *result,
            })
            .collect(),
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: results
            .iter()
            .position(|row| holds(row, total))
            .map_or(Next::Sibling, Next::Enter),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a roll is a number from 1 to the sides from the game's random stream (the same seed gives the same number and the seeds give every face), logged for the player who rolled, and the step enters the first row of the table that holds the number plus the amount added, none when no row does; two dice with a choice ask between the different numbers and log the chosen one last; a player who has left the game rolls nothing; one die for each of an amount keeps only the highest (none for 0); and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game(seed: u64) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        game
    }

    fn roll(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    /// A d6 whose table has three rows: 1 to 2, 3 to 4 and 5 and up.
    fn d6(plus: i32) -> String {
        format!(
            r#"{{"RollDice":{{"sides":6,"plus":{plus},"results":[{{"from":1,"to":2,"effects":[{{"Draw":{{}}}}]}},{{"from":3,"to":4,"effects":[{{"Draw":{{}}}}]}},{{"from":5,"effects":[{{"Draw":{{}}}}]}}]}}}}"#
        )
    }

    /// The number rolled and the row entered by a roll of `json` in the game of `seed`.
    fn rolled(seed: u64, json: &str) -> (u32, Next) {
        let Run::Done { events, next, .. } = roll(&game(seed), json).unwrap() else {
            panic!("a roll asks nothing");
        };
        let [
            Event::DieRolled {
                player,
                sides,
                result,
            },
        ] = events.as_slice()
        else {
            panic!("one roll is logged: {events:?}");
        };
        assert_eq!((*player, *sides), (ME, 6));
        (*result, next)
    }

    #[test]
    fn the_same_seed_rolls_the_same_and_the_seeds_give_every_face() {
        let faces: Vec<u32> = (0..64).map(|seed| rolled(seed, &d6(0)).0).collect();
        assert_eq!(
            faces,
            (0..64)
                .map(|seed| rolled(seed, &d6(0)).0)
                .collect::<Vec<_>>()
        );
        for face in 1..=6 {
            assert!(faces.contains(&face), "no seed rolls a {face}");
        }
        assert!(faces.iter().all(|face| (1..=6).contains(face)));
    }

    #[rstest]
    #[case::no_amount_added(0)]
    #[case::one_added(1)]
    #[case::enough_added_to_leave_the_table(10)]
    #[case::taken_off(-3)]
    fn the_row_is_the_first_that_holds_the_number_with_the_amount_added(#[case] plus: i32) {
        for seed in 0..32 {
            let (result, next) = rolled(seed, &d6(plus));
            let total = i64::from(result) + i64::from(plus);
            let expected = match total {
                ..=0 => Next::Sibling,
                1..=2 => Next::Enter(0),
                3..=4 => Next::Enter(1),
                _ => Next::Enter(2),
            };
            assert_eq!(
                next, expected,
                "seed {seed}: rolled {result}, total {total}"
            );
        }
    }

    #[test]
    fn a_player_who_has_left_rolls_nothing() {
        let mut game = game(1);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let json = d6(0).replace(r#""sides":6"#, r#""who":"Opponents","sides":6"#);
        let Run::Done { events, next, .. } = roll(&game, &json).unwrap() else {
            panic!("a roll asks nothing");
        };
        assert_eq!(events.len(), 0);
        assert_eq!(next, Next::Sibling);
    }

    const C11E_TWO_D6: &str = r#"{"RollDice":{"sides":6,"dice":2,"choose":true,"results":[]}}"#;

    /// Two d6 with a choice in the game of `seed`, with the answers given so far.
    fn c11e_choose(seed: u64, journal: Vec<(PlayerId, crate::choose::Choice)>) -> Run {
        let game = game(seed);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(C11E_TWO_D6).unwrap()).unwrap()
    }

    fn c11e_results(run: &Run) -> Vec<u32> {
        let Run::Done { events, .. } = run else {
            panic!("the roll asks: {run:?}");
        };
        events
            .iter()
            .map(|event| match event {
                Event::DieRolled { result, .. } => *result,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::the_first_option(0)]
    #[case::the_second_option(1)]
    fn a_choice_of_one_result_asks_between_different_numbers_and_logs_the_chosen_last(
        #[case] answer: usize,
    ) {
        for seed in 0..32 {
            let Run::Ask(decision) = c11e_choose(seed, Vec::new()) else {
                // Both dice rolled the same: nothing to choose, and both are logged.
                let rolled = c11e_results(&c11e_choose(seed, Vec::new()));
                assert_eq!((rolled.len(), rolled[0]), (2, rolled[1]), "seed {seed}");
                continue;
            };
            let crate::decision::DecisionKind::ChooseOption(options) = decision.kind else {
                panic!("a choice of option: {decision:?}");
            };
            assert_eq!(options.options.len(), 2, "seed {seed}");
            let rolled = c11e_results(&c11e_choose(
                seed,
                vec![(ME, crate::choose::Choice::Option(answer))],
            ));
            assert_eq!(
                options.options[answer],
                ChoiceOption::Label(rolled[1].to_string()),
                "seed {seed}: the chosen result is logged last"
            );
            assert_ne!(rolled[0], rolled[1]);
        }
    }

    #[rstest]
    #[case::none(0)]
    #[case::one(1)]
    #[case::three(3)]
    #[case::four(4)]
    fn r5518_one_die_for_each_and_only_the_highest_counts(#[case] count: u32) {
        let json = d6(0).replace(r#""sides":6"#, &format!(r#""sides":6,"highest":{count}"#));
        for seed in 0..32 {
            let Run::Done { events, next, .. } = roll(&game(seed), &json).unwrap() else {
                panic!("a roll asks nothing");
            };
            let rolled: Vec<u32> = events
                .iter()
                .map(|event| match event {
                    Event::DieRolled { result, .. } => *result,
                    other => panic!("{other:?}"),
                })
                .collect();
            assert_eq!(rolled.len(), count as usize, "seed {seed}");
            let Some(highest) = rolled.iter().max() else {
                assert_eq!(next, Next::Sibling);
                continue;
            };
            assert_eq!(
                rolled.last(),
                Some(highest),
                "seed {seed}: the highest is read"
            );
            let expected = match highest {
                1..=2 => Next::Enter(0),
                3..=4 => Next::Enter(1),
                _ => Next::Enter(2),
            };
            assert_eq!(next, expected, "seed {seed}: {rolled:?}");
        }
    }

    /// The rolls logged and the numbers kept by a roll of `dice` d6 in the game of `seed`.
    fn r5507_many(seed: u64, dice: u32) -> (Vec<u32>, Vec<(String, i64)>) {
        let json = format!(r#"{{"RollDice":{{"sides":6,"results":[],"count":{dice}}}}}"#);
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = roll(&game(seed), &json).unwrap()
        else {
            panic!("a roll asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        let rolled = events
            .iter()
            .map(|event| match event {
                Event::DieRolled {
                    result,
                    sides: 6,
                    player: ME,
                } => *result,
                other => panic!("{other:?}"),
            })
            .collect();
        let kept = bound
            .into_iter()
            .map(|(name, _)| {
                let (value, kept) = name
                    .strip_prefix(crate::resolve::LET)
                    .unwrap()
                    .split_once(':')
                    .unwrap();
                (kept.to_owned(), value.parse().unwrap())
            })
            .collect();
        (rolled, kept)
    }

    #[rstest]
    #[case::none(0)]
    #[case::one(1)]
    #[case::five(5)]
    #[case::twelve(12)]
    fn r5507_a_number_of_dice_keeps_odd_even_total_and_each_face(#[case] dice: u32) {
        use mtg_cards::value::DieResult;
        for seed in 0..16 {
            let (rolled, kept) = r5507_many(seed, dice);
            assert_eq!(rolled.len(), usize::try_from(dice).unwrap());
            let value = |which| {
                kept.iter()
                    .find(|(name, _)| *name == r5507_kept(which))
                    .map(|(_, value)| *value)
                    .unwrap()
            };
            let odd = rolled.iter().filter(|r| *r % 2 == 1).count();
            assert_eq!(value(DieResult::Odd), i64::try_from(odd).unwrap());
            assert_eq!(
                value(DieResult::Even),
                i64::try_from(rolled.len() - odd).unwrap()
            );
            assert_eq!(
                value(DieResult::Total),
                rolled.iter().map(|r| i64::from(*r)).sum::<i64>()
            );
            for face in 1..=6 {
                let times = rolled.iter().filter(|r| **r == face).count();
                assert_eq!(value(DieResult::Times(face)), i64::try_from(times).unwrap());
            }
        }
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = roll(&game(1), r#"{"Draw":{}}"#).unwrap_err();
        assert!(error.to_string().contains("RollDice"), "{error}");
    }
}
