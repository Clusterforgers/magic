//! `Effect::TapOrUntap`: the controller chooses to tap or to untap the permanents the effect names (CR 701.26).

use mtg_cards::effect::Effect;

use super::{tap, wrong_effect};
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The controller picks "Tap" or "Untap", a public choice, and the permanents are set that way; with no permanent left
/// on the battlefield to name nobody is asked.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::TapOrUntap { what } = effect else {
        return Err(wrong_effect("TapOrUntap"));
    };
    let live = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .any(|object| object.zone() == Zone::Battlefield);
    if !live {
        return Ok(Run::events(Vec::new()));
    }
    let chooser = OptionChooser {
        player: cx.scope.controller,
        options: vec![
            ChoiceOption::Label("Tap".to_owned()),
            ChoiceOption::Label("Untap".to_owned()),
        ],
        text: "Tap or untap it.".to_owned(),
    };
    match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => Ok(Run::Ask(decision)),
        Optioned::Done(taken) => {
            let untap = taken.first().is_some_and(|(_, picked)| *picked == 1);
            tap::set(cx, what, !untap)
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: tap or untap asks the controller between the two, taps or untaps the permanent as answered (a permanent already in that state stays), asks nothing when nothing it names is on the battlefield, and refuses another effect by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const EITHER: &str = r#"{"TapOrUntap":{"what":"This"}}"#;

    fn game(tapped: bool) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        if tapped {
            let id = game.zone(Zone::Battlefield)[0];
            crate::fold::apply_event(&mut game, &Event::PermanentTapped { object: id }).unwrap();
        }
        game
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

    fn events(run: Run) -> Vec<Event> {
        match run {
            Run::Done { events, .. } => events,
            _ => panic!("the step is done"),
        }
    }

    #[test]
    fn the_controller_is_asked_between_tap_and_untap() {
        let Run::Ask(decision) = play(&game(false), Vec::new(), EITHER).unwrap() else {
            panic!("tap or untap asks");
        };
        assert_eq!(decision.player, PlayerId(0));
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("the options are labels");
        };
        assert_eq!(options.options.len(), 2);
    }

    #[rstest]
    #[case::tap_untapped(false, 0, true)]
    #[case::untap_untapped(false, 1, false)]
    #[case::tap_tapped(true, 0, false)]
    #[case::untap_tapped(true, 1, true)]
    fn the_answer_decides_the_step(
        #[case] tapped: bool,
        #[case] picked: usize,
        #[case] changes: bool,
    ) {
        let game = game(tapped);
        let made =
            events(play(&game, vec![(PlayerId(0), Choice::Option(picked))], EITHER).unwrap());
        assert_eq!(made.len(), usize::from(changes));
        if changes {
            assert_eq!(
                matches!(made[0], Event::PermanentTapped { .. }),
                picked == 0
            );
        }
    }

    #[test]
    fn nothing_on_the_battlefield_asks_nothing() {
        let made =
            events(play(&game(false), Vec::new(), r#"{"TapOrUntap":{"what":"You"}}"#).unwrap());
        assert!(made.is_empty());
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(false), Vec::new(), r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("TapOrUntap")
        ));
    }
}
