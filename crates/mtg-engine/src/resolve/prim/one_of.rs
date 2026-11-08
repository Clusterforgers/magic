//! `Effect::OneOf`: a player chooses one of several lists of effects as the effect resolves, and the step goes into it.

use mtg_cards::effect::Effect;

use super::w2d_villainous::w2d_label;
use super::wrong_effect;
use crate::choose::{OptionChooser, Optioned, pick_options};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The first player the effect names who is still in the game chooses one of the options, a public choice labelled with
/// the words of its effects, and the step goes into it; nobody in the game to choose is no choice. A clock that runs out
/// takes the first option, as for every question.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::OneOf { who, options } = effect else {
        return Err(wrong_effect("OneOf"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let chooser = OptionChooser {
        player,
        options: options
            .iter()
            .enumerate()
            .map(|(at, option)| w2d_label(option, &format!("Option {}", at + 1)))
            .collect(),
        text: "Choose one.".to_owned(),
    };
    Ok(match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => Run::Ask(decision),
        Optioned::Done(taken) => Run::Done {
            events: Vec::new(),
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Enter(taken.first().map_or(0, |(_, picked)| *picked)),
        },
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a choice of lists asks the player it names between all its options in the words of their effects, goes into the option chosen, asks nobody when that player has left, and refuses another effect by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choice, ChoiceOption};
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const THREE: &str = r#"{"OneOf":{"who":"Opponents","options":[[{"Draw":{}}],[{"GainLife":{"amount":3}}],[{"LoseLife":{"amount":2}}]]}}"#;

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn play(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[test]
    fn the_named_player_is_asked_among_every_option() {
        let Run::Ask(decision) = play(&game(), Vec::new(), THREE).unwrap() else {
            panic!("a choice asks");
        };
        assert_eq!(decision.player, PlayerId(1));
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("the options are labels");
        };
        assert_eq!(
            options.options,
            [
                ChoiceOption::Label("Draw a card".to_owned()),
                ChoiceOption::Label("Gain 3 life".to_owned()),
                ChoiceOption::Label("Lose 2 life".to_owned())
            ]
        );
    }

    #[rstest]
    #[case::the_first(0)]
    #[case::the_second(1)]
    #[case::the_third(2)]
    fn the_option_chosen_is_entered(#[case] picked: usize) {
        let run = play(&game(), vec![(PlayerId(1), Choice::Option(picked))], THREE).unwrap();
        assert!(matches!(run, Run::Done { next: Next::Enter(n), .. } if n == picked));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = play(&game(), Vec::new(), r#"{"Draw":{}}"#).unwrap_err();
        assert!(error.to_string().contains("OneOf"), "{error}");
    }
}
