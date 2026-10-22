//! Goal: mana abilities that add mixed mana, pay life, energy or counters of their source, wait for a condition or choose from the commander's colours or from what lands could produce are offered only while they can be used, add exactly their mana, pay their cost in one batch without the stack, and pay for spells with the fewest side effects; whole bot games with them keep every invariant at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::{Commander, Normal};
use mtg_engine::mana::ManaOption;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp, wastes};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, play_game_with, ready, with_counters};
use rstest::rstest;

/// A card by name: a basic land of the testkit or a corpus card.
fn definition(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        "Wastes" => wastes(),
        other => card(other).unwrap(),
    }
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A permanent by name: a basic land of the testkit or a corpus card.
fn permanent(name: &str) -> mtg_engine::PresetPermanent {
    ready(definition(name))
}

/// Seat 0 holds the spell and has `mine` in play, seat 1 has `theirs`.
fn table(cost: &str, mine: &[&str], theirs: &[&str]) -> Scenario {
    Scenario::new(2)
        .hand(0, [spell(cost)])
        .battlefield(0, mine.iter().map(|name| permanent(name)))
        .battlefield(1, theirs.iter().map(|name| permanent(name)))
}

fn offered(driver: &Driver) -> Vec<ManaOption> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.mana_abilities.clone(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn floating(driver: &Driver) -> u32 {
    driver.state().player(pid(0)).unwrap().mana().total()
}

fn energy(driver: &Driver) -> u32 {
    driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("energy".into()))
}

/// The mana a list of events adds, in order.
fn added(events: &[Event]) -> Vec<ManaType> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::the_two_it_makes("{W}{U}", &["Azorius Chancery"], true, 0)]
#[case::one_of_them_and_the_other_floats("{W}", &["Azorius Chancery"], true, 1)]
#[case::a_colour_it_does_not_make("{R}{W}", &["Azorius Chancery"], false, 0)]
#[case::generic_mana_from_both("{1}{1}", &["Azorius Chancery"], true, 0)]
#[case::three_with_a_plains("{W}{W}{U}", &["Azorius Chancery", "Plains"], true, 0)]
#[case::five_colours_from_one_tap("{W}{U}{B}{R}{G}", &["Timeless Lotus"], true, 0)]
#[case::four_of_five("{W}{U}{B}{R}", &["Timeless Lotus"], true, 1)]
fn a_land_that_adds_mixed_mana_pays_for_every_type_it_adds(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] affordable: bool,
    #[case] spare: u32,
) {
    let mut driver = Driver::from_scenario(table(cost, mine, &[])).unwrap();
    assert_eq!(castable(&driver), affordable);
    if affordable {
        driver.cast("Spell").unwrap();
        assert_eq!(floating(&driver), spare);
    }
}

#[test]
fn a_bounceland_adds_both_mana_in_one_batch_and_taps() {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Azorius Chancery"], &[])).unwrap();
    let found = offered(&driver);
    assert_eq!(found.len(), 1, "one way to use it: {found:?}");
    assert_eq!(found[0].made(), [ManaType::White, ManaType::Blue]);
    let events = driver.tap_for_mana("Azorius Chancery").unwrap();
    assert_eq!(added(&events), [ManaType::White, ManaType::Blue]);
    assert!(matches!(events[0], Event::PermanentTapped { .. }));
    assert_eq!(floating(&driver), 2);
}

#[rstest]
#[case::plenty_of_life(20, true, 19)]
#[case::two_life(2, true, 1)]
#[case::one_life_would_end_the_game(1, false, 1)]
fn a_land_that_pays_life_for_its_mana_never_takes_the_payer_to_zero_in_a_payment(
    #[case] start: i32,
    #[case] affordable: bool,
    #[case] after: i32,
) {
    let mut driver =
        Driver::from_scenario(table("{G}", &["Horizon Canopy"], &[]).life(0, start)).unwrap();
    assert_eq!(castable(&driver), affordable);
    if affordable {
        driver.cast("Spell").unwrap();
    }
    assert_eq!(life(&driver), after);
}

#[test]
fn paying_life_for_mana_is_part_of_the_activation() {
    let mut driver =
        Driver::from_scenario(table("{G}", &["Horizon Canopy"], &[]).life(0, 5)).unwrap();
    let found = offered(&driver);
    assert_eq!(
        found
            .iter()
            .map(|o| (o.produces, o.life))
            .collect::<Vec<_>>(),
        [(ManaType::Green, 1), (ManaType::White, 1)]
    );
    let events = driver
        .tap_for_mana_of("Horizon Canopy", ManaType::White)
        .unwrap();
    assert!(
        matches!(events[0], Event::LifePaid { amount: 1, .. }),
        "{events:?}"
    );
    assert_eq!(added(&events), [ManaType::White]);
    assert_eq!(life(&driver), 4);
}

#[test]
fn a_land_that_pays_life_is_used_only_when_a_land_that_costs_nothing_cannot_pay() {
    let mut driver =
        Driver::from_scenario(table("{G}", &["Horizon Canopy", "Forest"], &[]).life(0, 20))
            .unwrap();
    driver.cast("Spell").unwrap();
    assert_eq!(life(&driver), 20, "the Forest pays, not the Canopy");
    let mut driver =
        Driver::from_scenario(table("{G}{W}", &["Horizon Canopy", "Forest"], &[]).life(0, 20))
            .unwrap();
    driver.cast("Spell").unwrap();
    assert_eq!(life(&driver), 19, "the Canopy pays for the white mana");
}

#[rstest]
#[case::two_energy(2, true)]
#[case::one_energy(1, true)]
#[case::no_energy(0, false)]
fn mana_of_any_colour_for_energy_is_offered_while_there_is_energy(
    #[case] held: u32,
    #[case] offered_any: bool,
) {
    let scenario =
        table("{U}", &["Aether Hub"], &[]).counters(0, CounterKind::Other("energy".into()), held);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(castable(&driver), offered_any);
    assert_eq!(
        offered(&driver).len(),
        if offered_any { 6 } else { 1 },
        "colourless alone without energy, with a colour of each type otherwise"
    );
    if offered_any {
        driver.cast("Spell").unwrap();
        assert_eq!(energy(&driver), held - 1);
    }
}

#[test]
fn counters_removed_as_a_cost_are_gone_and_the_ability_waits_for_the_next() {
    let bridge = with_counters(permanent("Tendo Ice Bridge"), CounterKind::Charge, 1);
    let scenario = Scenario::new(2)
        .hand(0, [spell("{U}")])
        .battlefield(0, [bridge]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(offered(&driver).len(), 6);
    let events = driver
        .tap_for_mana_of("Tendo Ice Bridge", ManaType::Blue)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::CountersChanged(c) if c.delta == -1)),
        "{events:?}"
    );
    let id = driver.id_of("Tendo Ice Bridge").unwrap();
    assert_eq!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::Charge),
        0
    );
}

#[rstest]
#[case::with_a_swamp(&["Tainted Isle", "Swamp"], 3)]
#[case::without_one(&["Tainted Isle", "Island"], 1)]
#[case::alone(&["Tainted Isle"], 1)]
fn an_ability_that_is_activated_only_if_you_control_a_swamp_waits_for_one(
    #[case] mine: &[&str],
    #[case] colourless_and_choice_of_two: usize,
) {
    let driver = Driver::from_scenario(table("{U}", mine, &[])).unwrap();
    let found = offered(&driver);
    let from_isle: Vec<_> = found
        .iter()
        .filter(|o| driver.state().object(o.source.object).unwrap().card().name == "Tainted Isle")
        .collect();
    assert_eq!(from_isle.len(), colourless_and_choice_of_two, "{found:?}");
}

fn commander_table(commander_colors: &[Color]) -> Driver {
    let scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, commander_colors, 2, 2)])
        .battlefield(0, [permanent("Command Tower"), permanent("Arcane Signet")]);
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::mono(&[Color::Red], &[ManaType::Red])]
#[case::two_colours(&[Color::White, Color::Blue], &[ManaType::White, ManaType::Blue])]
#[case::three_colours(&[Color::Black, Color::Red, Color::Green], &[ManaType::Black, ManaType::Red, ManaType::Green])]
#[case::none(&[], &[])]
fn the_commanders_identity_decides_the_colours_a_tower_and_a_signet_add(
    #[case] commander: &[Color],
    #[case] colours: &[ManaType],
) {
    let driver = commander_table(commander);
    let found = offered(&driver);
    for name in ["Command Tower", "Arcane Signet"] {
        let kinds: Vec<ManaType> = found
            .iter()
            .filter(|o| driver.state().object(o.source.object).unwrap().card().name == name)
            .map(|o| o.produces)
            .collect();
        assert_eq!(kinds, colours, "{name}");
    }
}

#[test]
fn a_tower_adds_nothing_without_a_commander() {
    let driver = Driver::from_scenario(table("{R}", &["Command Tower"], &[])).unwrap();
    assert!(offered(&driver).is_empty());
    assert!(!castable(&driver));
}

#[test]
fn a_tower_pays_for_a_spell_in_the_commanders_colour() {
    let scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .hand(0, [spell("{R}")])
        .battlefield(0, [permanent("Command Tower")]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(castable(&driver));
    let events = driver.cast("Spell").unwrap();
    assert!(added(&events).contains(&ManaType::Red), "{events:?}");
}

#[rstest]
#[case::an_opponents_forest(&["Exotic Orchard"], &["Forest"], &[ManaType::Green])]
#[case::two_kinds(&["Exotic Orchard"], &["Forest", "Island"], &[ManaType::Blue, ManaType::Green])]
#[case::a_land_of_any_colour(&["Exotic Orchard"], &["Command Tower"], &[])]
#[case::nothing_colourless(&["Exotic Orchard"], &["Wastes"], &[])]
#[case::no_lands(&["Exotic Orchard"], &[], &[])]
#[case::my_own_lands_do_not_count(&["Exotic Orchard", "Forest"], &[], &[])]
#[case::two_orchards_depend_on_each_other(&["Exotic Orchard"], &["Exotic Orchard"], &[])]
#[case::an_opponents_pain_land(&["Exotic Orchard"], &["Horizon Canopy"], &[ManaType::White, ManaType::Green])]
fn mana_that_an_opponents_land_could_produce_is_the_colours_those_lands_make(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] colours: &[ManaType],
) {
    let driver = Driver::from_scenario(table("{G}", mine, theirs)).unwrap();
    let found = offered(&driver);
    let kinds: Vec<ManaType> = found
        .iter()
        .filter(|o| driver.state().object(o.source.object).unwrap().card().name == "Exotic Orchard")
        .map(|o| o.produces)
        .collect();
    let mut expected = colours.to_vec();
    expected.sort_by_key(|mana| mana.index());
    assert_eq!(kinds, expected);
}

#[rstest]
#[case::the_lands_i_control("Reflecting Pool", &["Reflecting Pool", "Forest"], &[ManaType::Green])]
#[case::colourless_counts_as_a_type("Reflecting Pool", &["Reflecting Pool", "Command Tower", "Wastes"], &[ManaType::Colorless])]
#[case::a_stone_sees_the_opponents_and_not_mine("Fellwar Stone", &["Fellwar Stone", "Forest"], &[])]
fn mana_of_a_type_my_lands_could_produce(
    #[case] source: &str,
    #[case] mine: &[&str],
    #[case] types: &[ManaType],
) {
    let driver = Driver::from_scenario(table("{G}", mine, &[])).unwrap();
    let kinds: Vec<ManaType> = offered(&driver)
        .iter()
        .filter(|o| driver.state().object(o.source.object).unwrap().card().name == source)
        .map(|o| o.produces)
        .collect();
    assert_eq!(kinds, types);
}

#[test]
fn a_summoning_sick_creature_still_pays_a_cost_without_a_tap() {
    let pent = with_counters(
        permanent("Servant of the Conduit"),
        CounterKind::Other("energy".into()),
        0,
    );
    let scenario = Scenario::new(2)
        .hand(0, [spell("{U}")])
        .counters(0, CounterKind::Other("energy".into()), 1)
        .battlefield(
            0,
            [mtg_testkit::sick(card("Servant of the Conduit").unwrap())],
        )
        .battlefield(1, [pent]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert!(offered(&driver).is_empty(), "its cost has a {{T}}");
}

/// Sixty cards: lands and mana sources of every kind of this task, with creatures and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 11),
        ("Island", 6),
        ("Swamp", 4),
        ("Azorius Chancery", 2),
        ("Timeless Lotus", 1),
        ("Horizon Canopy", 3),
        ("Aether Hub", 2),
        ("Tendo Ice Bridge", 2),
        ("Tainted Isle", 2),
        ("Exotic Orchard", 2),
        ("Fellwar Stone", 2),
        ("Reflecting Pool", 1),
        ("Command Tower", 2),
        ("Arcane Signet", 2),
        ("Servant of the Conduit", 2),
        ("Llanowar Elves", 3),
        ("Grizzly Bears", 4),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    mixed_at_once: u64,
    life_paid: u64,
    energy_paid: u64,
    counters_removed: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    for pair in events.windows(2) {
        match (&pair[0], &pair[1]) {
            (
                Event::ManaAdded {
                    mana: a,
                    source: Some(x),
                    ..
                },
                Event::ManaAdded {
                    mana: b,
                    source: Some(y),
                    ..
                },
            ) if x == y && a != b => seen.mixed_at_once += 1,
            (Event::LifePaid { .. }, Event::PermanentTapped { .. }) => seen.life_paid += 1,
            (Event::CountersChanged(c), Event::PermanentTapped { .. }) if c.delta < 0 => {
                if matches!(c.on, mtg_engine::choose::Target::Player(_)) {
                    seen.energy_paid += 1;
                } else {
                    seen.counters_removed += 1;
                }
            }
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_mana_abilities_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        tally(&outcome.events, &mut seen.lock().unwrap());
    });
    let seen = seen.into_inner().unwrap();
    assert!(
        seen.mixed_at_once > 0 && seen.life_paid > 0,
        "the games should use the abilities: mixed {}, life {}, energy {}, counters {}",
        seen.mixed_at_once,
        seen.life_paid,
        seen.energy_paid,
        seen.counters_removed
    );
}

#[test]
fn a_seeded_game_with_these_mana_abilities_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(11), play(11));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
