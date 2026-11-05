//! Goal: the counts of the count-expression grammar read the game as the rules say: the objects a reference names that
//! satisfy a filter (also after they left), a product, the most creatures sharing a type or a name, and a zone list that
//! counts control on the battlefield and ownership in the graveyard.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::fixtures::setup;
use crate::state::GameState;

fn creature(name: &str, kind: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_power_toughness(power, power),
    )
}

/// Seat 0 controls two Elves and a Goblin and owns a Goblin in the graveyard; seat 1 controls an Elf.
fn game() -> GameState {
    let mut setup = setup(
        vec![vec![], vec![]],
        vec![
            vec![
                creature("Elf A", "Elf", 1),
                creature("Elf A", "Elf", 3),
                creature("Goblin", "Goblin", 2),
            ],
            vec![creature("Other Elf", "Elf", 5)],
        ],
    );
    setup.players[0].graveyard = vec![creature("Dead Goblin", "Goblin", 4)];
    GameState::from_setup(&setup).unwrap()
}

fn value(json: &str) -> i64 {
    let game = game();
    let source = game.objects.values().next().unwrap().reference();
    let amount: Amount = serde_json::from_str(json).unwrap();
    Scope::announcing(&game, PlayerId(0), source)
        .amount(&amount)
        .unwrap()
}

/// Every creature of seat 0 on the battlefield or in its graveyard, written the way the translator writes a zone list.
const ZONES: &str = r#"[{"AnyOf":[
    {"Not":{"AnyOf":[{"Not":{"Type":"Creature"}},{"Not":{"ControlledBy":"You"}},{"Not":{"In":"Battlefield"}}]}},
    {"Not":{"AnyOf":[{"Not":{"Type":"Creature"}},{"Not":{"OwnedBy":"You"}},{"Not":{"In":"Graveyard"}}]}}
]}]"#;

#[rstest]
#[case::this_is_a_creature(r#"{"CountOf":{"of":"This","filter":[{"Type":"Creature"}]}}"#, 1)]
#[case::this_is_no_artifact(r#"{"CountOf":{"of":"This","filter":[{"Type":"Artifact"}]}}"#, 0)]
#[case::no_filter_counts_the_objects(r#"{"CountOf":{"of":"This","filter":[]}}"#, 1)]
#[case::a_name_nothing_bound_is_none(r#"{"CountOf":{"of":{"Bound":"nothing"},"filter":[]}}"#, 0)]
#[case::product(r#"{"Product":[{"Count":[{"Type":"Creature"}]},3]}"#, 12)]
#[case::product_with_zero(r#"{"Product":[{"Count":[{"Type":"Artifact"}]},3]}"#, 0)]
#[case::most_creatures_of_a_type(
    r#"{"MostCommon":{"of":[{"Type":"Creature"}],"stat":"CreatureType"}}"#,
    3
)]
#[case::most_of_a_type_you_control(
    r#"{"MostCommon":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"CreatureType"}}"#,
    2
)]
#[case::no_creatures_no_type(
    r#"{"MostCommon":{"of":[{"Type":"Artifact"}],"stat":"CreatureType"}}"#,
    0
)]
#[case::most_cards_with_a_name(r#"{"MostCommon":{"of":[{"Type":"Creature"}],"stat":"Name"}}"#, 2)]
fn the_count_amounts_read_the_game(#[case] json: &str, #[case] expected: i64) {
    assert_eq!(value(json), expected);
}

#[rstest]
#[case::count(format!(r#"{{"Count":{ZONES}}}"#), 4)]
#[case::greatest(format!(r#"{{"Greatest":{{"of":{ZONES},"stat":"Power"}}}}"#), 4)]
#[case::total(format!(r#"{{"Total":{{"of":{ZONES},"stat":"Power"}}}}"#), 10)]
fn a_zone_list_looks_in_each_zone_under_its_own_rule(#[case] json: String, #[case] expected: i64) {
    assert_eq!(value(&json), expected);
}
