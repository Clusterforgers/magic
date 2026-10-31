//! `Effect::Either`: the controller chooses one of two lists of effects as the effect resolves, and the step goes into it.

use mtg_cards::effect::Effect;

use super::w2d_villainous::choose_between;
use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};

/// The controller chooses one of the two lists, a public choice labelled with the words of its effects, and the step
/// goes into it; a controller who has left the game chooses nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Either { first, second } = effect else {
        return Err(wrong_effect("Either"));
    };
    let player = cx.scope.controller;
    if !cx.state().is_alive(player) {
        return Ok(Run::events(Vec::new()));
    }
    choose_between(cx, player, first, second, "Choose one.")
}

#[cfg(test)]
mod tests {
    //! Goal: an either asks the controller between its two options in the words of their effects, goes into the option chosen, and refuses another effect by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choice, ChoiceOption};
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const EITHER: &str =
        r#"{"Either":{"first":[{"Draw":{}}],"second":[{"GainLife":{"amount":3}}]}}"#;

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
    fn the_controller_is_asked_between_the_two_options() {
        let Run::Ask(decision) = play(&game(), Vec::new(), EITHER).unwrap() else {
            panic!("an either asks");
        };
        assert_eq!(decision.player, PlayerId(0));
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("the options are labels");
        };
        assert_eq!(
            options.options,
            [
                ChoiceOption::Label("Draw a card".to_owned()),
                ChoiceOption::Label("Gain 3 life".to_owned())
            ]
        );
    }

    #[rstest]
    #[case::the_first(0)]
    #[case::the_second(1)]
    fn the_option_chosen_is_entered(#[case] picked: usize) {
        let run = play(&game(), vec![(PlayerId(0), Choice::Option(picked))], EITHER).unwrap();
        assert!(matches!(run, Run::Done { next: Next::Enter(n), .. } if n == picked));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(), Vec::new(), r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("Either")
        ));
    }
}
