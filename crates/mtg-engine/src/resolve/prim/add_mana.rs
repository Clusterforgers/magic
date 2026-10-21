//! `Effect::AddMana`: mana goes into a player's pool (CR 106.4).

use mtg_cards::effect::{Effect, Produce};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use super::{unsupported, wrong_effect};
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
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
/// added as printed. "Any color" asks the player for a colour (`ChooseOption`) each time it is added, and every mana
/// of that addition has it ("three mana of any one color"); a choice with one option is not asked. The mana is
/// credited to the object that produced it while that object is in the game.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::AddMana { who, mana, times } = effect else {
        return Err(wrong_effect("AddMana"));
    };
    // A choice of mana other than any one color waits for the task that plays it; `Caps` names each by `Effect.AddMana.<kind>`.
    match mana {
        Produce::OneOf(_) => return unsupported("Effect.AddMana.OneOf"),
        Produce::ChosenColor(_) => return unsupported("Effect.AddMana.ChosenColor"),
        Produce::CommanderIdentity(_) => return unsupported("Effect.AddMana.CommanderIdentity"),
        Produce::ProducedBy { .. } => return unsupported("Effect.AddMana.ProducedBy"),
        Produce::Fixed(_) | Produce::AnyColor(_) => {}
    }
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
    match mana {
        Produce::Fixed(list) => {
            for player in &players {
                for _ in 0..times {
                    events.extend(list.iter().map(|&mana| Event::ManaAdded {
                        player: *player,
                        mana,
                        source,
                    }));
                }
            }
        }
        Produce::AnyColor(each) => {
            let options: Vec<ChoiceOption> =
                COLORS.iter().copied().map(ChoiceOption::Color).collect();
            let choosers: Vec<OptionChooser> = players
                .iter()
                .flat_map(|player| std::iter::repeat_n(*player, times))
                .map(|player| OptionChooser {
                    player,
                    options: options.clone(),
                    text: "Choose a color of mana.".into(),
                })
                .collect();
            let taken = match pick_options(cx, &choosers)? {
                Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
                Optioned::Done(taken) => taken,
            };
            for (player, index) in taken {
                let Some(color) = COLORS.get(index).copied() else {
                    return Err(ResolveError::Invalid(format!("no color is option {index}")));
                };
                events.extend(std::iter::repeat_n(
                    Event::ManaAdded {
                        player,
                        mana: mana_of(color),
                        source,
                    },
                    usize::try_from(*each).unwrap_or(0),
                ));
            }
        }
        Produce::OneOf(_)
        | Produce::ChosenColor(_)
        | Produce::CommanderIdentity(_)
        | Produce::ProducedBy { .. } => return Err(wrong_effect("AddMana")),
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: adding mana puts the printed mana in the pool of each player named, once for every time it is added; "any color" asks for a colour each time and adds every mana of that addition in it, a fixed mana asks nothing, and a count of nothing adds nothing.
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
    #[case::a_choice(
        r#"{"AddMana":{"mana":{"OneOf":["White","Blue"]}}}"#,
        "Effect.AddMana.OneOf"
    )]
    #[case::the_chosen_color(
        r#"{"AddMana":{"mana":{"ChosenColor":1}}}"#,
        "Effect.AddMana.ChosenColor"
    )]
    #[case::the_commanders_identity(
        r#"{"AddMana":{"mana":{"CommanderIdentity":1}}}"#,
        "Effect.AddMana.CommanderIdentity"
    )]
    #[case::what_a_land_could_produce(
        r#"{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"}],"count":1}}}}"#,
        "Effect.AddMana.ProducedBy"
    )]
    fn a_choice_of_mana_other_than_any_color_refuses_by_its_own_name(
        #[case] json: &str,
        #[case] name: &'static str,
    ) {
        assert_eq!(
            run_with(&game(), Vec::new(), &effect(json)).unwrap_err(),
            ResolveError::Unsupported(name)
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
                source: Some(source)
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
