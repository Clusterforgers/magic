//! `ChooseMode`: the modes of a modal spell or ability (CR 700.2).
//!
//! The modes are chosen as the spell or ability is announced, before its targets, and only the targets of the modes
//! chosen are asked. A mode whose targets cannot be met is not offered (CR 700.2a), and a spell or ability with too
//! few modes to choose cannot be cast or put on the stack at all. The modes are kept in the order they are printed,
//! which is the order they happen in.

use mtg_cards::ability::Ability;
use mtg_cards::compile::render_mode;
use mtg_cards::effect::{Effect, Mode};
use mtg_cards::refs::{Bounds, TargetSpec};
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::targets::options_before_x;
use crate::abilities::Cause;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;
use crate::state::GameState;

/// One mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModeOption {
    /// Its index among the ability's modes.
    pub index: usize,
    /// A short name, when the card gives one.
    pub label: Option<String>,
}

/// The modes a player may choose and how many.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModeOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The modes that may be chosen.
    pub modes: Vec<ModeOption>,
    /// The fewest to choose.
    pub min: u32,
    /// The most to choose.
    pub max: u32,
    /// The same mode may be chosen more than once.
    pub repeat: bool,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for. A triggered
    /// ability that chooses its modes as it goes on the stack cannot (CR 603.3c).
    pub can_cancel: bool,
}

/// The answer to a mode decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ModeChoice {
    /// These modes, by index, in any order: between `min` and `max` of them, each once unless the mode may repeat. The
    /// engine puts them in the order they are printed, which is the order they happen in.
    Modes(Vec<usize>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

/// Checks the answer against the options it answers; the modes it names are returned in printed order.
pub(crate) fn answer(
    _player: PlayerId,
    options: &ModeOptions,
    choice: ModeChoice,
) -> Result<Outcome, EngineError> {
    let mut modes = match choice {
        ModeChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        ModeChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        ModeChoice::Modes(modes) => modes,
    };
    for (at, mode) in modes.iter().enumerate() {
        if !options.repeat && modes[..at].contains(mode) {
            return Err(IllegalAnswer::Duplicate("mode").into());
        }
        if !options.modes.iter().any(|offered| offered.index == *mode) {
            return Err(IllegalAnswer::NotOffered("mode").into());
        }
    }
    let count = u32::try_from(modes.len()).unwrap_or(u32::MAX);
    let wanted = if count < options.min {
        Some(options.min)
    } else if count > options.max {
        Some(options.max)
    } else {
        None
    };
    if let Some(expected) = wanted {
        return Err(IllegalAnswer::WrongCount {
            expected: usize::try_from(expected).unwrap_or(usize::MAX),
            got: modes.len(),
        }
        .into());
    }
    modes.sort_unstable();
    Ok(Outcome::Modes(modes))
}

/// The modes of a modal ability and how many are chosen.
pub(crate) struct Modal<'a> {
    /// How many modes are chosen.
    pub(crate) choose: &'a Bounds,
    /// The same mode may be chosen more than once.
    pub(crate) repeat: bool,
    /// The modes, in printed order.
    pub(crate) modes: &'a [Mode],
}

/// The modes of the ability, if it is a modal one: a `Modal` is the only effect of its ability.
pub(crate) fn modal_of(rules: &Ability) -> Option<Modal<'_>> {
    match rules.effects() {
        [
            Effect::Modal {
                choose,
                repeat,
                modes,
            },
        ] => Some(Modal {
            choose,
            repeat: *repeat,
            modes,
        }),
        _ => None,
    }
}

/// Whether every target specification of the mode has enough legal targets for the mode to be chosen.
fn is_legal(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    mode: &Mode,
    cause: Option<&Cause>,
) -> bool {
    mode.targets.iter().enumerate().all(|(index, spec)| {
        options_before_x(state, source, controller, index, spec, cause).is_some()
    })
}

/// Whether every mode of `modal` has the targets it needs, which choosing all of them (entwine) asks for.
pub(crate) fn every_mode_is_legal(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    modal: &Modal<'_>,
    cause: Option<&Cause>,
) -> bool {
    modal
        .modes
        .iter()
        .all(|mode| is_legal(state, source, controller, mode, cause))
}

/// What a mode is called in the prompt: the label the card gives it, else the oracle wording of what it does (a
/// compiled mode has no label), with the card's own name where the wording says `~`.
fn label_of(mode: &Mode, card: Option<&str>) -> Option<String> {
    mode.label.clone().or_else(|| {
        render_mode(mode).map(|text| match card {
            Some(name) => text.replace('~', name),
            None => text,
        })
    })
}

/// The decision for the modes of `modal`, or `None` when too few can be chosen: fewer legal modes than the fewest to
/// choose, or none at all for a mode that must be chosen.
pub(crate) fn mode_options(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    modal: &Modal<'_>,
    can_cancel: bool,
    cause: Option<&Cause>,
) -> Option<ModeOptions> {
    let card = state.object(source).map(|object| object.def.name.as_str());
    let modes: Vec<ModeOption> = modal
        .modes
        .iter()
        .enumerate()
        .filter(|(_, mode)| is_legal(state, source, controller, mode, cause))
        .map(|(index, mode)| ModeOption {
            index,
            label: label_of(mode, card),
        })
        .collect();
    let available = u32::try_from(modes.len()).unwrap_or(u32::MAX);
    let min = modal.choose.min;
    // A mode that may repeat can be chosen as often as is wanted, so one legal mode is enough.
    let enough = if modal.repeat {
        available > 0 || min == 0
    } else {
        available >= min
    };
    if !enough {
        return None;
    }
    let wanted = match &modal.choose.max {
        None => available,
        Some(Amount::Fixed(n)) => u32::try_from(*n).unwrap_or(0),
        Some(other) => {
            let origin = state.object(source).map_or(
                ObjectRef {
                    id: source,
                    counter: 0,
                },
                GameObject::reference,
            );
            let amount = Scope::announcing(state, controller, origin)
                .amount(other)
                .ok()?;
            u32::try_from(amount).unwrap_or(0)
        }
    };
    let max = if modal.repeat {
        wanted
    } else {
        wanted.min(available)
    };
    Some(ModeOptions {
        source,
        modes,
        min,
        max: max.max(min),
        repeat: modal.repeat,
        can_cancel,
    })
}

/// The target specifications announced for a spell or ability, in the order they are asked: its own, or for a modal
/// one those of the modes chosen, mode after mode (a mode chosen twice asks its targets twice).
pub(crate) fn announced_specs<'a>(rules: &'a Ability, chosen: &[usize]) -> Vec<&'a TargetSpec> {
    match modal_of(rules) {
        None => rules.targets().iter().collect(),
        Some(modal) => chosen
            .iter()
            .filter_map(|index| modal.modes.get(*index))
            .flat_map(|mode| mode.targets.iter())
            .collect(),
    }
}

/// How many announced target specifications come before those of the `ordinal`-th mode chosen.
pub(crate) fn targets_before(rules: &Ability, chosen: &[usize], ordinal: usize) -> usize {
    modal_of(rules).map_or(0, |modal| {
        chosen
            .iter()
            .take(ordinal)
            .filter_map(|index| modal.modes.get(*index))
            .map(|mode| mode.targets.len())
            .sum()
    })
}

/// Whether a spell or ability of `source` controlled by `player` can be announced: every target specification has
/// enough legal targets (CR 601.2c) and, for a modal one, enough modes can be chosen (CR 700.2a).
pub(crate) fn announceable(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    rules: &Ability,
) -> bool {
    match modal_of(rules) {
        Some(modal) => mode_options(state, source, player, &modal, true, None).is_some(),
        None => rules.targets().iter().enumerate().all(|(index, spec)| {
            options_before_x(state, source, player, index, spec, None).is_some()
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a mode decision is accepted exactly when it names modes among those offered, each once unless the mode may repeat, between the fewest and the most, or backs out of a cast that can be backed out of, and the modes come back in printed order; a mode is offered only when its targets can be met, a modal ability with too few modes to choose cannot be announced, the target specifications of a modal ability are those of the modes chosen, in order, and a mode is offered under the label the card gives it, else under the oracle wording of what it does.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;

    const DAMAGE: &str = r#"{"label":"Damage","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],
        "effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}"#;
    const LIFE: &str = r#"{"label":"Life","effects":[{"GainLife":{"amount":3}}]}"#;
    const CARDS: &str = r#"{"label":"Cards","effects":[{"Draw":{}}]}"#;

    /// A modal spell with some of three modes: damage to a creature (0, which needs a creature to target), 3 life
    /// (1) and a card (2).
    fn modal_with(modes: &[usize], choose: &str, repeat: bool) -> Ability {
        let list: Vec<&str> = modes.iter().map(|i| [DAMAGE, LIFE, CARDS][*i]).collect();
        let json = format!(
            r#"{{"Spell":{{"effects":[{{"Modal":{{{choose}"repeat":{repeat},"modes":[{}]}}}}]}}}}"#,
            list.join(",")
        );
        serde_json::from_str(&json).unwrap()
    }

    fn modal(choose: &str, repeat: bool) -> Ability {
        modal_with(&[0, 1, 2], choose, repeat)
    }

    fn up_to(max: u32) -> String {
        format!(r#""choose":{{"min":0,"max":{max}}},"#)
    }

    fn between(min: u32, max: u32) -> String {
        format!(r#""choose":{{"min":{min},"max":{max}}},"#)
    }

    fn offered(
        indices: &[usize],
        min: u32,
        max: u32,
        repeat: bool,
        can_cancel: bool,
    ) -> ModeOptions {
        ModeOptions {
            source: ObjectId(1),
            modes: indices
                .iter()
                .map(|index| ModeOption {
                    index: *index,
                    label: None,
                })
                .collect(),
            min,
            max,
            repeat,
            can_cancel,
        }
    }

    fn accepted(options: &ModeOptions, modes: &[usize]) -> Vec<usize> {
        match answer(PlayerId(0), options, ModeChoice::Modes(modes.to_vec())).unwrap() {
            Outcome::Modes(chosen) => chosen,
            other => panic!("the modes were expected, got {other:?}"),
        }
    }

    fn refused(options: &ModeOptions, modes: &[usize]) -> IllegalAnswer {
        match answer(PlayerId(0), options, ModeChoice::Modes(modes.to_vec())).unwrap_err() {
            EngineError::Illegal(why) => why,
            other => panic!("an illegal answer was expected, got {other:?}"),
        }
    }

    #[rstest]
    #[case::one_of_three(offered(&[0, 1, 2], 1, 1, false, true), &[1], &[1])]
    #[case::two_of_three(offered(&[0, 1, 2], 2, 2, false, true), &[0, 2], &[0, 2])]
    #[case::in_any_order_comes_back_in_printed_order(offered(&[0, 1, 2], 2, 2, false, true), &[2, 0], &[0, 2])]
    #[case::one_or_both(offered(&[0, 1], 1, 2, false, true), &[1, 0], &[0, 1])]
    #[case::up_to_two_may_be_none(offered(&[0, 1, 2], 0, 2, false, true), &[], &[])]
    #[case::a_mode_that_may_repeat_twice(offered(&[0, 1, 2], 2, 2, true, true), &[1, 1], &[1, 1])]
    #[case::repeats_and_others_in_printed_order(offered(&[0, 1, 2], 3, 3, true, false), &[2, 0, 2], &[0, 2, 2])]
    #[case::only_the_offered_indices_count(offered(&[1, 2], 1, 1, false, true), &[2], &[2])]
    fn a_legal_answer_is_accepted_and_comes_back_in_printed_order(
        #[case] options: ModeOptions,
        #[case] answered: &[usize],
        #[case] chosen: &[usize],
    ) {
        assert_eq!(accepted(&options, answered), chosen);
    }

    #[rstest]
    #[case::a_mode_that_does_not_exist(offered(&[0, 1, 2], 1, 1, false, true), &[3], IllegalAnswer::NotOffered("mode"))]
    #[case::a_mode_that_is_not_offered(offered(&[1, 2], 1, 1, false, true), &[0], IllegalAnswer::NotOffered("mode"))]
    #[case::a_repeat_of_a_mode_that_may_not_repeat(offered(&[0, 1, 2], 2, 2, false, true), &[1, 1], IllegalAnswer::Duplicate("mode"))]
    #[case::too_few(offered(&[0, 1, 2], 2, 2, false, true), &[1], IllegalAnswer::WrongCount { expected: 2, got: 1 })]
    #[case::none_when_one_is_needed(offered(&[0, 1, 2], 1, 1, false, true), &[], IllegalAnswer::WrongCount { expected: 1, got: 0 })]
    #[case::too_many(offered(&[0, 1, 2], 1, 2, false, true), &[0, 1, 2], IllegalAnswer::WrongCount { expected: 2, got: 3 })]
    #[case::too_many_repeats(offered(&[0, 1], 2, 2, true, true), &[1, 1, 1], IllegalAnswer::WrongCount { expected: 2, got: 3 })]
    fn an_illegal_answer_is_refused_by_name(
        #[case] options: ModeOptions,
        #[case] answered: &[usize],
        #[case] why: IllegalAnswer,
    ) {
        assert_eq!(refused(&options, answered), why);
    }

    #[test]
    fn a_cast_can_be_backed_out_of() {
        let options = offered(&[0, 1], 1, 1, false, true);
        let outcome = answer(PlayerId(0), &options, ModeChoice::Cancel);
        assert!(matches!(outcome, Ok(Outcome::CancelCast)), "{outcome:?}");
    }

    #[test]
    fn a_trigger_cannot_be_backed_out_of() {
        let options = offered(&[0, 1], 1, 1, false, false);
        assert_eq!(
            answer(PlayerId(0), &options, ModeChoice::Cancel).unwrap_err(),
            EngineError::Illegal(IllegalAnswer::NotOffered("cancel"))
        );
    }

    /// Seat 0's Source, which is no creature, and, when `creatures`, seat 1's Bear are on the battlefield.
    fn game(creatures: bool) -> GameState {
        let source = Arc::new(CardDefinition::new("Source").with_type(CardType::Artifact));
        state(
            vec![vec![], vec![]],
            vec![
                vec![source],
                if creatures {
                    vec![creature("Bear", 2, 2)]
                } else {
                    Vec::new()
                },
            ],
        )
    }

    fn source(game: &GameState) -> ObjectId {
        game.zone(Zone::Battlefield)[0]
    }

    fn options_of(game: &GameState, rules: &Ability, can_cancel: bool) -> Option<ModeOptions> {
        let modal = modal_of(rules).expect("a modal ability");
        mode_options(game, source(game), PlayerId(0), &modal, can_cancel, None)
    }

    fn indices(options: &ModeOptions) -> Vec<usize> {
        options.modes.iter().map(|mode| mode.index).collect()
    }

    #[rstest]
    #[case::every_mode_when_a_creature_can_be_targeted(true, String::new(), false, &[0, 1, 2], 1, 1)]
    #[case::a_mode_without_a_legal_target_is_not_offered(false, String::new(), false, &[1, 2], 1, 1)]
    #[case::one_to_five_is_cut_to_the_modes_there_are(false, between(1, 5), false, &[1, 2], 1, 2)]
    #[case::up_to_two(true, up_to(2), false, &[0, 1, 2], 0, 2)]
    #[case::a_repeating_choice_of_three_is_not_cut(false, between(3, 3), true, &[1, 2], 3, 3)]
    fn the_modes_offered_are_those_that_can_be_chosen(
        #[case] creatures: bool,
        #[case] choose: String,
        #[case] repeat: bool,
        #[case] expected: &[usize],
        #[case] min: u32,
        #[case] max: u32,
    ) {
        let rules = modal(&choose, repeat);
        let game = game(creatures);
        let options = options_of(&game, &rules, true).expect("it can be announced");
        assert_eq!(indices(&options), expected);
        assert_eq!(
            (options.min, options.max, options.repeat),
            (min, max, repeat)
        );
        assert_eq!(options.source, source(&game));
        assert!(
            options.modes.iter().all(|mode| mode.label.is_some()),
            "the card's name for each mode"
        );
    }

    /// A modal spell whose modes carry no label, as the compiler writes them, or the given labels.
    fn unlabeled(labels: [Option<&str>; 3]) -> Ability {
        let modes = [
            r#"{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}"#,
            r#"{"effects":[{"GainLife":{"amount":3}}]}"#,
            r#"{"effects":[{"Draw":{}}]}"#,
        ];
        let modes: Vec<String> = modes
            .iter()
            .zip(labels)
            .map(|(mode, label)| match label {
                Some(label) => mode.replacen('{', &format!(r#"{{"label":"{label}","#), 1),
                None => (*mode).to_owned(),
            })
            .collect();
        serde_json::from_str(&format!(
            r#"{{"Spell":{{"effects":[{{"Modal":{{"choose":{{"min":1,"max":1}},"modes":[{}]}}}}]}}}}"#,
            modes.join(",")
        ))
        .unwrap()
    }

    fn labels_of(options: &ModeOptions) -> Vec<Option<&str>> {
        options
            .modes
            .iter()
            .map(|mode| mode.label.as_deref())
            .collect()
    }

    #[test]
    fn a_mode_without_a_label_is_offered_with_the_text_of_what_it_does() {
        let rules = unlabeled([None, None, None]);
        let options = options_of(&game(true), &rules, true).unwrap();
        assert_eq!(
            labels_of(&options),
            [
                Some("Source deals 2 damage to target creature"),
                Some("You gain 3 life"),
                Some("Draw a card"),
            ]
        );
    }

    #[test]
    fn a_label_the_card_gives_is_kept() {
        let rules = unlabeled([Some("Shock"), None, Some("Cantrip")]);
        let options = options_of(&game(true), &rules, true).unwrap();
        assert_eq!(
            labels_of(&options),
            [Some("Shock"), Some("You gain 3 life"), Some("Cantrip")]
        );
    }

    #[test]
    fn a_mode_the_renderer_cannot_write_has_no_label() {
        // Two target specifications are more than the renderer writes for a mode.
        let rules: Ability = serde_json::from_str(
            r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":1},"modes":[
                {"effects":[{"Draw":{}}]},
                {"targets":[{"what":"AnyTarget"},{"what":"AnyTarget"}],"effects":[
                    {"DealDamage":{"to":{"Target":0},"amount":1}},{"DealDamage":{"to":{"Target":1},"amount":1}}]}]}}]}}"#,
        )
        .unwrap();
        let options = options_of(&game(false), &rules, true).unwrap();
        assert_eq!(labels_of(&options), [Some("Draw a card"), None]);
    }

    #[rstest]
    #[case::two_needed_and_two_legal(&[0, 1, 2], between(2, 2), false, true)]
    #[case::two_needed_and_one_legal(&[0, 1], between(2, 2), false, false)]
    #[case::one_needed_and_none_legal(&[0], between(1, 1), false, false)]
    #[case::a_repeating_mode_needs_one_legal_mode(&[0, 1], between(3, 3), true, true)]
    #[case::a_repeating_mode_with_none_legal(&[0], between(3, 3), true, false)]
    #[case::up_to_needs_none(&[0], up_to(1), false, true)]
    fn a_modal_ability_with_too_few_modes_to_choose_cannot_be_announced(
        #[case] modes: &[usize],
        #[case] choose: String,
        #[case] repeat: bool,
        #[case] announced: bool,
    ) {
        // There is no creature to target, which leaves the damage mode out.
        let rules = modal_with(modes, &choose, repeat);
        let game = game(false);
        assert_eq!(options_of(&game, &rules, true).is_some(), announced);
        assert_eq!(
            announceable(&game, PlayerId(0), source(&game), &rules),
            announced
        );
    }

    #[test]
    fn a_trigger_cannot_back_out() {
        let rules = modal("", false);
        let options = options_of(&game(true), &rules, false).unwrap();
        assert!(!options.can_cancel);
    }

    #[test]
    fn the_target_specifications_are_those_of_the_modes_chosen_in_order() {
        let rules = modal(&between(1, 3), true);
        let kinds = |chosen: &[usize]| announced_specs(&rules, chosen).len();
        assert_eq!(kinds(&[]), 0, "nothing chosen asks nothing");
        assert_eq!(kinds(&[1, 2]), 0, "modes without targets ask nothing");
        assert_eq!(kinds(&[0]), 1);
        assert_eq!(kinds(&[0, 0, 1]), 2, "a mode chosen twice asks twice");
        assert_eq!(targets_before(&rules, &[0, 1, 0], 0), 0);
        assert_eq!(targets_before(&rules, &[0, 1, 0], 1), 1);
        assert_eq!(targets_before(&rules, &[0, 1, 0], 2), 1);
        assert_eq!(targets_before(&rules, &[0, 1, 0], 3), 2);
    }

    #[test]
    fn an_ability_that_is_not_modal_keeps_its_own_specifications() {
        let plain: Ability = serde_json::from_str(
            r#"{"Spell":{"targets":[{"what":"Player"},{"what":"AnyTarget"}],"effects":[{"Draw":{}}]}}"#,
        )
        .unwrap();
        assert!(modal_of(&plain).is_none());
        assert_eq!(announced_specs(&plain, &[]).len(), 2);
        assert_eq!(targets_before(&plain, &[], 1), 0);
        let game = game(false);
        assert!(announceable(&game, PlayerId(0), source(&game), &plain));
    }
}
