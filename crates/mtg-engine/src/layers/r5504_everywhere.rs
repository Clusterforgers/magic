//! Goal: a static ability over every zone changes the cards outside the battlefield as it changes the permanents, at
//! two to four seats: "the chosen color in addition to their other colors" for every card, spell and permanent
//! (Painter's Servant), "are the chosen type" for its controller's creatures only (Conspiracy), "are white" for its
//! controller's nonland cards (Celestial Dawn) and "basic" for its controller's lands and the land cards of their library
//! only (Rootpath Purifier); nothing changes while the source chose nothing.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::tests::rules;
use crate::characteristics::{Characteristics, characteristics};
use crate::events::Event;
use crate::fixtures::setup;
use crate::fold::apply_event;
use crate::object::Chosen;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

const EVERY_ZONE: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Stack"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Command"}]}"#;

/// An enchantment whose static ability gives `mods` to what `selector` names.
fn source(selector: &str, mods: &str) -> Arc<CardDefinition> {
    let ability = format!(r#"{{"Static":{{"affected":{{"Each":[{selector}]}},"mods":[{mods}]}}}}"#);
    Arc::new(
        CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rules(&ability)),
    )
}

fn elf(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_color_indicator(ColorSet::EMPTY.with(Color::Red))
            .with_power_toughness(1, 1),
    )
}

fn land(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
}

/// A game where every seat has an Elf and a land in hand, library and graveyard and an Elf in play; seat 0 also has the
/// source in play, which entered having chosen `chosen`.
fn table(seats: usize, source: &Arc<CardDefinition>, chosen: Option<Chosen>) -> GameState {
    let hands = (0..seats)
        .map(|seat| {
            vec![
                elf(&format!("Hand elf {seat}")),
                land(&format!("Hand land {seat}")),
            ]
        })
        .collect();
    let battlefields = (0..seats)
        .map(|seat| {
            let mut cards = vec![
                elf(&format!("Play elf {seat}")),
                land(&format!("Play land {seat}")),
            ];
            if seat == 0 {
                cards.insert(0, source.clone());
            }
            cards
        })
        .collect();
    let mut setup = setup(hands, battlefields);
    for (seat, player) in setup.players.iter_mut().enumerate() {
        player.library = vec![
            elf(&format!("Library elf {seat}")),
            land(&format!("Library land {seat}")),
        ];
        player.graveyard = vec![
            elf(&format!("Grave elf {seat}")),
            land(&format!("Grave land {seat}")),
        ];
    }
    let mut game = GameState::from_setup(&setup).unwrap();
    let on = game.zone(Zone::Battlefield)[0];
    apply_event(
        &mut game,
        &Event::Entered {
            object: on,
            tapped: false,
            controller: ME,
            chosen,
        },
    )
    .unwrap();
    game
}

fn named(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|object| object.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn chars(game: &GameState, name: &str) -> Characteristics {
    characteristics(game, named(game, name)).unwrap()
}

const PLACES: [&str; 4] = ["Hand", "Play", "Library", "Grave"];

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn every_card_spell_and_permanent_is_also_the_chosen_color(#[case] seats: usize) {
    let game = table(
        seats,
        &source(EVERY_ZONE, r#""AddChosenColor""#),
        Some(Chosen::Color(Color::Blue)),
    );
    let both = ColorSet::EMPTY.with(Color::Red).with(Color::Blue);
    for seat in 0..seats {
        for place in PLACES {
            assert_eq!(chars(&game, &format!("{place} elf {seat}")).colors, both);
            assert_eq!(
                chars(&game, &format!("{place} land {seat}")).colors,
                ColorSet::EMPTY.with(Color::Blue)
            );
        }
    }
}

#[rstest]
#[case::no_color_chosen(None)]
#[case::a_type_is_no_color(Some(Chosen::CreatureType(Subtype("Rat".to_owned()))))]
fn nothing_chosen_adds_no_color(#[case] chosen: Option<Chosen>) {
    let game = table(3, &source(EVERY_ZONE, r#""AddChosenColor""#), chosen);
    for place in PLACES {
        assert_eq!(
            chars(&game, &format!("{place} elf 1")).colors,
            ColorSet::EMPTY.with(Color::Red)
        );
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn the_controllers_creatures_are_only_the_chosen_type_in_every_zone(#[case] seats: usize) {
    let selector = format!(r#"{{"Type":"Creature"}},{{"ControlledBy":"You"}},{EVERY_ZONE}"#);
    let game = table(
        seats,
        &source(&selector, r#"{"SetCreatureTypes":[]},"AddChosenSubtype""#),
        Some(Chosen::CreatureType(Subtype("Rat".to_owned()))),
    );
    let rat = Subtype("Rat".to_owned());
    let elf = Subtype("Elf".to_owned());
    for place in PLACES {
        let mine = chars(&game, &format!("{place} elf 0"));
        assert!(
            mine.creature_types_set && mine.subtypes == vec![rat.clone()],
            "{place}: {mine:?}"
        );
        for seat in 1..seats {
            let theirs = chars(&game, &format!("{place} elf {seat}"));
            assert!(
                !theirs.creature_types_set && !theirs.subtypes.contains(&rat),
                "{place}"
            );
            assert!(
                game.object(named(&game, &format!("{place} elf {seat}")))
                    .unwrap()
                    .card()
                    .subtypes
                    .contains(&elf)
            );
        }
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn the_controllers_nonland_cards_are_white_in_every_zone(#[case] seats: usize) {
    let selector = format!(r#"{{"Not":{{"Type":"Land"}}}},{{"ControlledBy":"You"}},{EVERY_ZONE}"#);
    let game = table(
        seats,
        &source(&selector, r#"{"SetColors":["White"]}"#),
        None,
    );
    let white = ColorSet::EMPTY.with(Color::White);
    assert_eq!(chars(&game, "Source").colors, white);
    for place in PLACES {
        assert_eq!(
            chars(&game, &format!("{place} elf 0")).colors,
            white,
            "{place}"
        );
        assert_eq!(
            chars(&game, &format!("{place} land 0")).colors,
            ColorSet::EMPTY,
            "{place}"
        );
        for seat in 1..seats {
            assert_eq!(
                chars(&game, &format!("{place} elf {seat}")).colors,
                ColorSet::EMPTY.with(Color::Red)
            );
        }
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn the_controllers_lands_and_library_lands_are_basic(#[case] seats: usize) {
    let selector = r#"{"Type":"Land"},{"ControlledBy":"You"},{"AnyOf":[{"In":"Battlefield"},{"In":"Library"}]}"#;
    let game = table(
        seats,
        &source(selector, r#"{"AddSupertype":"Basic"}"#),
        None,
    );
    let basic = |name: &str| chars(&game, name).supertypes.contains(&Supertype::Basic);
    assert!(basic("Play land 0"));
    assert!(basic("Library land 0"));
    assert!(!basic("Hand land 0"));
    assert!(!basic("Grave land 0"));
    assert!(!basic("Library elf 0"));
    for seat in 1..seats {
        for place in PLACES {
            assert!(!basic(&format!("{place} land {seat}")), "{place} {seat}");
        }
    }
}
