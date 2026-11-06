//! Goal: a replacement of the mana of a tap (CR 106.6, 614.1) changes only the permanents it selects and only for the players it names, in type, in amount or in one color, applies to every way to tap a permanent for mana, leaves taps it does not cover alone, and shows in the options a payment plans with and in the mana the activation adds.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType::{
    self, Black as B, Colorless as C, Green as G, Red as R, White as W,
};
use rstest::rstest;

use crate::events::Event;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::mana::ManaOption;
use crate::mana_ability::{activation, options};
use crate::state::GameState;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn land(color: Color) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(color)))
}

/// An enchantment with the rule "mana produced: <change>" for the permanents `what` selects and the players `who`.
fn replacement(what: &str, who: &str, change: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Rule":{{"rule":{{"ManaProduced":{{"what":{what},"who":"{who}","change":{change}}}}}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Replacer")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// An artifact that taps for two white mana: a tap that adds more than one.
fn sol_ring() -> Arc<CardDefinition> {
    let json = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Colorless"]}}}]}}"#;
    Arc::new(
        CardDefinition::new("Ring")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

const LAND: &str = r#"[{"Type":"Land"}]"#;

/// ME has a Forest, a Swamp, a Ring and the replacer; FOE has a Forest.
fn board(replacer: Arc<CardDefinition>) -> GameState {
    state(
        vec![vec![], vec![]],
        vec![
            vec![land(Color::Green), land(Color::Black), sol_ring(), replacer],
            vec![land(Color::Green)],
        ],
    )
}

fn made(state: &GameState, player: PlayerId) -> Vec<Vec<ManaType>> {
    options(state, player)
        .iter()
        .map(ManaOption::made)
        .collect()
}

#[rstest]
#[case::black_instead_of_any_amount(LAND, "EachPlayer", r#"{"One":"Black"}"#, vec![vec![B], vec![B], vec![W, C]], vec![vec![B]])]
#[case::black_in_the_same_amount(LAND, "EachPlayer", r#"{"OnlyType":"Black"}"#, vec![vec![B], vec![B], vec![W, C]], vec![vec![B]])]
#[case::only_your_taps(LAND, "You", r#"{"OnlyType":"Red"}"#, vec![vec![R], vec![R], vec![W, C]], vec![vec![G]])]
#[case::only_the_permanents_selected(r#"[{"Subtype":"Forest"}]"#, "EachPlayer", r#"{"OnlyType":"Red"}"#, vec![vec![R], vec![B], vec![W, C]], vec![vec![R]])]
#[case::any_permanent_in_the_same_type(r"[]", "You", r#"{"OnlyType":"Red"}"#, vec![vec![R], vec![R], vec![R, R]], vec![vec![G]])]
#[case::twice_as_much(r"[]", "You", r#"{"Times":2}"#, vec![vec![G, G], vec![B, B], vec![W, W, C, C]], vec![vec![G]])]
#[case::three_times_for_lands(r#"[{"Type":"Land"}]"#, "You", r#"{"Times":3}"#, vec![vec![G, G, G], vec![B, B, B], vec![W, C]], vec![vec![G]])]
#[case::one_color_becomes_another(r"[]", "You", r#"{"Convert":{"from":"Green","to":"Colorless"}}"#, vec![vec![C], vec![B], vec![W, C]], vec![vec![G]])]
fn a_replacement_changes_the_mana_of_the_taps_it_covers(
    #[case] what: &str,
    #[case] who: &str,
    #[case] change: &str,
    #[case] mine: Vec<Vec<ManaType>>,
    #[case] theirs: Vec<Vec<ManaType>>,
) {
    let game = board(replacement(what, who, change));
    assert_eq!(made(&game, ME), mine);
    assert_eq!(made(&game, FOE), theirs);
}

#[test]
fn the_variants_of_one_ability_stay_sources_of_their_own() {
    let painland = {
        let json = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}}]}}"#;
        Arc::new(
            CardDefinition::new("Pain")
                .with_type(CardType::Land)
                .with_rules(serde_json::from_str(json).unwrap()),
        )
    };
    let game = state(
        vec![vec![], vec![]],
        vec![
            vec![painland, replacement(LAND, "You", r#"{"One":"Black"}"#)],
            vec![],
        ],
    );
    assert_eq!(
        made(&game, ME),
        [vec![B], vec![B]],
        "white or blue is black either way"
    );
}

#[test]
fn the_activation_adds_the_replaced_mana_and_nothing_else() {
    let mut game = board(replacement(LAND, "EachPlayer", r#"{"One":"Black"}"#));
    let option = options(&game, ME)
        .into_iter()
        .find(|option| option.source.object == game.zone(crate::zones::Zone::Battlefield)[0])
        .unwrap();
    let events = activation(&game, ME, &option, &[]).unwrap();
    for event in &events {
        apply_event(&mut game, event).unwrap();
    }
    let added: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(added, [B]);
}

#[test]
fn a_replacer_that_left_play_changes_nothing() {
    let game = state(vec![vec![], vec![]], vec![vec![land(Color::Green)], vec![]]);
    assert_eq!(made(&game, ME), [vec![G]]);
}

/// A game of ME with the given permanents.
fn mine(permanents: Vec<Arc<CardDefinition>>) -> GameState {
    state(vec![vec![], vec![]], vec![permanents, vec![]])
}

/// An artifact that sacrifices itself for one colorless mana: an ability that does not tap (Eldrazi Spawn).
fn spawn() -> Arc<CardDefinition> {
    let json = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#;
    Arc::new(
        CardDefinition::new("Spawn")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

/// An enchantment with "whenever you tap a land for mana, add {R}" (Gauntlet of Might style, CR 605.1b).
fn extra_red() -> Arc<CardDefinition> {
    let json = r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"who":"You","mana":{"Fixed":["Red"]}}}]}}"#;
    Arc::new(
        CardDefinition::new("Extra")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

fn sorted(mut all: Vec<Vec<ManaType>>) -> Vec<Vec<ManaType>> {
    for mana in &mut all {
        mana.sort_by_key(|m| format!("{m:?}"));
    }
    all
}

#[test]
fn an_ability_that_does_not_tap_is_no_tap_for_mana() {
    // CR 106.6: a replacement covers mana produced as a permanent is tapped for mana only.
    let game = mine(vec![spawn(), replacement("[]", "You", r#"{"Times":2}"#)]);
    assert_eq!(made(&game, ME), [vec![C]]);
}

#[rstest]
#[case::doubling(r#"{"Times":2}"#, vec![vec![G, G, R]])]
#[case::black_only(r#"{"One":"Black"}"#, vec![vec![B, R]])]
fn mana_a_triggered_mana_ability_adds_is_not_replaced(
    #[case] change: &str,
    #[case] expected: Vec<Vec<ManaType>>,
) {
    // CR 605.1b, 106.6: the trigger adds its mana to what the land produced after the replacement.
    let game = mine(vec![
        land(Color::Green),
        extra_red(),
        replacement(LAND, "You", change),
    ]);
    assert_eq!(sorted(made(&game, ME)), sorted(expected));
}

/// A land of the two basic types Plains and Island that taps for white or blue.
fn tundra() -> Arc<CardDefinition> {
    let json =
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White"]}}}]}}"#;
    Arc::new(
        CardDefinition::new("Tundra")
            .with_type(CardType::Land)
            .with_subtype("Plains")
            .with_subtype("Island")
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

#[test]
fn the_player_orders_the_replacements_of_one_tap() {
    // CR 616.1: Tundra under "Plains tap for {R}" and "Island tap for {G}" gives red or green, the player's choice.
    let game = mine(vec![
        tundra(),
        replacement(
            r#"[{"Subtype":"Plains"}]"#,
            "EachPlayer",
            r#"{"OnlyType":"Red"}"#,
        ),
        replacement(
            r#"[{"Subtype":"Island"}]"#,
            "EachPlayer",
            r#"{"OnlyType":"Green"}"#,
        ),
    ]);
    let got = made(&game, ME);
    assert!(got.contains(&vec![R]) && got.contains(&vec![G]), "{got:?}");
    let all = options(&game, ME);
    let sources: Vec<_> = all.iter().map(|o| o.source).collect();
    assert!(
        sources
            .iter()
            .enumerate()
            .all(|(n, s)| !sources[..n].contains(s)),
        "each order is a source of its own"
    );
    let green = all.into_iter().find(|o| o.made() == [G]).unwrap();
    let events = activation(&game, ME, &green, &[]).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ManaAdded { mana: G, .. }))
    );
}

#[test]
fn contamination_and_a_doubler_give_black_or_two() {
    // CR 616.1: "one black" first leaves one; doubling first leaves two black.
    let game = mine(vec![
        land(Color::Green),
        replacement(LAND, "EachPlayer", r#"{"One":"Black"}"#),
        replacement(r"[]", "You", r#"{"Times":2}"#),
    ]);
    let got = made(&game, ME);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.contains(&vec![B]) && got.contains(&vec![B, B]));
}
