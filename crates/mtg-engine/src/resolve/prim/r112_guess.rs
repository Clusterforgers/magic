//! `Effect::R112GuessNumber`: the controller secretly chooses a number its source has not chosen before, another player
//! guesses it, and the number is revealed (The Toymaker's Trap).

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::may::performed_name;
use super::wrong_effect;
use crate::choose::{Choice, ChoiceOption, NumberOptions, Target};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::memory::{Held, Remembered};
use crate::object::Chosen;
use crate::resolve::{CHOSEN_VALUE, Cx, Next, ResolveError, Run};

/// The questions of the step in the order asked: the guesser (when several may guess), the secret number, the guess.
struct Walk {
    guesser: Option<PlayerId>,
    number: u32,
    guess: Option<u32>,
}

/// A question for `player` of a number from `min` to `max` without `except`.
fn ask_number(
    cx: &Cx<'_>,
    player: PlayerId,
    (min, max): (u32, u32),
    except: Vec<u32>,
    secret: bool,
) -> Decision {
    let text = if secret {
        format!("Secretly choose a number from {min} to {max}.")
    } else {
        format!("Guess the number chosen, from {min} to {max}.")
    };
    Decision {
        player,
        kind: DecisionKind::ChooseNumber(NumberOptions {
            source: cx.scope.source.id,
            min,
            max: Some(max),
            except,
            text,
            can_cancel: false,
            cost: None,
            per_x: 0,
            secret,
        }),
    }
}

/// Reads the answers in the journal, or the next question.
fn walk(
    cx: &Cx<'_>,
    guessers: &[PlayerId],
    range: (u32, u32),
    used: &[u32],
) -> Result<Result<Walk, Decision>, ResolveError> {
    let controller = cx.scope.controller;
    let mut answers = cx.journal().iter();
    let unfit =
        |what: &str| ResolveError::Invalid(format!("no answer to the {what} of a guessed number"));
    let guesser = match guessers {
        [] => None,
        [one] => Some(*one),
        several => match answers.next() {
            None => {
                return Ok(Err(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseOption(crate::choose::OptionOptions {
                        can_cancel: false,
                        source: cx.scope.source.id,
                        options: several.iter().copied().map(ChoiceOption::Player).collect(),
                        text: "Choose an opponent to guess.".to_owned(),
                    }),
                }));
            }
            Some((who, Choice::Option(at))) if *who == controller && *at < several.len() => {
                Some(several[*at])
            }
            Some(_) => return Err(unfit("guesser")),
        },
    };
    let number = match answers.next() {
        None => return Ok(Err(ask_number(cx, controller, range, used.to_vec(), true))),
        Some((who, Choice::SecretNumber(n) | Choice::Number(n)))
            if *who == controller && (range.0..=range.1).contains(n) && !used.contains(n) =>
        {
            *n
        }
        Some(_) => return Err(unfit("secret number")),
    };
    let guess = match guesser {
        None => None,
        Some(guesser) => match answers.next() {
            None => return Ok(Err(ask_number(cx, guesser, range, Vec::new(), false))),
            Some((who, Choice::Number(n)))
                if *who == guesser && (range.0..=range.1).contains(n) =>
            {
                Some(*n)
            }
            Some(_) => return Err(unfit("guess")),
        },
    };
    if answers.next().is_some() {
        return Err(unfit("questions"));
    }
    Ok(Ok(Walk {
        guesser,
        number,
        guess,
    }))
}

/// The value bound under `name` for `Amount::ChosenAs`.
fn value(name: &str, number: u32) -> Option<(String, Vec<Target>)> {
    let json = serde_json::to_string(&Chosen::Number(number)).ok()?;
    Some((format!("{CHOSEN_VALUE}{name}\t{json}"), Vec::new()))
}

/// The controller chooses a number the source has not chosen (none left binds nothing), a guesser guesses it, and the
/// number joins the source's memory; the number, the guess, the guesser and "if you do" are bound.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112GuessNumber {
        min,
        max,
        guesser,
        bind,
        guess_as,
        guesser_as,
    } = effect
    else {
        return Err(wrong_effect("R112GuessNumber"));
    };
    let state = cx.state();
    let source = cx.scope.source;
    let used: Vec<u32> =
        crate::memory::recall_numbers(state, source, &crate::memory::r112_lasting(bind))
            .into_iter()
            .filter_map(|n| u32::try_from(n).ok())
            .collect();
    let open = (*min..=*max).any(|n| !used.contains(&n));
    if !open {
        return Ok(Run::Done {
            events: Vec::new(),
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Sibling,
        });
    }
    let named = cx.scope.players(guesser)?;
    let guessers: Vec<PlayerId> = state
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .filter(|player| named.contains(player) && state.is_alive(*player))
        .collect();
    let walked = match walk(cx, &guessers, (*min, *max), &used)? {
        Ok(walked) => walked,
        Err(decision) => return Ok(Run::Ask(decision)),
    };
    let mut bound: Vec<(String, Vec<Target>)> = value(bind, walked.number).into_iter().collect();
    if let Some(guess) = walked.guess {
        bound.extend(value(guess_as, guess));
    }
    bound.push((
        guesser_as.clone(),
        walked.guesser.map(Target::Player).into_iter().collect(),
    ));
    bound.push((
        performed_name(cx.resolving.position()),
        vec![Target::Player(cx.scope.controller)],
    ));
    let remembered = Event::Remembered(Box::new(Remembered {
        source,
        slot: crate::memory::r112_lasting(bind),
        objects: Vec::new(),
        values: vec![Held::Number(i64::from(walked.number))],
        replace: false,
    }));
    let mut events = vec![remembered];
    if walked.guess.is_some() {
        events.push(Event::R112Grouped {
            object: cx.resolving.object,
        });
    }
    Ok(Run::Done {
        events,
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the number is asked secretly of the controller without the numbers the source chose before, the guess of
    //! the guesser after it, a number out of range or chosen before is refused, and with every number chosen nothing is
    //! asked or bound.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;
    use rstest::rstest;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);
    const TRAP: &str = r#"{"R112GuessNumber":{"min":1,"max":5,"guesser":"Opponents","as":"number","guess_as":"guess","guesser_as":"guesser"}}"#;

    fn game(used: &[i64]) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Trap", 1, 1)], vec![]],
        );
        let source = game
            .object(game.zone(Zone::Battlefield)[0])
            .unwrap()
            .reference();
        crate::memory::fold(
            &mut game,
            &Remembered {
                source,
                slot: crate::memory::r112_lasting("number"),
                objects: Vec::new(),
                values: used.iter().copied().map(Held::Number).collect(),
                replace: false,
            },
        );
        game
    }

    fn run_with(game: &GameState, journal: Vec<(PlayerId, Choice)>) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(TRAP).unwrap())
    }

    #[test]
    fn r112_the_secret_number_leaves_out_the_numbers_chosen_before() {
        let game = game(&[2, 4]);
        let Run::Ask(decision) = run_with(&game, Vec::new()).unwrap() else {
            panic!("nothing asked");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseNumber(options) = decision.kind else {
            panic!("no number asked");
        };
        assert!(options.secret);
        assert_eq!(
            (options.min, options.max, options.except),
            (1, Some(5), vec![2, 4])
        );
    }

    #[test]
    fn r112_the_guess_follows_and_both_are_bound() {
        let game = game(&[]);
        let Run::Ask(decision) = run_with(&game, vec![(ME, Choice::SecretNumber(3))]).unwrap()
        else {
            panic!("no guess asked");
        };
        assert_eq!(decision.player, FOE);
        let run = run_with(
            &game,
            vec![(ME, Choice::SecretNumber(3)), (FOE, Choice::Number(5))],
        )
        .unwrap();
        let Run::Done { bound, events, .. } = run else {
            panic!("not done");
        };
        assert!(
            bound
                .iter()
                .any(|(name, _)| name.starts_with("~value:number\t"))
        );
        assert!(
            bound
                .iter()
                .any(|(name, _)| name.starts_with("~value:guess\t"))
        );
        assert!(
            bound
                .iter()
                .any(|(name, who)| name == "guesser" && who == &vec![Target::Player(FOE)])
        );
        assert_eq!(events.len(), 2, "remembered and grouped");
    }

    #[rstest]
    #[case::chosen_before(vec![(ME, Choice::SecretNumber(2))])]
    #[case::out_of_range(vec![(ME, Choice::SecretNumber(6))])]
    #[case::guess_out_of_range(vec![(ME, Choice::SecretNumber(3)), (FOE, Choice::Number(0))])]
    #[case::guess_by_the_chooser(vec![(ME, Choice::SecretNumber(3)), (ME, Choice::Number(1))])]
    fn r112_a_wrong_answer_is_refused(#[case] journal: Vec<(PlayerId, Choice)>) {
        assert!(run_with(&game(&[2]), journal).is_err());
    }

    #[test]
    fn r112_every_number_chosen_binds_nothing() {
        let run = run_with(&game(&[1, 2, 3, 4, 5]), Vec::new()).unwrap();
        assert!(matches!(run, Run::Done { ref bound, .. } if bound.is_empty()));
    }
}
