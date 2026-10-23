//! Goal: the triggered mana abilities of the permanents in play join the mana of the tap that triggers them (whose land, whose tap, how many triggers), a pick of any color is a source for each color, using an option adds the bonus in the same batch as the tap, and the ability never triggers onto the stack.
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use rstest::rstest;

use super::*;
use crate::events::Event;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::mana_ability::{activation, options};

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn land(color: Color) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(color)))
}

/// An enchantment with one triggered ability on a tap for mana.
fn trigger(what: &str, by: &str, who: &str, mana: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"TappedForMana":{{"what":{what},"by":"{by}"}}}},"effects":[{{"AddMana":{{"who":"{who}","mana":{mana}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Mana Trigger")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

const LAND: &str = r#"[{"Type":"Land"}]"#;

/// ME has a Forest, a Swamp and the given permanents; FOE has a Forest.
fn board(triggers: Vec<Arc<CardDefinition>>) -> GameState {
    let mut mine = vec![land(Color::Green), land(Color::Black)];
    mine.extend(triggers);
    state(vec![vec![], vec![]], vec![mine, vec![land(Color::Green)]])
}

fn made(state: &GameState, player: PlayerId) -> Vec<Vec<ManaType>> {
    options(state, player)
        .iter()
        .map(ManaOption::made)
        .collect()
}

use ManaType::{Black as B, Green as G};

#[rstest]
#[case::a_land_you_tap(trigger(LAND, "You", "You", r#""OfTapped""#), vec![vec![G, G], vec![B, B]], vec![vec![G]])]
#[case::a_land_any_player_taps(trigger(LAND, "EachPlayer", "Triggering", r#""OfTapped""#), vec![vec![G, G], vec![B, B]], vec![vec![G, G]])]
#[case::only_a_swamp(trigger(r#"[{"Subtype":"Swamp"}]"#, "You", "You", r#"{"Fixed":["Black"]}"#), vec![vec![G], vec![B, B]], vec![vec![G]])]
#[case::an_additional_mana_of_a_type(trigger(LAND, "You", "You", r#"{"Fixed":["Green"]}"#), vec![vec![G, G], vec![B, G]], vec![vec![G]])]
#[case::no_chosen_color(trigger(LAND, "You", "You", r#"{"ChosenColor":1}"#), vec![vec![G], vec![B]], vec![vec![G]])]
fn a_tap_for_mana_makes_the_mana_of_the_triggers_that_fire(
    #[case] trigger: Arc<CardDefinition>,
    #[case] mine: Vec<Vec<ManaType>>,
    #[case] theirs: Vec<Vec<ManaType>>,
) {
    let game = board(vec![trigger]);
    assert_eq!(made(&game, ME), mine);
    assert_eq!(made(&game, FOE), theirs);
}

#[test]
fn several_triggers_add_their_mana_one_after_the_other() {
    let game = board(vec![
        trigger(LAND, "You", "You", r#""OfTapped""#),
        trigger(LAND, "You", "You", r#"{"Fixed":["Green"]}"#),
    ]);
    assert_eq!(
        made(&game, ME),
        [vec![G, G, G], vec![B, B, G]],
        "each trigger adds its own mana"
    );
}

#[test]
fn a_pick_of_any_color_is_a_source_for_each_color_with_numbers_of_its_own() {
    let game = board(vec![trigger(LAND, "You", "You", r#"{"AnyColor":1}"#)]);
    let found = options(&game, ME);
    let forest = game.zone(Zone::Battlefield)[0];
    let forest: Vec<&ManaOption> = found.iter().filter(|o| o.source.object == forest).collect();
    assert_eq!(forest.len(), 5);
    let mut numbers: Vec<usize> = forest.iter().map(|o| o.source.ability).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), 5, "a source for each pick");
    assert!(forest.iter().all(|o| o.source.ability % STRIDE == 0));
    let picked: Vec<ManaType> = forest.iter().map(|o| o.made()[1]).collect();
    assert_eq!(picked, COLORS);
}

#[test]
fn using_a_source_adds_the_bonus_with_the_tap_and_triggers_nothing() {
    let mut game = board(vec![trigger(LAND, "You", "You", r#""OfTapped""#)]);
    let pre = game.clone();
    let option = options(&game, ME).remove(0);
    let events = activation(&game, ME, &option, &[]).unwrap();
    let forest = game.zone(Zone::Battlefield)[0];
    let added = Event::ManaAdded {
        player: ME,
        mana: G,
        source: Some(forest),
        only: Vec::new(),
    };
    assert_eq!(
        events,
        [
            Event::PermanentTapped { object: forest },
            added.clone(),
            added
        ]
    );
    for event in &events {
        apply_event(&mut game, event).unwrap();
    }
    let stacked = crate::triggers::detect(&pre, &events, &game);
    assert!(
        stacked.is_empty(),
        "a mana ability uses no stack: {stacked:?}"
    );
}

#[test]
fn an_ability_that_does_more_than_add_mana_still_triggers_onto_the_stack() {
    let hurting = Arc::new(
        CardDefinition::new("Manabarbs")
            .with_type(CardType::Enchantment)
            .with_rules(
                serde_json::from_str(
                    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let mut game = board(vec![hurting]);
    let pre = game.clone();
    let option = options(&game, ME).remove(0);
    let events = activation(&game, ME, &option, &[]).unwrap();
    for event in &events {
        apply_event(&mut game, event).unwrap();
    }
    let stacked = crate::triggers::detect(&pre, &events, &game);
    assert_eq!(stacked.len(), 1, "{stacked:?}");
}
