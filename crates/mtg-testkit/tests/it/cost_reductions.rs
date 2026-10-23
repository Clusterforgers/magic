//! Goal: a cost reduction for the spells of two kinds ("Merfolk spells and Wizard spells you cast", "Blue spells and red spells you cast", "Angel spells and Human spells you cast ... for each +1/+1 counter") is the caster's own and leaves other spells alone, never takes a coloured symbol, stacks with another reduction and stops at the generic part; a reduction under a condition (a Dragon or a flier the caster controls, ten nonland permanents on the battlefield, ten creature cards in all graveyards, no basic land an opponent controls, life the caster gained this turn) holds exactly while the condition does, whoever else controls what it counts, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use mtg_cards::value::CounterKind;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// The table as seat 0 sees it: the hand `hand`, a dozen lands of every kind but one and the permanents `mine`, then
/// `theirs` on each other seat given with its place.
fn table(
    seats: usize,
    hand: &[&str],
    mine: Vec<PresetPermanent>,
    theirs: Vec<(usize, Vec<PresetPermanent>)>,
) -> Scenario {
    let mut battlefield = lands(4, island);
    battlefield.extend(lands(4, swamp));
    battlefield.extend(lands(4, forest));
    battlefield.extend(lands(4, plains));
    battlefield.extend(lands(4, mountain));
    battlefield.extend(mine);
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, battlefield);
    for (seat, permanents) in theirs {
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
}

/// The total cost at the payment of casting `name` from the priority of seat 0.
fn cost_of(scenario: Scenario, name: &str) -> Result<String, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(name)?;
    Ok(driver.pay_options()?.cost.to_string())
}

/// The total cost at the payment of casting `name` at the target called `target`.
fn cost_at(driver: &mut Driver, name: &str, target: &str) -> Result<String, ScriptError> {
    driver.begin_cast(name)?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    Ok(driver.pay_options()?.cost.to_string())
}

/// The cost as the engine writes it.
fn cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// `count` ready copies of a corpus permanent.
fn copies(name: &str, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(card(name).unwrap())).collect()
}

#[rstest]
#[case::mine_merfolk("Stonybrook Banneret", 0, "Atlantean Skirmisher", "{U}")]
#[case::mine_wizard("Stonybrook Banneret", 0, "Academy Elite", "{2}{U}")]
#[case::mine_neither("Stonybrook Banneret", 0, "Aven Squire", "{1}{W}")]
#[case::theirs_merfolk("Stonybrook Banneret", 1, "Atlantean Skirmisher", "{1}{U}")]
#[case::theirs_wizard("Stonybrook Banneret", 1, "Academy Elite", "{3}{U}")]
#[case::mine_blue("Nightscape Familiar", 0, "Academy Elite", "{2}{U}")]
#[case::mine_red("Nightscape Familiar", 0, "Bogardan Hellkite", "{5}{R}{R}")]
#[case::mine_green("Nightscape Familiar", 0, "Grizzly Bears", "{1}{G}")]
#[case::theirs_blue("Nightscape Familiar", 1, "Academy Elite", "{3}{U}")]
#[case::mine_goblin_or_rogue("Frogtosser Banneret", 0, "Atlantean Skirmisher", "{U}")]
#[case::mine_neither_goblin_nor_rogue("Frogtosser Banneret", 0, "Academy Elite", "{3}{U}")]
fn a_reduction_for_spells_of_two_kinds_is_the_casters_own(
    #[case] holder: &str,
    #[case] controller: usize,
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let reducer = vec![ready(card(holder).unwrap())];
    let (mine, theirs) = if controller == 0 {
        (reducer, vec![])
    } else {
        (vec![], vec![(controller, reducer)])
    };
    let scenario = table(seats, &[spell], mine, theirs);
    assert_eq!(cost_of(scenario, spell)?, cost(expected));
    Ok(())
}

#[rstest]
#[case::two_of_a_kind(&["Stonybrook Banneret", "Stonybrook Banneret"], "Atlantean Skirmisher", "{U}")]
#[case::two_kinds_stack(&["Stonybrook Banneret", "Nightscape Familiar"], "Academy Elite", "{1}{U}")]
#[case::three_stack_to_the_colored_symbol(&["Stonybrook Banneret", "Nightscape Familiar", "Stonybrook Banneret"], "Academy Elite", "{U}")]
fn reductions_stack_and_stop_at_the_generic_part(
    #[case] holders: &[&str],
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = holders
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect();
    let scenario = table(seats, &[spell], mine, vec![]);
    assert_eq!(cost_of(scenario, spell)?, cost(expected));
    Ok(())
}

#[rstest]
#[case::no_counter(0, "{2}{W}")]
#[case::one_counter(1, "{1}{W}")]
#[case::two_counters(2, "{W}")]
#[case::more_than_the_generic_part(5, "{W}")]
fn a_reduction_for_each_counter_on_the_reducer_follows_the_count(
    #[case] counters: u32,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let herald = with_counters(
        ready(card("Herald of War").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let scenario = table(seats, &["Angel of Vitality"], vec![herald], vec![]);
    assert_eq!(cost_of(scenario, "Angel of Vitality")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::mine("Bogardan Hellkite", 0, "{3}{U}")]
#[case::an_opponents("Bogardan Hellkite", 1, "{5}{U}")]
#[case::none("Grizzly Bears", 0, "{5}{U}")]
fn a_dragon_the_caster_controls_cheapens_the_spell(
    #[case] creature: &str,
    #[case] controller: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let dragon = vec![ready(card(creature).unwrap())];
    let (mine, theirs) = if controller == 0 {
        (dragon, vec![])
    } else {
        (vec![], vec![(controller, dragon)])
    };
    let scenario = table(seats, &["Draconic Lore"], mine, theirs);
    assert_eq!(cost_of(scenario, "Draconic Lore")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::mine("Aven Squire", 0, "{1}{U}")]
#[case::an_opponents("Aven Squire", 1, "{2}{U}")]
#[case::without_flying("Grizzly Bears", 0, "{2}{U}")]
fn a_creature_with_flying_the_caster_controls_cheapens_the_spell(
    #[case] creature: &str,
    #[case] controller: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let flier = vec![ready(card(creature).unwrap())];
    let (mine, theirs) = if controller == 0 {
        (flier, vec![])
    } else {
        (vec![], vec![(controller, flier)])
    };
    let scenario = table(seats, &["Winged Words"], mine, theirs);
    assert_eq!(cost_of(scenario, "Winged Words")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::nine_of_mine(9, 0, "{3}{W}{W}{W}")]
#[case::ten_of_mine(10, 0, "{W}{W}{W}")]
#[case::five_each(5, 5, "{W}{W}{W}")]
#[case::four_and_five(4, 5, "{3}{W}{W}{W}")]
#[case::more_than_ten(14, 6, "{W}{W}{W}")]
fn ten_nonland_permanents_of_the_table_cheapen_the_sweeper(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let others = if theirs == 0 {
        vec![]
    } else {
        vec![(seats - 1, copies("Grizzly Bears", theirs))]
    };
    let scenario = table(
        seats,
        &["Hour of Revelation"],
        copies("Grizzly Bears", mine),
        others,
    );
    assert_eq!(cost_of(scenario, "Hour of Revelation")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::nine(5, 4, "{6}{B}{B}")]
#[case::ten_of_mine(10, 0, "{B}{B}")]
#[case::ten_across_the_table(3, 7, "{B}{B}")]
#[case::spells_do_not_count(5, 0, "{6}{B}{B}")]
fn ten_creature_cards_in_all_graveyards_cheapen_the_avatar(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, &["Avatar of Woe"], vec![], vec![]);
    scenario = scenario.graveyard_named(0, vec!["Grizzly Bears"; mine]);
    if theirs > 0 {
        scenario = scenario.graveyard_named(seats - 1, vec!["Grizzly Bears"; theirs]);
    }
    if expected == "{6}{B}{B}" && theirs == 0 {
        // Bolts in a graveyard are not creature cards.
        scenario = scenario.graveyard_named(1, vec!["Lightning Bolt"; 6]);
    }
    assert_eq!(cost_of(scenario, "Avatar of Woe")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::no_basic_land(None, "{1}{B}{B}")]
#[case::a_basic_land_of_one(Some(0), "{2}{B}{B}")]
#[case::a_basic_land_of_the_last(Some(1), "{2}{B}{B}")]
fn no_basic_land_an_opponent_controls_cheapens_the_removal(
    #[case] holder: Option<usize>,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // `holder` counts from the first opponent (seat 1) to the last.
    let mut theirs = vec![(1, copies("Grizzly Bears", 1))];
    if let Some(at) = holder {
        let seat = 1 + at * (seats - 2);
        match theirs.iter_mut().find(|(place, _)| *place == seat) {
            Some((_, permanents)) => permanents.extend(lands(1, forest)),
            None => theirs.push((seat, lands(1, forest))),
        }
    }
    let scenario = table(seats, &["Hagra Mauling // Hagra Broodpit"], vec![], theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    let paid = cost_at(&mut driver, "Hagra Mauling", "Grizzly Bears")?;
    assert_eq!(paid, cost(expected));
    Ok(())
}

#[rstest]
#[case::after_gaining_life(true, "{B}{G}")]
#[case::without_gaining_life(false, "{2}{B}{G}")]
fn life_gained_this_turn_cheapens_the_removal(
    #[case] gain: bool,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand: &[&str] = if gain {
        &["Renewed Faith", "Mortality Spear"]
    } else {
        &["Mortality Spear"]
    };
    let target = vec![(1, copies("Grizzly Bears", 1))];
    let mut driver = Driver::from_scenario(table(seats, hand, vec![], target))?;
    if gain {
        driver.cast_tapping("Renewed Faith", &["Plains", "Plains", "Plains"])?;
        for _ in 0..seats {
            driver.pass()?;
        }
    }
    let paid = cost_at(&mut driver, "Mortality Spear", "Grizzly Bears")?;
    assert_eq!(paid, cost(expected));
    Ok(())
}
