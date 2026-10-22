//! Goal: a filter land ("{W/U}, {T}: Add {W}{W}, {W}{U}, or {U}{U}") is a source that costs mana: it pays for a spell from the mana another source makes, never from its own, is offered as one source for each list of mana, adds every mana of the list in one batch, is seen by the mana a land could produce, and whole bot games with the ten filter lands keep every invariant at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::mana::ManaOption;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, play_game_with, ready};
use rstest::rstest;

/// The ten filter lands of the corpus.
const FILTER_LANDS: [&str; 10] = [
    "Mystic Gate",
    "Sunken Ruins",
    "Graven Cairns",
    "Fire-Lit Thicket",
    "Wooded Bastion",
    "Fetid Heath",
    "Cascade Bluffs",
    "Twilight Mire",
    "Rugged Prairie",
    "Flooded Grove",
];

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

/// Seat 0 holds the spell and has `mine` in play.
fn table(cost: &str, mine: &[&str]) -> Scenario {
    Scenario::new(2)
        .hand(0, [spell(cost)])
        .battlefield(0, mine.iter().map(|name| ready(definition(name))))
}

fn priority_of(driver: &Driver) -> &mtg_engine::decision::PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options,
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn floating(driver: &Driver) -> u32 {
    driver.state().player(pid(0)).unwrap().mana().total()
}

fn of_land<'a>(options: &'a [ManaOption], driver: &Driver, name: &str) -> Vec<&'a ManaOption> {
    options
        .iter()
        .filter(|o| driver.state().object(o.source.object).unwrap().card().name == name)
        .collect()
}

#[rstest]
#[case::a_land_of_the_first_colour("{W}{W}", &["Island", "Mystic Gate"], true, 0)]
#[case::a_land_of_the_second_colour("{U}{U}", &["Plains", "Mystic Gate"], true, 0)]
#[case::one_of_each("{W}{U}", &["Plains", "Mystic Gate"], true, 0)]
#[case::the_spare_mana_floats("{W}", &["Island", "Mystic Gate"], true, 1)]
#[case::three_mana_from_two_lands_and_a_filter("{W}{W}{W}", &["Plains", "Plains", "Mystic Gate"], true, 0)]
#[case::the_free_ability_pays_with_a_land_of_another_colour("{2}", &["Forest", "Mystic Gate"], true, 0)]
#[case::the_filter_alone_has_nothing_to_pay_with("{W}{W}", &["Mystic Gate"], false, 0)]
#[case::a_land_that_cannot_pay_the_hybrid_symbol("{W}{W}", &["Forest", "Mystic Gate"], false, 0)]
#[case::a_colour_the_filter_does_not_make("{B}{B}", &["Plains", "Mystic Gate"], false, 0)]
#[case::two_filters_in_a_chain("{U}{B}{B}", &["Plains", "Mystic Gate", "Sunken Ruins"], true, 0)]
#[case::two_filters_cannot_start_each_other("{U}{B}{B}", &["Mystic Gate", "Sunken Ruins"], false, 0)]
#[case::the_second_pair_of_colours("{G}{G}{G}", &["Forest", "Forest", "Wooded Bastion"], true, 0)]
#[case::the_other_hybrid_colour_pays_too("{R}{G}", &["Forest", "Fire-Lit Thicket"], true, 0)]
#[case::two_filters_of_one_pair("{R}{R}{R}{R}", &["Mountain", "Mountain", "Fire-Lit Thicket", "Fire-Lit Thicket"], true, 0)]
fn a_filter_land_pays_only_from_other_mana(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] affordable: bool,
    #[case] spare: u32,
) {
    let mut driver = Driver::from_scenario(table(cost, mine)).unwrap();
    assert_eq!(!priority_of(&driver).castable.is_empty(), affordable);
    if affordable {
        driver.cast("Spell").unwrap();
        assert_eq!(floating(&driver), spare);
    }
}

#[test]
fn a_filter_land_is_one_source_for_each_list_and_one_for_the_colorless_ability() {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Plains", "Mystic Gate"])).unwrap();
    driver.begin_cast("Spell").unwrap();
    let sources = &driver.pay_options().unwrap().sources;
    let gate = of_land(sources, &driver, "Mystic Gate");
    let shape: Vec<(ManaType, u32, Vec<ManaType>, bool)> = gate
        .iter()
        .map(|o| (o.produces, o.count, o.also.clone(), o.is_costly()))
        .collect();
    assert_eq!(
        shape,
        [
            (ManaType::Colorless, 1, vec![], false),
            (ManaType::White, 2, vec![], true),
            (ManaType::White, 1, vec![ManaType::Blue], true),
            (ManaType::Blue, 2, vec![], true),
        ]
    );
    let mut keys: Vec<usize> = gate.iter().map(|o| o.source.ability).collect();
    keys.dedup();
    assert_eq!(keys.len(), 4, "every list is its own source");
}

#[test]
fn only_the_free_ability_of_a_filter_land_is_offered_at_priority() {
    let driver = Driver::from_scenario(table("{W}", &["Plains", "Mystic Gate"])).unwrap();
    let offered = &priority_of(&driver).mana_abilities;
    let gate = of_land(offered, &driver, "Mystic Gate");
    assert_eq!(gate.len(), 1);
    assert_eq!(gate[0].produces, ManaType::Colorless);
}

#[test]
fn the_cost_is_spent_from_the_pool_just_before_the_filter_taps_and_the_whole_list_is_added() {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Plains", "Mystic Gate"])).unwrap();
    let events = driver.cast("Spell").unwrap();
    let kinds: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { .. } => Some("tap".to_owned()),
            Event::ManaAdded { mana, .. } => Some(format!("add {mana:?}")),
            Event::ManaSpent { mana, .. } => Some(format!("spend {mana:?}")),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "tap",
            "add White",
            "spend [White]",
            "tap",
            "add White",
            "add Blue",
            "spend [White, Blue]"
        ]
    );
}

#[rstest]
#[case::the_lands_i_control_see_the_two_colours_of_a_filter("Reflecting Pool", &["Reflecting Pool", "Mystic Gate"], &[ManaType::White, ManaType::Blue, ManaType::Colorless])]
#[case::a_pair_without_the_colourless_one("Reflecting Pool", &["Reflecting Pool", "Mystic Gate", "Forest"], &[ManaType::White, ManaType::Blue, ManaType::Green, ManaType::Colorless])]
fn the_mana_a_land_could_produce_includes_every_type_of_a_filter(
    #[case] source: &str,
    #[case] mine: &[&str],
    #[case] types: &[ManaType],
) {
    let driver = Driver::from_scenario(table("{G}", mine)).unwrap();
    let mut kinds: Vec<ManaType> = of_land(&priority_of(&driver).mana_abilities, &driver, source)
        .iter()
        .map(|o| o.produces)
        .collect();
    kinds.sort_by_key(|mana| mana.index());
    let mut expected = types.to_vec();
    expected.sort_by_key(|mana| mana.index());
    assert_eq!(kinds, expected);
}

#[rstest]
#[case::mystic_gate("Mystic Gate", ("Plains", 'W'), ("Island", 'U'), "Swamp")]
#[case::sunken_ruins("Sunken Ruins", ("Island", 'U'), ("Swamp", 'B'), "Plains")]
#[case::graven_cairns("Graven Cairns", ("Swamp", 'B'), ("Mountain", 'R'), "Plains")]
#[case::fire_lit_thicket("Fire-Lit Thicket", ("Mountain", 'R'), ("Forest", 'G'), "Plains")]
#[case::wooded_bastion("Wooded Bastion", ("Forest", 'G'), ("Plains", 'W'), "Swamp")]
#[case::fetid_heath("Fetid Heath", ("Plains", 'W'), ("Swamp", 'B'), "Forest")]
#[case::cascade_bluffs("Cascade Bluffs", ("Island", 'U'), ("Mountain", 'R'), "Plains")]
#[case::twilight_mire("Twilight Mire", ("Swamp", 'B'), ("Forest", 'G'), "Plains")]
#[case::rugged_prairie("Rugged Prairie", ("Mountain", 'R'), ("Plains", 'W'), "Swamp")]
#[case::flooded_grove("Flooded Grove", ("Forest", 'G'), ("Island", 'U'), "Plains")]
fn every_filter_land_makes_its_pair_from_a_land_of_either_colour_and_from_no_other(
    #[case] land: &str,
    #[case] first: (&str, char),
    #[case] second: (&str, char),
    #[case] outside: &str,
) {
    let ((one, a), (other, b)) = (first, second);
    for (basic, cost) in [
        (one, format!("{{{a}}}{{{a}}}")),
        (other, format!("{{{b}}}{{{b}}}")),
        (one, format!("{{{a}}}{{{b}}}")),
        (other, format!("{{{a}}}{{{b}}}")),
    ] {
        let driver = Driver::from_scenario(table(&cost, &[basic, land])).unwrap();
        assert!(
            !priority_of(&driver).castable.is_empty(),
            "{basic} and {land} make {cost}"
        );
    }
    let driver =
        Driver::from_scenario(table(&format!("{{{a}}}{{{a}}}"), &[outside, land])).unwrap();
    assert!(
        priority_of(&driver).castable.is_empty(),
        "{outside} cannot pay for {land}"
    );
}

/// Sixty cards: the ten filter lands, basic lands and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut cards: Vec<Arc<CardDefinition>> = FILTER_LANDS.iter().map(|n| definition(n)).collect();
    cards.extend(
        [
            ("Forest", 7),
            ("Island", 6),
            ("Swamp", 5),
            ("Plains", 6),
            ("Mountain", 6),
            ("Llanowar Elves", 3),
            ("Grizzly Bears", 4),
            ("Serra Angel", 3),
            ("Giant Growth", 3),
            ("Lightning Bolt", 4),
            ("Counterspell", 3),
        ]
        .into_iter()
        .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name))),
    );
    cards
}

/// What a campaign saw: costs paid from the pool right before a tap, and two kinds of mana added by one source.
#[derive(Default)]
struct Seen {
    costs_paid: u64,
    mixed_at_once: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    for pair in events.windows(2) {
        match (&pair[0], &pair[1]) {
            (Event::ManaSpent { .. }, Event::PermanentTapped { .. }) => seen.costs_paid += 1,
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
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_filter_lands_finish_and_keep_every_invariant(#[case] players: usize) {
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
        seen.costs_paid > 0 && seen.mixed_at_once > 0,
        "the games should use the filters: costs {}, mixed {}",
        seen.costs_paid,
        seen.mixed_at_once
    );
}

#[test]
fn a_seeded_game_with_the_filter_lands_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(11), play(11));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
