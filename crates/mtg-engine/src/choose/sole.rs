//! The engine's answer to a decision that leaves its player one legal answer.
//!
//! The table gives it for a seat out of Full Control (`docs/arena-flow.md`, P10). Only decisions whose options every
//! player can see qualify, so answering at once never shows what a hidden zone holds: a question whose single answer
//! could come from a hand or a library (a discard, a search, a payment the player cannot make) is always asked.

use super::{ModeChoice, NumberChoice, OptionChoice, ReplacementChoice, Target, TargetChoice};
use crate::decision::{Answer, Decision, DecisionKind};
use crate::state::GameState;
use crate::zones::Zone;

/// The one answer the rules leave `decision`, when every option it lists is in sight of every player.
pub fn sole_answer(state: &GameState, decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        // A triggered ability with exactly the targets it needs must take them (CR 603.3d); `Remove` is for none.
        DecisionKind::ChooseTargets(options) => {
            let need = usize::try_from(options.min).ok()?;
            let only = !options.can_cancel
                && options.apart.is_empty()
                && need > 0
                && options.min == options.max
                && options.legal.len() == need
                && options.legal.iter().all(|target| in_sight(state, *target));
            only.then(|| Answer::ChooseTargets(TargetChoice::Targets(options.legal.clone())))
        }
        DecisionKind::ChooseMode(options) => {
            let need = usize::try_from(options.min).ok()?;
            let only = !options.can_cancel
                && !options.repeat
                && options.min == options.max
                && options.modes.len() == need;
            only.then(|| {
                Answer::ChooseMode(ModeChoice::Modes(
                    options.modes.iter().map(|mode| mode.index).collect(),
                ))
            })
        }
        DecisionKind::ChooseNumber(options) => {
            let only = !options.can_cancel
                && options.max == Some(options.min)
                && !options.except.contains(&options.min);
            only.then_some(Answer::ChooseNumber(NumberChoice::Number(options.min)))
        }
        DecisionKind::ChooseOption(options) => {
            (options.options.len() == 1).then_some(Answer::ChooseOption(OptionChoice(0)))
        }
        DecisionKind::ChooseReplacement(options) => (options.candidates.len() == 1)
            .then_some(Answer::ChooseReplacement(ReplacementChoice(0))),
        _ => None,
    }
}

/// Whether every player can see `target`: a player, or an object on the battlefield, on the stack, in a graveyard or in
/// a command zone.
fn in_sight(state: &GameState, target: Target) -> bool {
    match target {
        Target::Player(_) => true,
        Target::Object(object) => state.object(object.id).is_some_and(|o| {
            matches!(
                o.zone(),
                Zone::Battlefield | Zone::Stack | Zone::Graveyard(_) | Zone::Command(_)
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a decision is answered alone exactly when the rules leave one answer and its options are in sight: targets only when the needed count equals the legal ones, none can be backed out of and none lies in a hidden zone; modes, numbers, options and replacement effects only with one way to answer; every other kind is always asked.
    use rstest::rstest;

    use super::*;
    use crate::choose::{
        ChoiceOption, ModeOption, ModeOptions, NumberOptions, OptionOptions, ReplacementCandidate,
        ReplacementOptions, TargetOptions, YesNoOptions,
    };
    use crate::fixtures::{creature, state};
    use crate::object::ObjectRef;
    use mtg_core::ids::{ObjectId, PlayerId};

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![creature("Held", 1, 1)], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        )
    }

    fn ask(kind: DecisionKind) -> Decision {
        Decision { player: ME, kind }
    }

    fn object(game: &GameState, zone: Zone) -> Target {
        let id = game.zone(zone)[0];
        let counter = game.object(id).map_or(0, |o| o.reference().counter);
        Target::Object(ObjectRef { id, counter })
    }

    fn targets(legal: Vec<Target>, min: u32, max: u32, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseTargets(TargetOptions {
            source: ObjectId(99),
            spec: 0,
            legal,
            min,
            max,
            can_cancel,
            wording: String::new(),
            triggered: !can_cancel,
            apart: Vec::new(),
        })
    }

    #[rstest]
    #[case::one_player(&["player"], 1, 1, false, true)]
    #[case::one_permanent(&["bear"], 1, 1, false, true)]
    #[case::both_needed(&["player", "bear"], 2, 2, false, true)]
    #[case::two_for_one(&["player", "bear"], 1, 1, false, false)]
    #[case::up_to_one(&["bear"], 0, 1, false, false)]
    #[case::cast_backs_out(&["bear"], 1, 1, true, false)]
    #[case::card_in_hand(&["hand"], 1, 1, false, false)]
    #[case::card_in_library(&["library"], 1, 1, false, false)]
    fn targets_are_taken_alone_only_when_forced_and_in_sight(
        #[case] names: &[&str],
        #[case] min: u32,
        #[case] max: u32,
        #[case] can_cancel: bool,
        #[case] alone: bool,
    ) {
        let game = game();
        let legal: Vec<Target> = names
            .iter()
            .map(|name| match *name {
                "player" => Target::Player(PlayerId(1)),
                "bear" => object(&game, Zone::Battlefield),
                "hand" => object(&game, Zone::Hand(ME)),
                _ => object(&game, Zone::Library(ME)),
            })
            .collect();
        let answer = sole_answer(&game, &ask(targets(legal.clone(), min, max, can_cancel)));
        let expected = alone.then_some(Answer::ChooseTargets(TargetChoice::Targets(legal)));
        assert_eq!(answer, expected);
    }

    fn modes(count: usize, min: u32, max: u32, repeat: bool, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseMode(ModeOptions {
            source: ObjectId(99),
            modes: (0..count)
                .map(|at| ModeOption {
                    index: at * 2,
                    label: None,
                    cost: None,
                })
                .collect(),
            min,
            max,
            repeat,
            can_cancel,
            cheapest: Vec::new(),
        })
    }

    fn number(min: u32, max: Option<u32>, except: Vec<u32>, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseNumber(NumberOptions {
            source: ObjectId(99),
            min,
            max,
            except,
            text: String::new(),
            can_cancel,
            cost: None,
            per_x: 0,
        })
    }

    fn choose(count: usize) -> DecisionKind {
        DecisionKind::ChooseOption(OptionOptions {
            source: ObjectId(99),
            options: (0..count)
                .map(|at| ChoiceOption::Label(at.to_string()))
                .collect(),
            text: String::new(),
        })
    }

    fn replacements(count: usize) -> DecisionKind {
        DecisionKind::ChooseReplacement(ReplacementOptions {
            candidates: (0..count)
                .map(|at| ReplacementCandidate {
                    source: ObjectId(99),
                    ability: at,
                    shield: None,
                    text: String::new(),
                })
                .collect(),
            text: String::new(),
        })
    }

    #[rstest]
    #[case::one_mode(modes(1, 1, 1, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![0]))))]
    #[case::all_modes(modes(2, 2, 2, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![0, 2]))))]
    #[case::mode_of_two(modes(2, 1, 1, false, false), None)]
    #[case::mode_repeats(modes(1, 2, 2, true, false), None)]
    #[case::mode_of_a_cast(modes(1, 1, 1, false, true), None)]
    #[case::fixed_number(number(3, Some(3), vec![], false), Some(Answer::ChooseNumber(NumberChoice::Number(3))))]
    #[case::number_range(number(0, Some(3), vec![], false), None)]
    #[case::number_unbounded(number(3, None, vec![], false), None)]
    #[case::number_of_a_cast(number(3, Some(3), vec![], true), None)]
    #[case::number_refused(number(3, Some(3), vec![3], false), None)]
    #[case::one_option(choose(1), Some(Answer::ChooseOption(OptionChoice(0))))]
    #[case::two_options(choose(2), None)]
    #[case::one_replacement(replacements(1), Some(Answer::ChooseReplacement(ReplacementChoice(0))))]
    #[case::two_replacements(replacements(2), None)]
    #[case::yes_no(DecisionKind::YesNo(YesNoOptions { source: ObjectId(99), text: String::new(), only_no: true }), None)]
    fn one_way_to_answer_is_given_alone(
        #[case] kind: DecisionKind,
        #[case] expected: Option<Answer>,
    ) {
        assert_eq!(sole_answer(&game(), &ask(kind)), expected);
    }
}
