//! Goal: the rule options of a game behave as their parameters say: permanents untap in the untap steps of the players a rule names, the least maximum hand size in force limits the hand at cleanup, the legend rule stops for the players a rule names, and a trigger repeats only for the permanents the rule's selector picks.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::flow::Progress;
use crate::fold::apply_event;
use crate::setup::Start;
use crate::state::GameState;
use crate::triggers::detect;
use crate::turn::Step;

fn rule(json: &str) -> Ability {
    let text = format!(r#"{{"Rule":{{"rule":{json}}}}}"#);
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{json}: {error}"))
}

fn holder(name: &str, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Artifact)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    Arc::new(rules.into_iter().fold(card, CardDefinition::with_rules))
}

fn build(
    hands: Vec<Vec<Arc<CardDefinition>>>,
    boards: Vec<Vec<Arc<CardDefinition>>>,
    tapped: bool,
    active: usize,
    step: Step,
) -> GameState {
    let mut game = setup(hands, boards);
    for player in &mut game.players {
        for permanent in &mut player.battlefield {
            permanent.tapped = tapped;
        }
    }
    game.start = Start::At {
        turn: 3,
        active: PlayerId::from_index(active),
        step,
    };
    GameState::from_setup(&game).unwrap_or_else(|error| panic!("valid setup: {error}"))
}

fn name_of(state: &GameState, id: mtg_core::ids::ObjectId) -> String {
    state
        .object(id)
        .map_or_else(String::new, |o| o.card().name.clone())
}

#[rstest]
#[case::your_creatures_in_their_step(r#"[{"Type":"Creature"}]"#, "Opponents", 1, &["Muse", "Wolf"])]
#[case::only_creatures_with_a_type(r#"[{"Type":"Land"}]"#, "Opponents", 1, &[])]
#[case::not_in_your_own_step(r#"[{"Type":"Creature"}]"#, "Opponents", 0, &["Muse", "Wolf"])]
#[case::everyones_steps_include_yours(r#"[{"Type":"Creature"}]"#, "EachPlayer", 1, &["Muse", "Wolf"])]
#[case::only_your_step_leaves_theirs(r#"[{"Type":"Creature"}]"#, "You", 1, &[])]
fn permanents_untap_in_the_untap_steps_of_the_players_the_rule_names(
    #[case] what: &str,
    #[case] whose: &str,
    #[case] active: usize,
    #[case] untapped: &[&str],
) {
    let json = format!(r#"{{"UntapDuring":{{"what":{what},"whose":"{whose}"}}}}"#);
    let state = build(
        vec![vec![], vec![]],
        vec![
            vec![holder("Muse", vec![rule(&json)]), creature("Wolf", 2, 2)],
            vec![creature("Bear", 2, 2)],
        ],
        true,
        active,
        Step::Untap,
    );
    let Progress::Events(events) = crate::steps::next(&state) else {
        panic!("the untap step runs");
    };
    let mut found: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentUntapped { object, .. } => Some(name_of(&state, *object)),
            _ => None,
        })
        .filter(|name| name != "Bear")
        .collect();
    found.sort();
    assert_eq!(found, untapped.to_vec());
}

#[rstest]
#[case::default_limit_is_seven(&[], 9, 2)]
#[case::a_smaller_size(&[r#"{"MaxHandSize":{"who":"You","size":4}}"#], 9, 5)]
#[case::the_least_size_wins(&[r#"{"MaxHandSize":{"who":"You","size":4}}"#, r#"{"MaxHandSize":{"who":"You","size":2}}"#], 9, 7)]
#[case::a_larger_size(&[r#"{"MaxHandSize":{"who":"You","size":10}}"#], 9, 0)]
#[case::another_players_size_is_ignored(&[r#"{"MaxHandSize":{"who":"Opponents","size":1}}"#], 9, 2)]
#[case::no_maximum_beats_a_size(&[r#"{"MaxHandSize":{"who":"You","size":1}}"#, r#"{"NoMaxHandSize":"You"}"#], 9, 0)]
#[case::zero(&[r#"{"MaxHandSize":{"who":"You","size":0}}"#], 3, 3)]
fn the_maximum_hand_size_in_force_sets_how_many_cards_cleanup_discards(
    #[case] rules: &[&str],
    #[case] hand: usize,
    #[case] discards: u32,
) {
    let cards = (0..hand)
        .map(|n| creature(&format!("Card {n}"), 1, 1))
        .collect();
    let state = build(
        vec![cards, vec![]],
        vec![
            vec![holder("Holder", rules.iter().map(|r| rule(r)).collect())],
            vec![],
        ],
        false,
        0,
        Step::Cleanup,
    );
    match crate::steps::next(&state) {
        Progress::Decide(decision) => {
            let crate::decision::DecisionKind::Discard(options) = decision.kind else {
                panic!("cleanup asks for a discard");
            };
            assert_eq!(options.count, discards as usize);
        }
        _ => assert_eq!(discards, 0, "cleanup discards nothing"),
    }
}

fn legend(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_power_toughness(2, 2),
    )
}

#[rstest]
#[case::without_the_rule(None, 1)]
#[case::for_you(Some("You"), 0)]
#[case::for_everyone(Some("EachPlayer"), 0)]
#[case::for_opponents_only(Some("Opponents"), 1)]
fn the_legend_rule_stops_for_the_players_the_rule_names(
    #[case] who: Option<&str>,
    #[case] groups: usize,
) {
    let rules = who
        .map(|who| rule(&format!(r#"{{"NoLegendRule":"{who}"}}"#)))
        .into_iter()
        .collect();
    let state = build(
        vec![vec![], vec![]],
        vec![
            vec![holder("Gallery", rules), legend("Ana"), legend("Ana")],
            vec![],
        ],
        false,
        0,
        Step::Main1,
    );
    assert_eq!(crate::legend::groups(&state).len(), groups);
    assert_eq!(crate::legend::decision(&state).is_some(), groups == 1);
}

fn watcher(name: &str, types: &[CardType]) -> Arc<CardDefinition> {
    let upkeep: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let card = types.iter().fold(
        CardDefinition::new(name).with_power_toughness(1, 1),
        |card, kind| card.with_type(*kind),
    );
    Arc::new(card.with_rules(upkeep))
}

#[rstest]
#[case::every_permanent(None, 2, 2)]
#[case::only_artifacts(Some(r#"[{"Type":"Artifact"}]"#), 2, 1)]
#[case::nothing_that_matches(Some(r#"[{"Type":"Land"}]"#), 1, 1)]
fn a_trigger_repeats_for_the_permanents_the_rule_picks(
    #[case] source: Option<&str>,
    #[case] gear: usize,
    #[case] bear: usize,
) {
    let cause = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;
    let source = source.map_or_else(String::new, |s| format!(r#","source":{s}"#));
    let again = rule(&format!(
        r#"{{"TriggerAgain":{{"cause":{cause}{source}}}}}"#
    ));
    let state = build(
        vec![vec![], vec![]],
        vec![
            vec![
                holder("Holder", vec![again]),
                watcher("Gear", &[CardType::Artifact, CardType::Creature]),
                watcher("Bear", &[CardType::Creature]),
            ],
            vec![],
        ],
        false,
        0,
        Step::Main1,
    );
    let events = [Event::StepBegan { step: Step::Upkeep }];
    let mut after = state.clone();
    apply_event(&mut after, &events[0]).unwrap_or_else(|error| panic!("{error}"));
    let found = detect(&state, &events, &after);
    let by = |name: &str| {
        found
            .iter()
            .filter(|t| name_of(&state, t.source.id) == name)
            .count()
    };
    assert_eq!((by("Gear"), by("Bear")), (gear, bear));
}
