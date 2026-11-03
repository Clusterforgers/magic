//! `Effect::ChooseValue`: a player chooses a colour, a type, a card name, a number or one of some words, and the
//! resolution remembers the choice under a name for `Pred::ChosenAs` and `Amount::ChosenAs`.

use std::collections::BTreeSet;

use mtg_cards::effect::{Effect, ValueKind};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::values;
use crate::choose::{Choice, ChoiceOption, NumberOptions, OptionChooser, Optioned, pick_options};
use crate::decision::{Decision, DecisionKind};
use crate::object::Chosen;
use crate::resolve::{CHOSEN_VALUE, Cx, Next, ResolveError, Run};

/// The player `who` names (exactly one) chooses what `kind` asks for. A choice with one option is taken without a
/// question, one with none binds nothing, and a number is bounded by its two amounts (the larger of the two when they
/// are reversed).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChooseValue { who, kind, bind } = effect else {
        return Err(wrong_effect("ChooseValue"));
    };
    let choosers: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .collect();
    let player = match choosers.as_slice() {
        [] => return Ok(done(None, bind)),
        [player] => *player,
        _ => {
            return Err(ResolveError::Invalid(
                "a value is chosen by one player".into(),
            ));
        }
    };
    match choice(cx, player, kind)? {
        Ok(chosen) => Ok(done(chosen, bind)),
        Err(decision) => Ok(Run::Ask(decision)),
    }
}

/// The value the player chose, or the question that is still to be answered.
fn choice(
    cx: &Cx<'_>,
    player: PlayerId,
    kind: &ValueKind,
) -> Result<Result<Option<Chosen>, Decision>, ResolveError> {
    if let ValueKind::Number(low, high) = kind {
        let low = u32::try_from(cx.scope.amount(low)?.max(0)).unwrap_or(u32::MAX);
        let high = u32::try_from(cx.scope.amount(high)?.max(0)).unwrap_or(u32::MAX);
        return Ok(number(cx, player, low, high.max(low))?.map(|n| Some(Chosen::Number(n))));
    }
    let options = options(cx, kind)?;
    let chooser = OptionChooser {
        player,
        text: question(kind),
        options: options.clone(),
    };
    Ok(match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => Err(decision),
        Optioned::Done(taken) => Ok(taken
            .first()
            .and_then(|(_, index)| options.get(*index))
            .and_then(|option| value(kind, option))),
    })
}

/// The step that remembers the value, if there is one.
fn done(chosen: Option<Chosen>, bind: &str) -> Run {
    let bound = chosen
        .and_then(|value| serde_json::to_string(&value).ok())
        .map(|json| (format!("{CHOSEN_VALUE}{bind}\t{json}"), Vec::new()))
        .into_iter()
        .collect();
    Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    }
}

/// The number the player chose, read from the journal, or the question for it; a single allowed number is not asked.
fn number(
    cx: &Cx<'_>,
    player: PlayerId,
    low: u32,
    high: u32,
) -> Result<Result<u32, Decision>, ResolveError> {
    if low == high {
        return Ok(Ok(low));
    }
    match cx.journal() {
        [] => Ok(Err(Decision {
            player,
            kind: DecisionKind::ChooseNumber(NumberOptions {
                source: cx.scope.source.id,
                min: low,
                max: Some(high),
                except: Vec::new(),
                text: "Choose a number.".to_owned(),
                can_cancel: false,
                cost: None,
                per_x: 0,
            }),
        })),
        [(who, Choice::Number(number))] if *who == player && (low..=high).contains(number) => {
            Ok(Ok(*number))
        }
        other => Err(ResolveError::Invalid(format!(
            "{other:?} is no number from {low} to {high} chosen by {player:?}"
        ))),
    }
}

/// What the player chooses between.
fn options(cx: &Cx<'_>, kind: &ValueKind) -> Result<Vec<ChoiceOption>, ResolveError> {
    Ok(match kind {
        ValueKind::Color => Color::ALL
            .iter()
            .copied()
            .map(ChoiceOption::Color)
            .collect(),
        ValueKind::CreatureType => values::creature_types(cx.state()),
        ValueKind::CardType => values::card_types(),
        ValueKind::LandType => values::land_types(),
        ValueKind::CardName(filter) => {
            let mut fits = BTreeSet::new();
            for object in cx.state().objects() {
                if cx.scope.selects(filter, object.reference())? {
                    fits.insert(object.id);
                }
            }
            values::card_names(cx.state(), |object| fits.contains(&object.id))
        }
        ValueKind::Options(words) => words.iter().cloned().map(ChoiceOption::Label).collect(),
        ValueKind::Number(..) => Vec::new(),
    })
}

/// The question in the words of a card.
fn question(kind: &ValueKind) -> String {
    match kind {
        ValueKind::Color => "Choose a color.".to_owned(),
        ValueKind::CreatureType => "Choose a creature type.".to_owned(),
        ValueKind::CardType => "Choose a card type.".to_owned(),
        ValueKind::LandType => "Choose a land type.".to_owned(),
        ValueKind::CardName(_) => "Choose a card name.".to_owned(),
        ValueKind::Options(words) => format!("Choose {}.", words.join(" or ")),
        ValueKind::Number(..) => "Choose a number.".to_owned(),
    }
}

/// The value an option stands for.
fn value(kind: &ValueKind, option: &ChoiceOption) -> Option<Chosen> {
    match (kind, option) {
        (ValueKind::Color, ChoiceOption::Color(color)) => Some(Chosen::Color(*color)),
        (ValueKind::CreatureType, ChoiceOption::Label(name)) => {
            Some(Chosen::CreatureType(Subtype(name.clone())))
        }
        (ValueKind::LandType, ChoiceOption::Label(name)) => {
            Some(Chosen::LandType(Subtype(name.clone())))
        }
        (ValueKind::CardType, ChoiceOption::Label(name)) => values::CARD_TYPES
            .into_iter()
            .find(|card_type: &CardType| format!("{card_type:?}") == *name)
            .map(Chosen::CardType),
        (ValueKind::CardName(_), ChoiceOption::Label(name)) => Some(Chosen::Name(name.clone())),
        (ValueKind::Options(_), ChoiceOption::Label(word)) => Some(Chosen::Word(word.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a chosen value is asked of exactly one player with the options its kind offers (colours, card and land types, the names of the cards in the game the filter accepts, the words of the card, a number in its bounds), a choice with one option is not asked, the answer is read from the journal and remembered under the name as the value its option stands for, and an answer outside the options is refused.
    use rstest::rstest;

    use mtg_core::card::LAND_TYPES;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::Resolving;
    use crate::resolve::Scope;
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![creature("A", 1, 1), creature("B", 1, 1)], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn run_with(journal: Vec<(PlayerId, Choice)>, json: &str) -> Result<Run, ResolveError> {
        let game = game();
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &effect)
    }

    /// The value a finished step remembers under `hue`, if it remembers one.
    fn remembered(run: Run) -> Option<Chosen> {
        let Run::Done { bound, .. } = run else {
            panic!("the step was to be done: {run:?}");
        };
        bound.first().map(|(name, targets)| {
            assert_eq!(targets.as_slice(), []);
            let json = name.strip_prefix("~value:hue\t").unwrap();
            serde_json::from_str(json).unwrap()
        })
    }

    fn kind(json: &str) -> String {
        format!(r#"{{"ChooseValue":{{"kind":{json},"as":"hue"}}}}"#)
    }

    #[rstest]
    #[case::colors(r#""Color""#, 5, "Choose a color.")]
    #[case::card_types(r#""CardType""#, 9, "Choose a card type.")]
    #[case::land_types(r#""LandType""#, 17, "Choose a land type.")]
    #[case::every_card_name(r#"{"CardName":[]}"#, 8, "Choose a card name.")]
    #[case::names_in_a_hand(r#"{"CardName":[{"In":"Hand"}]}"#, 2, "Choose a card name.")]
    #[case::words(r#"{"Options":["odd","even"]}"#, 2, "Choose odd or even.")]
    fn the_player_is_asked_between_the_options_of_the_kind(
        #[case] json: &str,
        #[case] count: usize,
        #[case] text: &str,
    ) {
        let Run::Ask(decision) = run_with(vec![], &kind(json)).unwrap() else {
            panic!("the controller is asked");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option was expected");
        };
        assert_eq!(
            (options.options.len(), options.text.as_str()),
            (count, text)
        );
    }

    #[rstest]
    #[case::a_color(r#""Color""#, 1, Chosen::Color(Color::ALL[1]))]
    #[case::a_land_type(r#""LandType""#, 0, Chosen::LandType(Subtype(LAND_TYPES[0].to_owned())))]
    #[case::a_card_type(r#""CardType""#, 2, Chosen::CardType(CardType::Creature))]
    #[case::a_name(r#"{"CardName":[{"In":"Hand"}]}"#, 1, Chosen::Name("B".to_owned()))]
    #[case::a_word(r#"{"Options":["odd","even"]}"#, 1, Chosen::Word("even".to_owned()))]
    fn the_answer_is_remembered_as_the_value_of_its_option(
        #[case] json: &str,
        #[case] index: usize,
        #[case] expected: Chosen,
    ) {
        let journal = vec![(ME, Choice::Option(index))];
        let done = run_with(journal, &kind(json)).unwrap();
        assert_eq!(remembered(done), Some(expected));
    }

    #[rstest]
    #[case::inside(r#"{"Number":[1,5]}"#, 3, Some(Chosen::Number(3)))]
    #[case::lowest(r#"{"Number":[0,2]}"#, 0, Some(Chosen::Number(0)))]
    #[case::an_amount_bounds_it(r#"{"Number":[1,{"Sum":[2,3]}]}"#, 3, Some(Chosen::Number(3)))]
    #[case::above(r#"{"Number":[1,5]}"#, 6, None)]
    #[case::below(r#"{"Number":[2,5]}"#, 1, None)]
    fn a_number_is_held_to_its_bounds(
        #[case] json: &str,
        #[case] number: u32,
        #[case] expected: Option<Chosen>,
    ) {
        let asked = run_with(vec![], &kind(json)).unwrap();
        assert!(matches!(
            asked,
            Run::Ask(Decision {
                kind: DecisionKind::ChooseNumber(_),
                ..
            })
        ));
        let answered = run_with(vec![(ME, Choice::Number(number))], &kind(json));
        match expected {
            Some(_) => assert_eq!(remembered(answered.unwrap()), expected),
            None => assert!(answered.is_err()),
        }
    }

    #[rstest]
    #[case::one_number(r#"{"Number":[4,4]}"#, Some(Chosen::Number(4)))]
    #[case::reversed_bounds_take_the_lower(r#"{"Number":[4,2]}"#, Some(Chosen::Number(4)))]
    #[case::one_word(r#"{"Options":["only"]}"#, Some(Chosen::Word("only".to_owned())))]
    #[case::no_card_name(r#"{"CardName":[{"In":"Exile"}]}"#, None)]
    fn a_choice_with_one_option_or_none_is_not_asked(
        #[case] json: &str,
        #[case] expected: Option<Chosen>,
    ) {
        let done = run_with(vec![], &kind(json)).unwrap();
        assert_eq!(remembered(done), expected);
    }

    #[rstest]
    #[case::an_index_past_the_options(vec![(ME, Choice::Option(9))])]
    #[case::another_player(vec![(PlayerId(1), Choice::Option(0))])]
    #[case::a_number_for_options(vec![(ME, Choice::Number(1))])]
    fn an_answer_outside_the_options_is_refused(#[case] journal: Vec<(PlayerId, Choice)>) {
        assert!(run_with(journal, &kind(r#""Color""#)).is_err());
    }

    #[test]
    fn several_choosers_are_refused_and_nobody_chooses_nothing() {
        let each = r#"{"ChooseValue":{"who":"EachPlayer","kind":"Color","as":"hue"}}"#;
        assert!(run_with(vec![], each).is_err());
        let nobody = r#"{"ChooseValue":{"who":{"Target":0},"kind":"Color","as":"hue"}}"#;
        assert_eq!(remembered(run_with(vec![], nobody).unwrap()), None);
    }
}
