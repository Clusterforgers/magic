//! `Effect::VillainousChoice`: a player faces a villainous choice (CR 701.56) and the resolution goes on with the option
//! they choose.

use mtg_cards::ability::Ability;
use mtg_cards::compile::render_ability;
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::wording;

/// How an option is offered: what its effects do as a card says it (the prompt wording of a short effect, else the
/// card's own sentence), or its place when there are no words for them.
fn w2d_label(effects: &[Effect], fallback: &str) -> ChoiceOption {
    let asked = wording::may(effects);
    let sentence = || {
        render_ability(&Ability::Spell {
            targets: Vec::new(),
            effects: effects.to_vec(),
        })
    };
    let text = match asked.strip_suffix('?') {
        Some(text) if !text.is_empty() => text.to_owned(),
        _ => sentence()
            .map(|text| {
                let mut text = text.trim_end_matches('.').to_owned();
                if let Some(first) = text.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                text
            })
            .unwrap_or_default(),
    };
    ChoiceOption::Label(if text.is_empty() {
        fallback.to_owned()
    } else {
        text
    })
}

/// The first player the effect names who is still in the game chooses one of the two options, a public choice, and the
/// step goes into it; nobody in the game to choose is no choice.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::VillainousChoice {
        who, first, second, ..
    } = effect
    else {
        return Err(wrong_effect("VillainousChoice"));
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
    choose_between(cx, player, first, second, "Face a villainous choice.")
}

/// `player` chooses one of the two lists, a public choice, and the step goes into it.
pub(super) fn choose_between(
    cx: &Cx<'_>,
    player: PlayerId,
    first: &[Effect],
    second: &[Effect],
    text: &str,
) -> Result<Run, ResolveError> {
    let chooser = OptionChooser {
        player,
        options: vec![
            w2d_label(first, "The first option"),
            w2d_label(second, "The second option"),
        ],
        text: text.to_owned(),
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
    //! Goal: a villainous choice asks the player it names (not the controller) between the two options in the words of their effects, goes into the option chosen, asks nobody when that player has left, and refuses another effect by name.
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;
    use mtg_core::ids::PlayerId;

    const FACED: &str = r#"{"VillainousChoice":{"who":"Opponents","first":[{"Draw":{}}],"second":[{"GainLife":{"amount":3}}]}}"#;

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
    fn the_player_it_names_is_asked_between_the_two_options() {
        let Run::Ask(decision) = play(&game(), Vec::new(), FACED).unwrap() else {
            panic!("a villainous choice asks");
        };
        assert_eq!(decision.player, PlayerId(1));
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
        let run = play(&game(), vec![(PlayerId(1), Choice::Option(picked))], FACED).unwrap();
        assert!(matches!(run, Run::Done { next: Next::Enter(n), .. } if n == picked));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(), Vec::new(), r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("VillainousChoice")
        ));
    }
}
