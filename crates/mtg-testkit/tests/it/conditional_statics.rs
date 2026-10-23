//! Goal: the statics under "as long as" that the compiler reads from card text hold exactly while their condition holds, at two, three and four seats: a God is a creature only with enough devotion (mana symbols of its colors among the permanents its controller controls, hybrid symbols counted once, opponents' permanents not at all) and stops being one the moment a permanent changes hands; delirium counts the card types in the graveyard of the controller alone; a quest enchantment pumps the creatures its controller has once it holds enough counters.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;
use std::sync::Arc;

/// The corpus card of that name.
fn card(name: &str) -> Result<Arc<CardDefinition>, ScriptError> {
    mtg_testkit::corpus::card(name).map_err(|error| ScriptError::Stuck(error.to_string()))
}

/// A 1/1 creature of that name with that mana cost.
fn costing(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A card of one type with nothing else, for a graveyard.
fn of_type(name: &str, card_type: CardType) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(card_type))
}

fn stats(driver: &Driver, name: &str) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power, chars.toughness)
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .types
        .contains(CardType::Creature)
}

/// Thassa, God of the Sea ({1}{U}, a creature with devotion to blue of five) among the permanents of cost `mine` of seat 0
/// and `theirs` of seat 1.
fn thassa(seats: usize, mine: &[&str], theirs: &[&str]) -> Result<Driver, ScriptError> {
    let named = |costs: &[&str], side: &str| {
        costs
            .iter()
            .enumerate()
            .map(|(at, cost)| ready(costing(&format!("{side}{at}"), cost)))
            .collect::<Vec<_>>()
    };
    let mut yours = named(mine, "Mine");
    yours.push(ready(card("Thassa, God of the Sea")?));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, yours)
        .battlefield(1, named(theirs, "Theirs"));
    Driver::from_scenario(scenario)
}

/// Devotion to blue counts each blue mana symbol of the permanents you control (Thassa herself is one).
#[rstest]
#[case::alone(&[], &[], false)]
#[case::five_symbols(&["{U}{U}", "{U}{U}"], &[], true)]
#[case::four_symbols(&["{U}{U}", "{U}"], &[], false)]
#[case::opponents_symbols_do_not_count(&[], &["{U}{U}", "{U}{U}", "{U}{U}"], false)]
#[case::yours_with_theirs(&["{U}{U}", "{U}{U}"], &["{U}{U}{U}{U}"], true)]
#[case::hybrid_symbols_count_once_each(&["{U/B}{U/B}", "{U}{U}"], &[], true)]
#[case::generic_and_other_colors_do_not_count(&["{3}{U}", "{R}{R}{R}", "{2}{G}"], &[], false)]
#[case::generic_beside_blue(&["{3}{U}", "{3}{U}", "{3}{U}", "{3}{U}"], &[], true)]
fn a_god_is_a_creature_only_with_enough_devotion(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] creature: bool,
) -> Result<(), ScriptError> {
    let driver = thassa(seats, mine, theirs)?;
    assert_eq!(is_creature(&driver, "Thassa, God of the Sea"), creature);
    Ok(())
}

/// A permanent that changes hands moves the devotion with it, in the middle of the turn.
#[rstest]
fn the_god_stops_being_a_creature_when_a_permanent_changes_hands(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = thassa(seats, &["{U}{U}", "{U}{U}"], &[])?;
    assert!(is_creature(&driver, "Thassa, God of the Sea"));
    let first = driver.id_of("Mine0")?;
    driver.add_effect(
        driver.on(first),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    assert!(!is_creature(&driver, "Thassa, God of the Sea"));
    let theirs = driver.id_of("Mine0")?;
    driver.add_effect(
        driver.on(theirs),
        Modification::SetController(pid(0)),
        Duration::Forever,
    )?;
    assert!(is_creature(&driver, "Thassa, God of the Sea"));
    Ok(())
}

/// Delirium: four or more card types among the cards in the controller's graveyard (Gnarlwood Dryad is a 1/1).
#[rstest]
#[case::no_cards(&[], &[], 1)]
#[case::three_types(&[CardType::Land, CardType::Creature, CardType::Instant], &[], 1)]
#[case::four_types(&[CardType::Land, CardType::Creature, CardType::Instant, CardType::Sorcery], &[], 3)]
#[case::one_type_many_times(&[CardType::Land, CardType::Land, CardType::Land, CardType::Land], &[], 1)]
#[case::an_opponents_graveyard_does_not_count(&[CardType::Land], &[CardType::Creature, CardType::Instant, CardType::Sorcery], 1)]
#[case::five_types(&[CardType::Land, CardType::Creature, CardType::Instant, CardType::Sorcery, CardType::Artifact], &[], 3)]
fn delirium_counts_the_types_in_ones_own_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[CardType],
    #[case] theirs: &[CardType],
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let graveyard = |types: &[CardType], side: &str| {
        types
            .iter()
            .enumerate()
            .map(|(at, card_type)| of_type(&format!("{side}{at}"), *card_type))
            .collect::<Vec<_>>()
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, vec![ready(card("Gnarlwood Dryad")?)])
        .graveyard(0, graveyard(mine, "Mine"))
        .graveyard(1, graveyard(theirs, "Theirs"));
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        stats(&driver, "Gnarlwood Dryad"),
        (Some(power), Some(power)),
        "{mine:?} {theirs:?}"
    );
    Ok(())
}

/// Quest enchantments: the creatures of the controller get the bonus from the counter that reaches the threshold on.
#[rstest]
#[case::quest_below("Quest for the Goblin Lord", 4, (2, 2))]
#[case::quest_at("Quest for the Goblin Lord", 5, (4, 2))]
#[case::quest_above("Quest for the Goblin Lord", 9, (4, 2))]
#[case::ascension_below("Beastmaster Ascension", 6, (2, 2))]
#[case::ascension_at("Beastmaster Ascension", 7, (7, 7))]
fn a_quest_enchantment_pumps_the_creatures_of_its_controller_once_it_holds_enough_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] quest: &str,
    #[case] counters: u32,
    #[case] expected: (i32, i32),
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            vec![
                with_counters(
                    ready(card(quest)?),
                    CounterKind::Other("quest".to_owned()),
                    counters,
                ),
                ready(free_creature("Mine", 2, 2)),
            ],
        )
        .battlefield(1, vec![ready(free_creature("Theirs", 2, 2))]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(stats(&driver, "Mine"), (Some(expected.0), Some(expected.1)));
    assert_eq!(stats(&driver, "Theirs"), (Some(2), Some(2)));
    Ok(())
}
