//! `Effect::ChooseWords`: each player the effect names chooses one of its words, the controller first and the others in
//! turn order (face down when secret, revealed together), and each word is bound to the players who chose it.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, Target, pick_options};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// How a word is shown in the question and in the options: the words of the card.
fn shown(word: &str) -> String {
    word.replace('_', " ")
}

/// The choosers: each player `who` names who is still in the game, the controller first and the others in turn order.
fn choosers(cx: &Cx<'_>, who: &mtg_cards::refs::Ref) -> Result<Vec<PlayerId>, ResolveError> {
    let named = cx.scope.players(who)?;
    Ok(cx
        .state()
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .filter(|player| named.contains(player))
        .collect())
}

/// One question to the single player `who` names for each player `about` names, in turn order from the controller
/// ("For each player, choose friend or foe"); the questions name the player by seat.
fn r6008_for_each(
    cx: &Cx<'_>,
    who: &mtg_cards::refs::Ref,
    about: &mtg_cards::refs::Ref,
    options: &[ChoiceOption],
    words: &[String],
) -> Result<Vec<OptionChooser>, ResolveError> {
    let [chooser] = cx.scope.players(who)?[..] else {
        return Err(ResolveError::Invalid(
            "one player chooses the words for each player".into(),
        ));
    };
    let listed = words
        .iter()
        .map(|word| shown(word))
        .collect::<Vec<_>>()
        .join(" or ");
    Ok(choosers(cx, about)?
        .into_iter()
        .map(|player| OptionChooser {
            player: chooser,
            options: options.to_vec(),
            text: if player == chooser {
                format!("For yourself, choose {listed}.")
            } else {
                format!("For seat {}, choose {listed}.", player.0 + 1)
            },
        })
        .collect())
}

/// Each chooser picks one of the words (face down when `secret`); each word is bound to the players who chose it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChooseWords {
        who,
        words,
        secret,
        about,
    } = effect
    else {
        return Err(wrong_effect("ChooseWords"));
    };
    let options: Vec<ChoiceOption> = words
        .iter()
        .map(|word| {
            if *secret {
                ChoiceOption::Secret(shown(word))
            } else {
                ChoiceOption::Label(shown(word))
            }
        })
        .collect();
    let text = format!(
        "{} {}.",
        if *secret { "Secretly choose" } else { "Choose" },
        words
            .iter()
            .map(|word| shown(word))
            .collect::<Vec<_>>()
            .join(" or ")
    );
    let asked: Vec<OptionChooser> = match about {
        Some(about) => r6008_for_each(cx, who, about, &options, words)?,
        None => choosers(cx, who)?
            .into_iter()
            .map(|player| OptionChooser {
                player,
                options: options.clone(),
                text: text.clone(),
            })
            .collect(),
    };
    let mut taken = match pick_options(cx, &asked)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    if let Some(about) = about {
        // Each answer is about the player its question named, in the order asked.
        let named = choosers(cx, about)?;
        taken = named
            .into_iter()
            .zip(taken)
            .map(|(player, (_, picked))| (player, picked))
            .collect();
    }
    let bound = words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let chose: Vec<Target> = taken
                .iter()
                .filter(|(_, picked)| *picked == index)
                .map(|(player, _)| Target::Player(*player))
                .collect();
            (word.clone(), chose)
        })
        .collect();
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a choice of words asks the players it names, the controller's opponents in turn order from the controller at
    //! two to four seats, offers the words face down when secret and open otherwise, and refuses another effect by name.
    use rstest::rstest;

    use super::*;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn play(game: &GameState, controller: u64, effect: &str) -> Result<Run, ResolveError> {
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(controller), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(effect).unwrap())
    }

    const DILEMMA: &str =
        r#"{"ChooseWords":{"who":"Opponents","words":["silence","snitch"],"secret":true}}"#;

    #[rstest]
    #[case::two_seats(2, 0, 1)]
    #[case::three_seats(3, 0, 1)]
    #[case::four_seats_from_the_last(4, 3, 0)]
    #[case::four_seats_from_seat_one(4, 1, 2)]
    fn the_first_opponent_in_turn_order_chooses_first(
        #[case] seats: usize,
        #[case] controller: u64,
        #[case] first: u64,
    ) {
        let Run::Ask(decision) = play(&game(seats), controller, DILEMMA).unwrap() else {
            panic!("a choice of words asks the first chooser");
        };
        assert_eq!(decision.player, PlayerId(first));
    }

    #[rstest]
    #[case::secret(DILEMMA, true, "Secretly choose silence or snitch.")]
    #[case::open(
        r#"{"ChooseWords":{"who":"Opponents","words":["silence","snitch"]}}"#,
        false,
        "Choose silence or snitch."
    )]
    fn the_words_are_offered_face_down_when_secret(
        #[case] json: &str,
        #[case] secret: bool,
        #[case] text: &str,
    ) {
        let Run::Ask(decision) = play(&game(3), 0, json).unwrap() else {
            panic!("a choice of words asks the first chooser");
        };
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("words are options");
        };
        let want = |word: &str| {
            if secret {
                ChoiceOption::Secret(word.to_owned())
            } else {
                ChoiceOption::Label(word.to_owned())
            }
        };
        assert_eq!(options.options, [want("silence"), want("snitch")]);
        assert_eq!(options.text, text);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(2), 0, r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("ChooseWords")
        ));
    }

    const FRIEND_OR_FOE: &str =
        r#"{"ChooseWords":{"who":"You","words":["friend","foe"],"about":"EachPlayer"}}"#;

    #[rstest]
    #[case::two_seats(2, 0, &[1, 0])]
    #[case::three_seats(3, 1, &[0, 1, 1])]
    #[case::four_seats(4, 2, &[1, 1, 0, 0])]
    fn r6008_the_controller_chooses_a_word_for_each_player(
        #[case] seats: usize,
        #[case] controller: u64,
        #[case] picks: &[usize],
    ) {
        let game = game(seats);
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let mut resolving = Resolving::begin(source.id());
        let mut asked = 0;
        let bound = loop {
            let cx = Cx {
                scope: Scope::announcing(&game, PlayerId(controller), source.reference()),
                resolving: &resolving,
            };
            match run(&cx, &serde_json::from_str(FRIEND_OR_FOE).unwrap()).unwrap() {
                Run::Ask(decision) => {
                    assert_eq!(
                        decision.player,
                        PlayerId(controller),
                        "only the controller is asked"
                    );
                    resolving.journal.push((
                        PlayerId(controller),
                        crate::choose::Choice::Option(picks[asked]),
                    ));
                    asked += 1;
                }
                Run::Done { bound, .. } => break bound,
                Run::Cast(_) => panic!("a choice of words asks or is done"),
            }
        };
        assert_eq!(asked, seats, "one question for each player");
        let order = game.turn_order_from(PlayerId(controller));
        let want = |word: usize| -> Vec<Target> {
            order
                .iter()
                .zip(picks)
                .filter(|(_, pick)| **pick == word)
                .map(|(player, _)| Target::Player(*player))
                .collect()
        };
        assert_eq!(
            bound,
            vec![("friend".to_owned(), want(0)), ("foe".to_owned(), want(1))]
        );
    }
}
