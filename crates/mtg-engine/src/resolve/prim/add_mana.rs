//! `Effect::AddMana`: mana goes into a player's pool (CR 106.4).

use mtg_cards::effect::{Effect, Produce};
use mtg_cards::spend::SpendOn;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use super::{unsupported, wrong_effect};
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::mana_kinds::{identity_types, producible_types};
use crate::object::{Chosen, GameObject, PermanentState};
use crate::resolve::{Cx, ResolveError, Run};

/// The colours "any color" offers, in the order of the five-colour wheel.
const COLORS: [Color; 5] = [
    Color::White,
    Color::Blue,
    Color::Black,
    Color::Red,
    Color::Green,
];

/// The mana of a colour.
const fn mana_of(color: Color) -> ManaType {
    match color {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

/// Each player `who` names who is still in the game adds the mana `times` times to their pool. A fixed mana is
/// added as printed. "Any color" and a choice between types ("{W} or {U}") ask the player (`ChooseOption`) each time
/// the mana is added, and every mana of that addition has the type chosen ("three mana of any one color"); a choice
/// with one option is not asked. A choice between lists of mana ("{W}{W}, {W}{U}, or {U}{U}") asks the same way and adds
/// every mana of the list chosen. The chosen color is the one the source's controller chose as it entered, and
/// none is added while there is none. The mana is credited to the object that produced it while that object is in
/// the game.
#[allow(clippy::too_many_lines)]
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::AddMana {
        who,
        mana,
        times,
        only,
    } = effect
    else {
        return Err(wrong_effect("AddMana"));
    };
    let state = cx.state();
    let times = usize::try_from(cx.scope.amount(times)?.max(0)).unwrap_or(0);
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    let source = Some(cx.scope.source.id).filter(|id| state.object(*id).is_some());
    let mut events = Vec::new();
    // The types a player picks between and how many mana of the one picked.
    let (kinds, each): (Vec<ManaType>, u32) = match mana {
        Produce::Fixed(list) => {
            for player in &players {
                for _ in 0..times {
                    events.extend(list.iter().map(|&mana| Event::ManaAdded {
                        player: *player,
                        mana,
                        source,
                        only: only.clone(),
                    }));
                }
            }
            return Ok(Run::events(events));
        }
        Produce::ChosenColor(each) => {
            let chosen = source
                .and_then(|id| state.object(id))
                .and_then(GameObject::permanent)
                .and_then(PermanentState::chosen);
            if let Some(Chosen::Color(color)) = chosen {
                for player in &players {
                    events.extend(
                        std::iter::repeat_n(
                            Event::ManaAdded {
                                player: *player,
                                mana: mana_of(*color),
                                source,
                                only: only.clone(),
                            },
                            usize::try_from(*each).unwrap_or(0).saturating_mul(times),
                        )
                        .collect::<Vec<_>>(),
                    );
                }
            }
            return Ok(Run::events(events));
        }
        Produce::AnyColor(each) => (COLORS.iter().copied().map(mana_of).collect(), *each),
        Produce::OrChosenColor(printed) => {
            let chosen = source
                .and_then(|id| state.object(id))
                .and_then(GameObject::permanent)
                .and_then(PermanentState::chosen);
            let mut types = vec![*printed];
            if let Some(Chosen::Color(color)) = chosen
                && mana_of(*color) != *printed
            {
                types.push(mana_of(*color));
            }
            (types, 1)
        }
        Produce::OneOf(types) => (types.clone(), 1),
        Produce::OneOfLists(lists) => {
            let added = Added {
                players: &players,
                times,
                source,
                only,
            };
            return choose_list(cx, &added, lists);
        }
        // What depends on the game is read for the one player who adds it; none is added when the list is empty.
        Produce::CommanderIdentity(each) => match players.as_slice() {
            [player] => (identity_types(state, *player), *each),
            _ => return unsupported("Effect.AddMana.CommanderIdentity.Players"),
        },
        Produce::ProducedBy {
            among,
            count,
            colorless,
        } => match players.as_slice() {
            [player] => (
                producible_types(
                    state,
                    *player,
                    (cx.scope.source.id, None),
                    among,
                    *colorless,
                ),
                *count,
            ),
            _ => return unsupported("Effect.AddMana.ProducedBy.Players"),
        },
    };
    if kinds.is_empty() {
        return Ok(Run::events(events));
    }
    let options: Vec<ChoiceOption> = match mana {
        Produce::OneOf(_) => kinds
            .iter()
            .map(|kind| ChoiceOption::Label(format!("{{{}}}", kind.symbol())))
            .collect(),
        Produce::ProducedBy { .. } | Produce::OrChosenColor(_) => kinds
            .iter()
            .map(|kind| match kind.color() {
                Some(color) => ChoiceOption::Color(color),
                None => ChoiceOption::Label(format!("{{{}}}", kind.symbol())),
            })
            .collect(),
        Produce::CommanderIdentity(_) => kinds
            .iter()
            .filter_map(|kind| kind.color().map(ChoiceOption::Color))
            .collect(),
        _ => COLORS.iter().copied().map(ChoiceOption::Color).collect(),
    };
    let choosers: Vec<OptionChooser> = players
        .iter()
        .flat_map(|player| std::iter::repeat_n(*player, times))
        .map(|player| OptionChooser {
            player,
            options: options.clone(),
            text: if matches!(mana, Produce::OneOf(_) | Produce::OrChosenColor(_)) {
                "Choose a type of mana.".into()
            } else {
                "Choose a color of mana.".into()
            },
        })
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    for (player, index) in taken {
        let Some(kind) = kinds.get(index).copied() else {
            return Err(ResolveError::Invalid(format!(
                "no type of mana is option {index}"
            )));
        };
        events.extend(std::iter::repeat_n(
            Event::ManaAdded {
                player,
                mana: kind,
                source,
                only: only.clone(),
            },
            usize::try_from(each).unwrap_or(0),
        ));
    }
    Ok(Run::events(events))
}

/// Who adds mana, how many times, and from what.
struct Added<'a> {
    players: &'a [PlayerId],
    times: usize,
    source: Option<ObjectId>,
    only: &'a [SpendOn],
}

/// Each addition asks its player to pick one of `lists` ("Add {W}{W}, {W}{U}, or {U}{U}") and adds all the mana of the
/// list picked; one list is not asked.
fn choose_list(
    cx: &Cx<'_>,
    added: &Added<'_>,
    lists: &[Vec<ManaType>],
) -> Result<Run, ResolveError> {
    let options: Vec<ChoiceOption> = lists
        .iter()
        .map(|list| {
            let mut label = String::new();
            for kind in list {
                label.push('{');
                label.push(kind.symbol());
                label.push('}');
            }
            ChoiceOption::Label(label)
        })
        .collect();
    let choosers: Vec<OptionChooser> = added
        .players
        .iter()
        .flat_map(|player| std::iter::repeat_n(*player, added.times))
        .map(|player| OptionChooser {
            player,
            options: options.clone(),
            text: "Choose the mana to add.".into(),
        })
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let mut events = Vec::new();
    for (player, index) in taken {
        let Some(list) = lists.get(index) else {
            return Err(ResolveError::Invalid(format!(
                "no list of mana is option {index}"
            )));
        };
        events.extend(list.iter().map(|&mana| Event::ManaAdded {
            player,
            mana,
            source: added.source,
            only: added.only.to_vec(),
        }));
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: adding mana puts the printed mana in the pool of each player named, once for every time it is added; "any color" asks for a colour each time and adds every mana of that addition in it, a fixed mana asks nothing, a choice between lists of mana asks each time and adds every mana of the list chosen, and a count of nothing adds nothing.
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// The mana a finished run adds, as (player, mana).
    fn added(run: Run) -> Vec<(PlayerId, ManaType)> {
        let Run::Done { events, .. } = run else {
            panic!("the effect is done");
        };
        events
            .into_iter()
            .map(|event| match event {
                Event::ManaAdded { player, mana, .. } => (player, mana),
                other => panic!("only mana was expected, got {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::one_green(r#"{"AddMana":{"mana":{"Fixed":["Green"]}}}"#, &[(ME, ManaType::Green)])]
    #[case::two_colorless(r#"{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}"#, &[(ME, ManaType::Colorless), (ME, ManaType::Colorless)])]
    #[case::three_times(r#"{"AddMana":{"mana":{"Fixed":["Red"]},"times":3}}"#, &[(ME, ManaType::Red), (ME, ManaType::Red), (ME, ManaType::Red)])]
    #[case::each_time_the_whole_list(r#"{"AddMana":{"mana":{"Fixed":["Black","Red"]},"times":2}}"#, &[(ME, ManaType::Black), (ME, ManaType::Red), (ME, ManaType::Black), (ME, ManaType::Red)])]
    #[case::for_an_opponent(r#"{"AddMana":{"who":"Opponents","mana":{"Fixed":["Blue"]}}}"#, &[(FOE, ManaType::Blue)])]
    #[case::counting_creatures(r#"{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"}]}}}"#, &[(ME, ManaType::Green)])]
    #[case::nothing(r#"{"AddMana":{"mana":{"Fixed":["Green"]},"times":0}}"#, &[])]
    fn a_fixed_mana_is_added_as_printed_without_a_question(
        #[case] json: &str,
        #[case] expected: &[(PlayerId, ManaType)],
    ) {
        assert_eq!(
            added(run_with(&game(), Vec::new(), &effect(json)).unwrap()),
            expected
        );
    }

    #[rstest]
    #[case::the_commanders_identity(r#"{"AddMana":{"mana":{"CommanderIdentity":1}}}"#)]
    #[case::what_a_land_could_produce(
        r#"{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"}],"count":1}}}}"#
    )]
    #[case::any_type_a_land_could_produce(
        r#"{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"}],"count":1,"colorless":true}}}}"#
    )]
    fn mana_that_needs_a_commander_or_a_land_adds_none_without_one(#[case] json: &str) {
        assert_eq!(
            added(run_with(&game(), Vec::new(), &effect(json)).unwrap()),
            []
        );
    }

    #[test]
    fn the_mana_is_credited_to_the_source_that_is_in_the_game() {
        let game = game();
        let fixed = effect(r#"{"AddMana":{"mana":{"Fixed":["Green"]}}}"#);
        let Run::Done { events, .. } = run_with(&game, Vec::new(), &fixed).unwrap() else {
            panic!("done");
        };
        let source = game.zone(Zone::Battlefield)[0];
        assert_eq!(
            events,
            [Event::ManaAdded {
                player: ME,
                mana: ManaType::Green,
                source: Some(source),
                only: Vec::new()
            }]
        );
    }

    #[test]
    fn any_color_asks_for_a_color_and_adds_it() {
        let game = game();
        let any = effect(r#"{"AddMana":{"mana":{"AnyColor":1}}}"#);
        let Run::Ask(decision) = run_with(&game, Vec::new(), &any).unwrap() else {
            panic!("the player chooses");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option");
        };
        assert_eq!(
            options.options,
            COLORS.map(ChoiceOption::Color).to_vec(),
            "all five colours, white first"
        );
        for (index, expected) in [
            ManaType::White,
            ManaType::Blue,
            ManaType::Black,
            ManaType::Red,
            ManaType::Green,
        ]
        .into_iter()
        .enumerate()
        {
            let run = run_with(&game, vec![(ME, Choice::Option(index))], &any).unwrap();
            assert_eq!(added(run), [(ME, expected)]);
        }
    }

    #[test]
    fn several_mana_of_any_one_color_share_one_choice() {
        let game = game();
        let three = effect(r#"{"AddMana":{"mana":{"AnyColor":3}}}"#);
        assert!(matches!(
            run_with(&game, Vec::new(), &three).unwrap(),
            Run::Ask(_)
        ));
        let run = run_with(&game, vec![(ME, Choice::Option(3))], &three).unwrap();
        assert_eq!(added(run), [(ME, ManaType::Red); 3]);
    }

    #[test]
    fn each_time_it_is_added_a_color_is_chosen() {
        let game = game();
        let twice = effect(r#"{"AddMana":{"mana":{"AnyColor":1},"times":2}}"#);
        assert!(matches!(
            run_with(&game, Vec::new(), &twice).unwrap(),
            Run::Ask(_)
        ));
        assert!(matches!(
            run_with(&game, vec![(ME, Choice::Option(0))], &twice).unwrap(),
            Run::Ask(_)
        ));
        let run = run_with(
            &game,
            vec![(ME, Choice::Option(0)), (ME, Choice::Option(4))],
            &twice,
        )
        .unwrap();
        assert_eq!(added(run), [(ME, ManaType::White), (ME, ManaType::Green)]);
    }

    #[rstest]
    #[case::first(0, ManaType::White)]
    #[case::second(1, ManaType::Blue)]
    #[case::third(2, ManaType::Colorless)]
    fn a_choice_between_types_asks_once_and_adds_the_type_chosen(
        #[case] index: usize,
        #[case] expected: ManaType,
    ) {
        let game = game();
        let one_of = effect(r#"{"AddMana":{"mana":{"OneOf":["White","Blue","Colorless"]}}}"#);
        let Run::Ask(decision) = run_with(&game, Vec::new(), &one_of).unwrap() else {
            panic!("the player chooses");
        };
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option");
        };
        assert_eq!(
            options.options,
            ["{W}", "{U}", "{C}"].map(|label| ChoiceOption::Label(label.into()))
        );
        let run = run_with(&game, vec![(ME, Choice::Option(index))], &one_of).unwrap();
        assert_eq!(added(run), [(ME, expected)]);
    }

    const LISTS: &str = r#"{"OneOfLists":[["White","White"],["White","Blue"],["Blue","Blue"]]}"#;

    /// The effect of `LISTS` with more fields after the mana (`,"times":2`).
    fn filter(more: &str) -> Effect {
        effect(&format!(r#"{{"AddMana":{{"mana":{LISTS}{more}}}}}"#))
    }

    #[rstest]
    #[case::two_of_the_first(0, &[ManaType::White, ManaType::White])]
    #[case::one_of_each(1, &[ManaType::White, ManaType::Blue])]
    #[case::two_of_the_last(2, &[ManaType::Blue, ManaType::Blue])]
    fn a_choice_between_lists_asks_once_and_adds_every_mana_of_the_list_chosen(
        #[case] index: usize,
        #[case] expected: &[ManaType],
    ) {
        let game = game();
        let lists = filter("");
        let Run::Ask(decision) = run_with(&game, Vec::new(), &lists).unwrap() else {
            panic!("the player chooses");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option");
        };
        assert_eq!(
            options.options,
            ["{W}{W}", "{W}{U}", "{U}{U}"].map(|label| ChoiceOption::Label(label.into()))
        );
        let run = run_with(&game, vec![(ME, Choice::Option(index))], &lists).unwrap();
        assert_eq!(
            added(run),
            expected.iter().map(|mana| (ME, *mana)).collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::once(1, &[0], &[ManaType::White, ManaType::White])]
    #[case::twice_with_the_same_list(2, &[2, 2], &[ManaType::Blue; 4])]
    #[case::twice_with_two_lists(2, &[0, 1], &[ManaType::White, ManaType::White, ManaType::White, ManaType::Blue])]
    fn each_time_it_is_added_a_list_is_chosen(
        #[case] times: i32,
        #[case] picks: &[usize],
        #[case] expected: &[ManaType],
    ) {
        let game = game();
        let lists = filter(&format!(r#","times":{times}"#));
        let mut journal = Vec::new();
        for pick in picks {
            assert!(matches!(
                run_with(&game, journal.clone(), &lists).unwrap(),
                Run::Ask(_)
            ));
            journal.push((ME, Choice::Option(*pick)));
        }
        let run = run_with(&game, journal, &lists).unwrap();
        assert_eq!(
            added(run),
            expected.iter().map(|mana| (ME, *mana)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_choice_with_one_list_is_not_asked_and_a_list_that_is_not_offered_is_refused() {
        let game = game();
        let one = effect(r#"{"AddMana":{"mana":{"OneOfLists":[["Red","Green"]]}}}"#);
        assert_eq!(
            added(run_with(&game, Vec::new(), &one).unwrap()),
            [(ME, ManaType::Red), (ME, ManaType::Green)]
        );
        let error = run_with(&game, vec![(ME, Choice::Option(3))], &filter("")).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)));
    }

    #[test]
    fn the_mana_of_a_list_keeps_its_source_and_its_restriction() {
        let game = game();
        let restricted = filter(r#","only":[{"Spell":{"types":["Creature"]}}]"#);
        let Run::Done { events, .. } =
            run_with(&game, vec![(ME, Choice::Option(1))], &restricted).unwrap()
        else {
            panic!("done");
        };
        let source = game.zone(Zone::Battlefield)[0];
        assert_eq!(events.len(), 2);
        for event in events {
            let Event::ManaAdded {
                source: from, only, ..
            } = event
            else {
                panic!("mana was added");
            };
            assert_eq!((from, only.len()), (Some(source), 1));
        }
    }

    #[test]
    fn mana_of_the_chosen_color_is_added_only_when_a_color_was_chosen() {
        let chosen = effect(r#"{"AddMana":{"mana":{"ChosenColor":2},"times":2}}"#);
        let mut game = game();
        let id = game.zone(Zone::Battlefield)[0];
        assert_eq!(added(run_with(&game, Vec::new(), &chosen).unwrap()), []);
        let entered = Event::Entered {
            object: id,
            tapped: false,
            controller: ME,
            chosen: Some(Chosen::Color(Color::Red)),
        };
        crate::fold::apply_event(&mut game, &entered).unwrap();
        assert_eq!(
            added(run_with(&game, Vec::new(), &chosen).unwrap()),
            [(ME, ManaType::Red); 4]
        );
    }

    #[test]
    fn a_count_of_nothing_asks_nothing() {
        let none = effect(r#"{"AddMana":{"mana":{"AnyColor":1},"times":0}}"#);
        assert_eq!(added(run_with(&game(), Vec::new(), &none).unwrap()), []);
    }

    #[test]
    fn an_answer_that_names_no_color_is_refused() {
        let any = effect(r#"{"AddMana":{"mana":{"AnyColor":1}}}"#);
        let error = run_with(&game(), vec![(ME, Choice::Option(9))], &any).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)));
    }
}
