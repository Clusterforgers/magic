//! `Effect::ChooseNumbers`: each player chooses a number, the controller first and the others in turn order (face down
//! when secret), and the resolution goes on with the highest number and the players at either end bound to names.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Choice, NumberOptions, Target};
use crate::decision::{Decision, DecisionKind};
use crate::object::Chosen;
use crate::resolve::{CHOSEN_VALUE, Cx, Next, ResolveError, Run};

/// The numbers chosen so far, in the order of `players`, or the question to the next player who has not chosen.
fn chosen(
    cx: &Cx<'_>,
    players: &[PlayerId],
    min: u32,
    secret: bool,
) -> Result<Result<Vec<u32>, Decision>, ResolveError> {
    let journal = cx.journal();
    let mut numbers = Vec::new();
    for (at, player) in players.iter().enumerate() {
        match journal.get(at) {
            Some((who, Choice::Number(number) | Choice::SecretNumber(number)))
                if who == player && *number >= min =>
            {
                numbers.push(*number);
            }
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no number of {min} or more chosen by {player:?}"
                )));
            }
            None => {
                let text = if secret {
                    format!("Secretly choose a number {min} or greater.")
                } else {
                    format!("Choose a number {min} or greater.")
                };
                return Ok(Err(Decision {
                    player: *player,
                    kind: DecisionKind::ChooseNumber(NumberOptions {
                        source: cx.scope.source.id,
                        min,
                        max: None,
                        except: Vec::new(),
                        text,
                        can_cancel: false,
                        cost: None,
                        per_x: 0,
                        secret,
                    }),
                }));
            }
        }
    }
    Ok(Ok(numbers))
}

/// The players who chose `number`, in the order they chose.
fn who_chose(players: &[PlayerId], numbers: &[u32], number: u32) -> Vec<Target> {
    players
        .iter()
        .zip(numbers)
        .filter(|(_, chose)| **chose == number)
        .map(|(player, _)| Target::Player(*player))
        .collect()
}

/// Each player in the game that `who` names chooses a number; when all have, the highest is remembered and the players
/// at the highest and the lowest are bound. Nobody to choose binds nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChooseNumbers {
        who,
        min,
        secret,
        bind,
        highest,
        lowest,
    } = effect
    else {
        return Err(wrong_effect("ChooseNumbers"));
    };
    let named = cx.scope.players(who)?;
    let players: Vec<PlayerId> = cx
        .state()
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .filter(|player| named.contains(player) && cx.state().is_alive(*player))
        .collect();
    let min = u32::try_from(cx.scope.amount(min)?.max(0)).unwrap_or(u32::MAX);
    let numbers = match chosen(cx, &players, min, *secret)? {
        Ok(numbers) => numbers,
        Err(decision) => return Ok(Run::Ask(decision)),
    };
    let mut bound = Vec::new();
    if let (Some(most), Some(least)) = (numbers.iter().max(), numbers.iter().min()) {
        if let Ok(json) = serde_json::to_string(&Chosen::Number(*most)) {
            bound.push((format!("{CHOSEN_VALUE}{bind}\t{json}"), Vec::new()));
        }
        bound.push((highest.clone(), who_chose(&players, &numbers, *most)));
        bound.push((lowest.clone(), who_chose(&players, &numbers, *least)));
        // The numbers are revealed together: the finished step, which every seat reads, names each player's number.
        for (player, number) in players.iter().zip(&numbers) {
            if let Ok(json) = serde_json::to_string(&Chosen::Number(*number)) {
                bound.push((
                    format!("{CHOSEN_VALUE}{bind}.{}\t{json}", player.index()),
                    vec![Target::Player(*player)],
                ));
            }
        }
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the numbers are asked of the controller first and the others in turn order, face down when secret and from the least allowed up with no most, a number below the least or of another player is refused, and the highest number, the players at each end (all of them when tied) and each player's number are bound.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const WHEEL: &str =
        r#"{"ChooseNumbers":{"secret":true,"as":"n","highest":"high","lowest":"low"}}"#;

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn play(
        game: &GameState,
        controller: u64,
        effect: &str,
        journal: &[(u64, Choice)],
    ) -> Result<Run, ResolveError> {
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving {
            journal: journal
                .iter()
                .map(|(player, choice)| (PlayerId(*player), choice.clone()))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(controller), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(effect).unwrap())
    }

    #[rstest]
    #[case::the_controller_first(3, 1, &[], 1)]
    #[case::then_the_next_seat(3, 1, &[(1, Choice::SecretNumber(4))], 2)]
    #[case::around_the_table(3, 1, &[(1, Choice::SecretNumber(4)), (2, Choice::SecretNumber(0))], 0)]
    fn the_numbers_are_asked_in_turn_order_from_the_controller(
        #[case] seats: usize,
        #[case] controller: u64,
        #[case] journal: &[(u64, Choice)],
        #[case] next: u64,
    ) {
        let Run::Ask(decision) = play(&game(seats), controller, WHEEL, journal).unwrap() else {
            panic!("a number is asked");
        };
        assert_eq!(decision.player, PlayerId(next));
        let DecisionKind::ChooseNumber(options) = decision.kind else {
            panic!("a number");
        };
        assert_eq!((options.min, options.max, options.secret), (0, None, true));
    }

    #[rstest]
    #[case::open(
        r#"{"ChooseNumbers":{"min":1,"as":"n","highest":"high","lowest":"low"}}"#,
        1,
        false
    )]
    #[case::secret(WHEEL, 0, true)]
    fn the_least_and_the_secrecy_are_asked(
        #[case] json: &str,
        #[case] min: u32,
        #[case] secret: bool,
    ) {
        let Run::Ask(decision) = play(&game(2), 0, json, &[]).unwrap() else {
            panic!("a number is asked");
        };
        let DecisionKind::ChooseNumber(options) = decision.kind else {
            panic!("a number");
        };
        assert_eq!((options.min, options.secret), (min, secret));
    }

    #[rstest]
    #[case::one_at_each_end(&[3, 7, 1], 7, &[1], &[2])]
    #[case::a_tie_at_the_top(&[7, 7, 1], 7, &[0, 1], &[2])]
    #[case::all_the_same(&[2, 2, 2], 2, &[0, 1, 2], &[0, 1, 2])]
    fn the_highest_number_and_the_players_at_each_end_are_bound(
        #[case] numbers: &[u32],
        #[case] most: u32,
        #[case] high: &[u64],
        #[case] low: &[u64],
    ) {
        let journal: Vec<(u64, Choice)> = numbers
            .iter()
            .enumerate()
            .map(|(seat, n)| (u64::try_from(seat).unwrap(), Choice::SecretNumber(*n)))
            .collect();
        let Run::Done { bound, .. } = play(&game(3), 0, WHEEL, &journal).unwrap() else {
            panic!("all have chosen");
        };
        let players = |seats: &[u64]| -> Vec<Target> {
            seats
                .iter()
                .map(|seat| Target::Player(PlayerId(*seat)))
                .collect()
        };
        let value = format!(
            "{CHOSEN_VALUE}n\t{}",
            serde_json::to_string(&Chosen::Number(most)).unwrap()
        );
        assert_eq!(
            bound[..3],
            [
                (value, Vec::new()),
                ("high".to_owned(), players(high)),
                ("low".to_owned(), players(low)),
            ]
        );
        // Each number is revealed with its player.
        let revealed: Vec<(String, Vec<Target>)> = numbers
            .iter()
            .enumerate()
            .map(|(seat, n)| {
                let json = serde_json::to_string(&Chosen::Number(*n)).unwrap();
                (
                    format!("{CHOSEN_VALUE}n.{seat}\t{json}"),
                    players(&[u64::try_from(seat).unwrap()]),
                )
            })
            .collect();
        assert_eq!(bound[3..], revealed);
    }

    #[rstest]
    #[case::below_the_least(
        r#"{"ChooseNumbers":{"min":1,"as":"n","highest":"high","lowest":"low"}}"#,
        &[(0, Choice::Number(0))]
    )]
    #[case::another_player(WHEEL, &[(1, Choice::SecretNumber(2))])]
    #[case::another_kind(WHEEL, &[(0, Choice::YesNo(true))])]
    fn a_wrong_answer_is_refused(#[case] json: &str, #[case] journal: &[(u64, Choice)]) {
        assert!(matches!(
            play(&game(2), 0, json, journal),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(2), 0, r#"{"Draw":{}}"#, &[]),
            Err(ResolveError::Invalid(message)) if message.contains("ChooseNumbers")
        ));
    }
}
