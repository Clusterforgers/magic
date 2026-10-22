//! Goal: changeling makes its object every creature type and devoid makes it colorless, in every zone, as characteristic-defining abilities: a later effect that sets the creature types or the colours has the last word on the battlefield, and the types that are not creature types are never added.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::{EffectId, ObjectId};
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::defining::{Where, game_with};
use super::tests::named;
use super::*;
use crate::characteristics::characteristics;
use crate::effects::{ContinuousEffect, Duration, Selector};
use crate::object::GameObject;

/// A red 2/2 Goblin "Hulk" with the keyword.
fn with_keyword(keyword: Keyword) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Hulk")
            .with_mana_cost(ManaCost::parse("{1}{R}").unwrap())
            .with_type(CardType::Creature)
            .with_subtype("Goblin")
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(keyword)),
    )
}

fn object_of(game: &GameState, id: ObjectId) -> &GameObject {
    game.object(id).unwrap()
}

fn is(game: &GameState, id: ObjectId, subtype: &str) -> bool {
    let chars = characteristics(game, id).unwrap();
    chars.has_subtype(
        &object_of(game, id).card().subtypes,
        &Subtype(subtype.into()),
    )
}

#[rstest]
fn changeling_is_every_creature_type_in_every_zone(
    #[values(
        Where::Battlefield,
        Where::Hand,
        Where::Library,
        Where::Graveyard,
        Where::Exile,
        Where::Stack
    )]
    at: Where,
) {
    let game = game_with(with_keyword(Keyword::Changeling), at);
    let id = named(&game, "Hulk").id;
    for creature_type in ["Goblin", "Elf", "Wizard", "Equipment-Golem", "Time Lord"] {
        assert!(is(&game, id, creature_type), "{at:?}: {creature_type}");
    }
    for other in ["Forest", "Equipment", "Aura", "Treasure"] {
        assert!(!is(&game, id, other), "{at:?}: {other}");
    }
}

#[rstest]
#[case::a_changeling(true)]
#[case::an_ordinary_creature(false)]
fn a_set_of_creature_types_replaces_every_creature_type(#[case] changeling: bool) {
    let card = if changeling {
        with_keyword(Keyword::Changeling)
    } else {
        with_keyword(Keyword::Flying)
    };
    let mut game = game_with(card, Where::Battlefield);
    let hulk = named(&game, "Hulk");
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::Objects(vec![hulk]),
        Modification::SetCreatureTypes(vec![Subtype("Weird".into())]),
        Duration::Forever,
        7,
    ));
    game.layers.reset();
    assert!(is(&game, hulk.id, "Weird"));
    assert!(!is(&game, hulk.id, "Goblin"));
    assert!(!is(&game, hulk.id, "Elf"));
}

#[rstest]
fn devoid_is_colorless_in_every_zone_whatever_the_mana_cost(
    #[values(
        Where::Battlefield,
        Where::Hand,
        Where::Library,
        Where::Graveyard,
        Where::Exile,
        Where::Stack
    )]
    at: Where,
) {
    let game = game_with(with_keyword(Keyword::Devoid), at);
    let chars = characteristics(&game, named(&game, "Hulk").id).unwrap();
    assert_eq!(chars.colors, ColorSet::EMPTY, "{at:?}");
    // The same card without devoid is red.
    let plain = game_with(with_keyword(Keyword::Flying), at);
    let chars = characteristics(&plain, named(&plain, "Hulk").id).unwrap();
    assert_eq!(chars.colors, ColorSet::EMPTY.with(Color::Red), "{at:?}");
}

#[rstest]
#[case::a_later_effect_colors_it(Some(Color::Blue))]
#[case::nothing_else(None)]
fn a_color_given_after_devoid_wins_on_the_battlefield(#[case] given: Option<Color>) {
    let mut game = game_with(with_keyword(Keyword::Devoid), Where::Battlefield);
    let hulk = named(&game, "Hulk");
    if let Some(color) = given {
        game.effects.push(ContinuousEffect::new(
            EffectId(0),
            Selector::Objects(vec![hulk]),
            Modification::SetColors(ColorSet::EMPTY.with(color)),
            Duration::Forever,
            1,
        ));
        game.layers.reset();
    }
    let chars = characteristics(&game, hulk.id).unwrap();
    let want = given.map_or(ColorSet::EMPTY, |color| ColorSet::EMPTY.with(color));
    assert_eq!(chars.colors, want);
}

#[test]
fn losing_all_abilities_does_not_take_a_characteristic_defining_ability_away() {
    // Layer 6 comes after the layers 4 and 5 the abilities of devoid and changeling applied in (CR 613.6).
    let mut game = game_with(with_keyword(Keyword::Devoid), Where::Battlefield);
    let hulk = named(&game, "Hulk");
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::Objects(vec![hulk]),
        Modification::LoseAbilities,
        Duration::Forever,
        1,
    ));
    game.layers.reset();
    let chars = characteristics(&game, hulk.id).unwrap();
    assert_eq!(chars.colors, ColorSet::EMPTY);
}
