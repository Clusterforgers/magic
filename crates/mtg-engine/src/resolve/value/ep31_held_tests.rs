//! Goal: `Pred::HasAbility` reads the abilities an object has: a vanilla creature has none, a keyword, a rule or a clause
//! played by hand is an ability, a plain or a ruled mana ability is a mana ability, and an activated ability that adds no
//! mana (or a loyalty ability) is a non-mana activated one; a card in a hand is read by its printed abilities.
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::definition::{AbilityDef, CardDefinition, ManaAbility, ManualClause};
use mtg_cards::refs::Selector;
use mtg_core::mana::ManaType;
use rstest::rstest;

use super::Scope;
use crate::fixtures::{creature, state};
use crate::state::GameState;
use crate::zones::Zone;

fn ruled(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        (*creature(name, 1, 1))
            .clone()
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn game() -> GameState {
    let elf = Arc::new(
        (*creature("Elf", 1, 1))
            .clone()
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            })),
    );
    let mut manual = (*creature("Odd", 1, 1)).clone();
    manual
        .manual
        .push(ManualClause::new("Odd things happen.", "by hand"));
    state(
        vec![vec![ruled("Flier", r#"{"Keyword":"Flying"}"#)], vec![]],
        vec![
            vec![
                creature("Bear", 2, 2),
                ruled("Flier", r#"{"Keyword":"Flying"}"#),
                elf,
                ruled(
                    "Pinger",
                    r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#,
                ),
                ruled(
                    "Bird",
                    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
                ),
                Arc::new(manual),
            ],
            vec![],
        ],
    )
}

fn matched(game: &GameState, json: &str) -> Vec<String> {
    let source = game
        .object(game.zone(Zone::Battlefield)[0])
        .unwrap()
        .reference();
    let selector: Selector = serde_json::from_str(json).unwrap();
    Scope::announcing(game, mtg_core::ids::PlayerId(0), source)
        .matching(&selector)
        .unwrap()
        .into_iter()
        .map(|r| {
            let object = game.object(r.id).unwrap();
            format!("{} {:?}", object.card().name, object.zone())
        })
        .collect()
}

#[rstest]
#[case::no_abilities(r#"[{"Not":{"HasAbility":"Any"}}]"#, &["Bear Battlefield"])]
#[case::a_mana_ability(r#"[{"HasAbility":"Mana"}]"#, &["Elf Battlefield", "Bird Battlefield"])]
#[case::non_mana_activated(r#"[{"HasAbility":"NonManaActivated"}]"#, &["Pinger Battlefield"])]
#[case::a_card_in_a_hand(
    r#"[{"In":"Hand"},{"HasAbility":"Any"}]"#,
    &["Flier Hand(PlayerId(0))"]
)]
fn ep31_the_abilities_an_object_has(#[case] json: &str, #[case] expected: &[&str]) {
    assert_eq!(matched(&game(), json), expected);
}
