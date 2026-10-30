//! Goal: a land has the mana ability of each basic land type it has once the type layer is through, not of the types its card prints: a type an effect gives (Urborg, Yavimaya, Prismatic Omen) brings its ability and a type it already has no second one, a set of the land types (Blood Moon, Magus of the Moon) takes the other land types and the abilities of the rules text and keeps the ability of the types it sets, effects of the layer go by timestamp, Blood Moon takes Urborg's ability before Urborg uses it, a later layer can take the ability away, and the land is as printed again when the effect leaves.
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::{AbilityDef, CardDefinition, ManaAbility};
use mtg_core::card::{CardType, LAND_TYPES, Subtype, Supertype};
use mtg_core::mana::ManaType::{self, Black, Blue, Green, Red, White};
use rstest::rstest;

use super::tests::{fold, named, rules};
use super::*;
use crate::ability_reader::abilities_of;
use crate::characteristics::characteristics;
use crate::events::{Event, ZoneCause};
use crate::fixtures::state;

const ME: PlayerId = PlayerId(0);
const URBORG: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddSubtype":"Swamp"}]}}"#;
const YAVIMAYA: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddSubtype":"Forest"}]}}"#;
const MOON: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}}]},"mods":[{"SetLandTypes":["Mountain"]}]}}"#;
const OMEN: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddSubtype":"Plains"},{"AddSubtype":"Island"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"},{"AddSubtype":"Forest"}]}}"#;
const LATTICE: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddType":"Artifact"}]}}"#;
const UNLAND: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"SetTypes":["Artifact"]}]}}"#;
const HUMILITY: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilities"]}}"#;
const SOL_RING: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#;

fn mana(produces: ManaType) -> AbilityDef {
    AbilityDef::Mana(ManaAbility { produces })
}

/// A nonbasic land of `types` as the server assembles it: the mana of each of its basic types, in the order of the rules.
fn land(name: &str, types: &[&str], plain: &[ManaType]) -> CardDefinition {
    let mut card = CardDefinition::new(name).with_type(CardType::Land);
    for subtype in types {
        card = card.with_subtype(*subtype);
    }
    plain
        .iter()
        .fold(card, |card, &produces| card.with_ability(mana(produces)))
}

/// The land called `name` in the tables.
fn land_called(name: &str) -> Arc<CardDefinition> {
    Arc::new(match name {
        "Forest" => CardDefinition::basic_land(Some(mtg_core::color::Color::Green)),
        "Volcanic Island" => land(name, &["Island", "Mountain"], &[Blue, Red]),
        "Dryad Arbor" => land(name, &["Forest", "Dryad"], &[Green])
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
        "Tomb" => land(name, &[], &[]).with_rules(rules(SOL_RING)),
        "Minamo" => land(name, &[], &[Blue]),
        "Strip Mine" => land(name, &[], &[]),
        other => panic!("no land called {other}"),
    })
}

/// The permanent called `name` in the tables, with the ability that does what it is known for.
fn effect_called(name: &str) -> Arc<CardDefinition> {
    let card = |kind: CardType, ability: &str| {
        CardDefinition::new(name)
            .with_type(kind)
            .with_rules(rules(ability))
    };
    Arc::new(match name {
        "Urborg" => card(CardType::Land, URBORG).with_supertype(Supertype::Legendary),
        "Yavimaya" => card(CardType::Land, YAVIMAYA).with_supertype(Supertype::Legendary),
        "Blood Moon" => card(CardType::Enchantment, MOON),
        "Magus" => card(CardType::Creature, MOON).with_power_toughness(2, 2),
        "Omen" => card(CardType::Enchantment, OMEN),
        "Lattice" => card(CardType::Artifact, LATTICE),
        "Unland" => card(CardType::Enchantment, UNLAND),
        "Humility" => card(CardType::Enchantment, HUMILITY),
        other => panic!("no permanent called {other}"),
    })
}

/// A game in which seat 0 has the land and the permanents called `effects` in play, each entering in turn after the land,
/// so the first named is the oldest.
fn game(land: &str, effects: &[&str]) -> GameState {
    let hand = effects.iter().map(|name| effect_called(name)).collect();
    let mut game = state(vec![hand, vec![]], vec![vec![land_called(land)], vec![]]);
    for name in effects {
        let object = named(&game, name).id;
        fold(
            &mut game,
            &[Event::ZoneChanged {
                object,
                from: Zone::Hand(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            }],
        );
    }
    game
}

/// The plain mana abilities the land has, in the order they are offered.
fn plain(game: &GameState, name: &str) -> Vec<ManaType> {
    let chars = characteristics(game, named(game, name).id).unwrap();
    chars
        .abilities
        .iter()
        .map(|AbilityDef::Mana(ability)| ability.produces)
        .collect()
}

/// The land types the permanent is: the ones its card prints and no effect took away, and those effects gave.
fn land_types(game: &GameState, name: &str) -> Vec<&'static str> {
    let object = named(game, name);
    let chars = characteristics(game, object.id).unwrap();
    let printed = &game.object(object.id).unwrap().card().subtypes;
    LAND_TYPES
        .into_iter()
        .filter(|kind| chars.has_subtype(printed, &Subtype((*kind).to_owned())))
        .collect()
}

/// The abilities in the rules of the permanent that make mana.
fn making_mana(game: &GameState, name: &str) -> usize {
    abilities_of(game, named(game, name).id)
        .iter()
        .filter(|(_, rules)| matches!(**rules, Ability::Activated { .. }))
        .count()
}

#[rstest]
#[case::basic_land("Forest", &[], &[Green], &["Forest"])]
#[case::basic_land_with_urborg("Forest", &["Urborg"], &[Black, Green], &["Forest", "Swamp"])]
#[case::basic_land_with_the_type_it_has("Forest", &["Yavimaya"], &[Green], &["Forest"])]
#[case::basic_land_with_both("Forest", &["Urborg", "Yavimaya"], &[Black, Green], &["Forest", "Swamp"])]
#[case::basic_land_under_blood_moon("Forest", &["Blood Moon"], &[Green], &["Forest"])]
#[case::basic_land_under_blood_moon_and_urborg("Forest", &["Blood Moon", "Urborg"], &[Green], &["Forest"])]
#[case::dual("Volcanic Island", &[], &[Blue, Red], &["Island", "Mountain"])]
#[case::dual_with_urborg("Volcanic Island", &["Urborg"], &[Blue, Black, Red], &["Island", "Mountain", "Swamp"])]
#[case::dual_with_yavimaya("Volcanic Island", &["Yavimaya"], &[Blue, Red, Green], &["Forest", "Island", "Mountain"])]
#[case::dual_with_both("Volcanic Island", &["Urborg", "Yavimaya"], &[Blue, Black, Red, Green], &["Forest", "Island", "Mountain", "Swamp"])]
#[case::dual_under_blood_moon("Volcanic Island", &["Blood Moon"], &[Red], &["Mountain"])]
#[case::dual_under_magus("Volcanic Island", &["Magus"], &[Red], &["Mountain"])]
#[case::dual_under_two_moons("Volcanic Island", &["Blood Moon", "Magus"], &[Red], &["Mountain"])]
#[case::dual_under_urborg_and_blood_moon("Volcanic Island", &["Urborg", "Blood Moon"], &[Red], &["Mountain"])]
#[case::dual_under_blood_moon_and_urborg("Volcanic Island", &["Blood Moon", "Urborg"], &[Red], &["Mountain"])]
#[case::dual_under_yavimaya_and_blood_moon("Volcanic Island", &["Yavimaya", "Blood Moon"], &[Red], &["Mountain"])]
#[case::a_type_after_the_set("Volcanic Island", &["Blood Moon", "Omen"], &[White, Blue, Black, Red, Green], &["Forest", "Island", "Mountain", "Plains", "Swamp"])]
#[case::a_type_before_the_set("Volcanic Island", &["Omen", "Blood Moon"], &[Red], &["Mountain"])]
#[case::a_type_that_changes_no_land_type("Volcanic Island", &["Lattice"], &[Blue, Red], &["Island", "Mountain"])]
#[case::creature_land("Dryad Arbor", &["Urborg"], &[Black, Green], &["Forest", "Swamp"])]
#[case::creature_land_under_blood_moon("Dryad Arbor", &["Blood Moon"], &[Red], &["Mountain"])]
#[case::land_of_no_type("Minamo", &[], &[Blue], &[])]
#[case::land_of_no_type_with_urborg("Minamo", &["Urborg"], &[Black, Blue], &["Swamp"])]
#[case::land_of_no_type_under_blood_moon("Minamo", &["Blood Moon"], &[Red], &["Mountain"])]
#[case::land_of_no_ability("Strip Mine", &["Urborg"], &[Black], &["Swamp"])]
#[case::land_of_no_ability_under_blood_moon("Strip Mine", &["Blood Moon"], &[Red], &["Mountain"])]
#[case::a_later_layer_takes_the_ability("Volcanic Island", &["Urborg", "Humility"], &[], &["Island", "Mountain", "Swamp"])]
#[case::a_later_layer_takes_the_set_one("Volcanic Island", &["Humility", "Blood Moon"], &[], &["Mountain"])]
fn a_land_has_the_mana_of_the_basic_types_it_has_once_the_type_layer_is_through(
    #[case] land: &str,
    #[case] effects: &[&str],
    #[case] mana: &[ManaType],
    #[case] types: &[&str],
) {
    let game = game(land, effects);
    assert_eq!(plain(&game, land), mana, "{land} with {effects:?}");
    assert_eq!(land_types(&game, land), types, "{land} with {effects:?}");
}

#[rstest]
#[case::urborg("Urborg")]
#[case::yavimaya("Yavimaya")]
fn blood_moon_takes_the_ability_of_a_land_that_changes_land_types_whatever_their_age(
    #[case] other: &str,
    #[values(false, true)] moon_first: bool,
) {
    let order = if moon_first {
        ["Blood Moon", other]
    } else {
        [other, "Blood Moon"]
    };
    let game = game("Forest", &order);
    // The other land is a Mountain with no ability of its own: it makes nothing a Swamp or a Forest.
    assert_eq!(plain(&game, other), [Red], "{order:?}");
    assert_eq!(land_types(&game, other), ["Mountain"], "{order:?}");
    assert_eq!(abilities_of(&game, named(&game, other).id).len(), 0);
    assert_eq!(plain(&game, "Forest"), [Green], "{order:?}");
    assert_eq!(land_types(&game, "Forest"), ["Forest"], "{order:?}");
}

#[rstest]
#[case::urborg_is_a_swamp_too("Urborg", &["Urborg"], &[Black], &["Swamp"])]
#[case::yavimaya_is_a_forest("Yavimaya", &["Yavimaya"], &[Green], &["Forest"])]
#[case::urborg_and_yavimaya("Urborg", &["Urborg", "Yavimaya"], &[Black, Green], &["Forest", "Swamp"])]
fn the_land_that_gives_a_type_has_it_too(
    #[case] land: &str,
    #[case] effects: &[&str],
    #[case] mana: &[ManaType],
    #[case] types: &[&str],
) {
    let game = game("Forest", effects);
    assert_eq!(plain(&game, land), mana);
    assert_eq!(land_types(&game, land), types);
}

#[rstest]
#[case::dual_with_urborg("Volcanic Island", "Urborg", &[Blue, Red], 0)]
#[case::dual_under_blood_moon("Volcanic Island", "Blood Moon", &[Blue, Red], 0)]
#[case::dual_under_magus("Volcanic Island", "Magus", &[Blue, Red], 0)]
#[case::basic_land_with_urborg("Forest", "Urborg", &[Green], 0)]
#[case::unusual_land_under_blood_moon("Tomb", "Blood Moon", &[], 1)]
fn the_land_is_as_printed_when_the_effect_leaves(
    #[case] land: &str,
    #[case] effect: &str,
    #[case] printed: &[ManaType],
    #[case] rules_left: usize,
) {
    let mut game = game(land, &[effect]);
    let object = named(&game, effect).id;
    let owner = game.object(object).unwrap().owner();
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Destroyed,
        }],
    );
    assert_eq!(plain(&game, land), printed);
    assert_eq!(making_mana(&game, land), rules_left);
}

#[rstest]
#[case::unusual_land("Tomb", &[], &[], 1)]
#[case::unusual_land_with_urborg("Tomb", &["Urborg"], &[Black], 1)]
#[case::unusual_land_under_blood_moon("Tomb", &["Blood Moon"], &[Red], 0)]
#[case::unusual_land_under_a_later_set("Tomb", &["Urborg", "Blood Moon"], &[Red], 0)]
#[case::no_land_changes_it("Tomb", &["Lattice"], &[], 1)]
fn the_mana_abilities_of_the_rules_text_stay_unless_the_land_types_are_set(
    #[case] land: &str,
    #[case] effects: &[&str],
    #[case] mana: &[ManaType],
    #[case] rules_left: usize,
) {
    let game = game(land, effects);
    assert_eq!(plain(&game, land), mana);
    assert_eq!(making_mana(&game, land), rules_left);
}

#[rstest]
#[case::basic_land("Forest")]
#[case::dual("Volcanic Island")]
#[case::creature_land("Dryad Arbor")]
fn a_permanent_that_is_no_longer_a_land_has_no_land_ability(#[case] land: &str) {
    let game = game(land, &["Unland"]);
    assert_eq!(plain(&game, land), []);
}

#[test]
fn the_creature_types_of_a_creature_land_stay_when_the_land_types_are_set() {
    let game = game("Dryad Arbor", &["Blood Moon"]);
    let chars = characteristics(&game, named(&game, "Dryad Arbor").id).unwrap();
    let printed = &game
        .object(named(&game, "Dryad Arbor").id)
        .unwrap()
        .card()
        .subtypes;
    assert!(chars.has_subtype(printed, &Subtype("Dryad".to_owned())));
    assert!(!chars.has_subtype(printed, &Subtype("Forest".to_owned())));
    assert!(chars.is_creature() && chars.types.contains(CardType::Land));
}
