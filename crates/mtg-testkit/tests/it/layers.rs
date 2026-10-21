//! Goal: the corpus cards that need the derivation of CR 613 get what the rules give them: a characteristic-defining ability (Nightmare, Maro, Transguild Courier) defines its card in every zone and counts what the layers made, a static ability reads the other objects as the earlier layers left them (Favorable Winds finds the flying Levitation gives, Nightmare finds the Swamps another card makes), an effect that depends on another comes after it whatever their ages (Goblin Chieftain after Humility, whichever entered first), and the one card whose static ability changes types, colours and power and toughness (Kormus Bell) does all three to the same permanents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::keywords::keywords_of;
use mtg_engine::{GameState, characteristics::Characteristics};
use mtg_testkit::cards::{forest, free_creature, swamp};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

fn id_of(game: &GameState, name: &str) -> mtg_core::ids::ObjectId {
    game.objects()
        .find(|object| object.card().name == name)
        .unwrap_or_else(|| panic!("{name} is in the game"))
        .id()
}

fn chars(game: &GameState, name: &str) -> Characteristics {
    characteristics(game, id_of(game, name)).unwrap()
}

fn stats(game: &GameState, name: &str) -> (Option<i32>, Option<i32>) {
    let shown = chars(game, name);
    (shown.power, shown.toughness)
}

fn keywords(game: &GameState, name: &str) -> Vec<String> {
    keywords_of(game, id_of(game, name))
        .into_iter()
        .map(|keyword| format!("{keyword:?}"))
        .collect()
}

/// The corpus card called `name`, on the battlefield untapped.
fn corpus(name: &str) -> mtg_engine::PresetPermanent {
    ready(mtg_testkit::corpus::card(name).unwrap())
}

fn goblin() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Goblin")
            .with_type(CardType::Creature)
            .with_subtype("Goblin")
            .with_power_toughness(1, 1),
    )
}

#[derive(Clone, Copy, Debug)]
enum Zoned {
    Battlefield,
    Hand,
    Library,
    Graveyard,
}

/// Seat 0 has three Swamps and a Forest on the battlefield, seat 1 one Swamp, and the card called `name` is in `zone`.
fn swamps_and(name: &str, zone: Zoned) -> GameState {
    let mut battlefield: Vec<_> = [swamp(), swamp(), swamp(), forest()]
        .into_iter()
        .map(ready)
        .collect();
    let mut scenario = Scenario::new(2);
    match zone {
        Zoned::Battlefield => battlefield.push(ready(mtg_testkit::corpus::card(name).unwrap())),
        Zoned::Hand => scenario = scenario.hand_named(0, [name]),
        Zoned::Library => scenario = scenario.library_named(0, [name]),
        Zoned::Graveyard => scenario = scenario.graveyard_named(0, [name]),
    }
    scenario
        .battlefield(0, battlefield)
        .battlefield(1, [ready(swamp())])
        .build()
        .unwrap()
        .state()
        .clone()
}

#[rstest]
fn nightmare_is_as_big_as_the_swamps_its_controller_has_wherever_it_is(
    #[values(Zoned::Battlefield, Zoned::Hand, Zoned::Library, Zoned::Graveyard)] zone: Zoned,
) {
    let game = swamps_and("Nightmare", zone);
    assert_eq!(stats(&game, "Nightmare"), (Some(3), Some(3)), "{zone:?}");
}

#[test]
fn nightmare_on_the_battlefield_flies_and_a_nightmare_in_hand_still_prints_it() {
    let game = swamps_and("Nightmare", Zoned::Battlefield);
    assert_eq!(keywords(&game, "Nightmare"), ["Flying"]);
}

#[rstest]
fn maro_is_as_big_as_the_hand_of_its_owner_wherever_it_is(
    #[values(Zoned::Battlefield, Zoned::Hand, Zoned::Graveyard)] zone: Zoned,
) {
    // Seat 0 holds two other cards, and Maro too when it is in the hand.
    let mut scenario = Scenario::new(2).hand_named(0, ["Lightning Bolt", "Giant Growth"]);
    scenario = match zone {
        Zoned::Battlefield => scenario.battlefield_named(0, ["Maro"]),
        Zoned::Graveyard => scenario.graveyard_named(0, ["Maro"]),
        _ => scenario.hand_named(0, ["Lightning Bolt", "Giant Growth", "Maro"]),
    };
    let game = scenario.build().unwrap().state().clone();
    let hand = if matches!(zone, Zoned::Hand) { 3 } else { 2 };
    assert_eq!(stats(&game, "Maro"), (Some(hand), Some(hand)), "{zone:?}");
}

#[rstest]
fn transguild_courier_is_every_colour_in_every_zone(
    #[values(Zoned::Battlefield, Zoned::Hand, Zoned::Library, Zoned::Graveyard)] zone: Zoned,
) {
    let game = swamps_and("Transguild Courier", zone);
    let colors = chars(&game, "Transguild Courier").colors;
    assert_eq!(colors.iter().count(), 5, "{zone:?}");
    for color in [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ] {
        assert!(colors.contains(color));
    }
}

#[test]
fn kormus_bell_makes_every_swamp_a_black_one_by_one_creature_that_is_still_a_land() {
    let game = Scenario::new(2)
        .battlefield(0, [corpus("Kormus Bell"), ready(swamp()), ready(forest())])
        .battlefield(1, [ready(swamp())])
        .build()
        .unwrap()
        .state()
        .clone();
    let swamps: Vec<_> = game
        .objects()
        .filter(|object| object.card().name == "Swamp")
        .map(|object| characteristics(&game, object.id()).unwrap())
        .collect();
    assert_eq!(swamps.len(), 2);
    for swamp in swamps {
        assert!(swamp.types.contains(CardType::Land));
        assert!(swamp.types.contains(CardType::Creature));
        assert_eq!((swamp.power, swamp.toughness), (Some(1), Some(1)));
        assert_eq!(swamp.colors.iter().collect::<Vec<_>>(), [Color::Black]);
    }
    let forest = chars(&game, "Forest");
    assert!(!forest.types.contains(CardType::Creature));
    assert_eq!(forest.power, None);
}

#[test]
fn a_definition_reads_the_permanents_after_the_layers_that_change_what_it_counts() {
    // Nightmare counts Swamps; the Bell makes them creatures and the Swamps are still Swamps.
    let game = Scenario::new(2)
        .battlefield(
            0,
            [
                corpus("Kormus Bell"),
                corpus("Nightmare"),
                ready(swamp()),
                ready(swamp()),
            ],
        )
        .build()
        .unwrap()
        .state()
        .clone();
    assert_eq!(stats(&game, "Nightmare"), (Some(2), Some(2)));
}

fn winds_and_wings(pumps: bool, flies: bool) -> GameState {
    let mut permanents = vec![ready(free_creature("Bear", 2, 2))];
    if pumps {
        permanents.push(corpus("Favorable Winds"));
    }
    if flies {
        permanents.push(corpus("Levitation"));
    }
    Scenario::new(2)
        .battlefield(0, permanents)
        .build()
        .unwrap()
        .state()
        .clone()
}

#[rstest]
#[case::neither(false, false, (2, 2), 0)]
#[case::only_the_winds_find_no_flying(true, false, (2, 2), 0)]
#[case::only_the_wings_give_flying_and_no_pump(false, true, (2, 2), 1)]
#[case::the_winds_find_the_flying_the_wings_give(true, true, (3, 3), 1)]
fn favorable_winds_finds_the_flying_levitation_gives(
    #[case] pumps: bool,
    #[case] flies: bool,
    #[case] bear: (i32, i32),
    #[case] flying: usize,
) {
    let game = winds_and_wings(pumps, flies);
    assert_eq!(stats(&game, "Bear"), (Some(bear.0), Some(bear.1)));
    assert_eq!(keywords(&game, "Bear").len(), flying);
}

/// Seat 0 has a Goblin Chieftain, a Goblin and, when `humility` is given, Humility, in the order the names are listed.
fn chieftain_game(order: &[&str]) -> GameState {
    let mut permanents = Vec::new();
    for name in order {
        permanents.push(ready(if *name == "Goblin" {
            goblin()
        } else {
            mtg_testkit::corpus::card(name).unwrap()
        }));
    }
    Scenario::new(2)
        .battlefield(0, permanents)
        .build()
        .unwrap()
        .state()
        .clone()
}

#[rstest]
#[case(&["Goblin Chieftain", "Goblin", "Humility"])]
#[case(&["Humility", "Goblin", "Goblin Chieftain"])]
#[case(&["Goblin", "Humility", "Goblin Chieftain"])]
#[case(&["Goblin Chieftain", "Humility", "Goblin"])]
fn humility_takes_the_abilities_of_a_goblin_chieftain_whichever_came_first(#[case] order: &[&str]) {
    let game = chieftain_game(order);
    assert_eq!(stats(&game, "Goblin"), (Some(1), Some(1)), "{order:?}");
    assert_eq!(stats(&game, "Goblin Chieftain"), (Some(1), Some(1)));
    assert!(
        keywords(&game, "Goblin").is_empty(),
        "no haste for the Goblin"
    );
    assert!(
        keywords(&game, "Goblin Chieftain").is_empty(),
        "nor its own"
    );
}

#[test]
fn a_goblin_chieftain_without_humility_pumps_and_gives_haste() {
    let game = chieftain_game(&["Goblin Chieftain", "Goblin"]);
    assert_eq!(stats(&game, "Goblin"), (Some(2), Some(2)));
    assert_eq!(keywords(&game, "Goblin"), ["Haste"]);
    assert!(
        chars(&game, "Goblin")
            .subtypes
            .iter()
            .all(|subtype| subtype != &Subtype("Elf".into()))
    );
}

#[test]
fn humility_leaves_a_nightmare_a_one_by_one_without_flying() {
    // The characteristic-defining power and toughness are lost with the abilities (layer 6 comes before 7a), and
    // Humility sets one and one.
    let game = Scenario::new(2)
        .battlefield(
            0,
            [
                corpus("Humility"),
                corpus("Nightmare"),
                ready(swamp()),
                ready(swamp()),
            ],
        )
        .build()
        .unwrap()
        .state()
        .clone();
    assert_eq!(stats(&game, "Nightmare"), (Some(1), Some(1)));
    assert_eq!(keywords(&game, "Nightmare"), Vec::<String>::new());
}
