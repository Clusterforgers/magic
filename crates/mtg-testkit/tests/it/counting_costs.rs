//! Goal: a cost reduction that counts (the creatures of the table or of the opponents, the land cards of the caster's graveyard, the total power of the caster's creatures, the mana value of the caster's noncreature enchantments, the attacking creatures) follows the count as it changes, takes generic mana down to nothing and a coloured symbol never, comes after the increases of other permanents, and counts only what its words name; a reduction for spells cast from anywhere other than a hand is the caster's own and leaves spells from a hand alone; a group that enters tapped taps the permanents of its kind that the opponents of its controller put onto the battlefield and no others, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::cost_cards::modifier;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The total cost at the payment of casting `name` from the priority of seat 0.
fn cost_of(scenario: Scenario, name: &str) -> Result<String, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(name)?;
    Ok(driver.pay_options()?.cost.to_string())
}

/// `count` untapped corpus permanents of this name.
fn copies(name: &str, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(card(name).unwrap())).collect()
}

/// Seat 0 holds `cast` with `count` basic lands of one kind, then the permanents `more`; the other seat has `theirs`.
fn table(
    cast: &str,
    land: fn() -> Arc<CardDefinition>,
    count: usize,
    more: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Scenario {
    let mut mine = lands(count, land);
    mine.extend(more);
    Scenario::new(2)
        .hand_named(0, [cast])
        .battlefield(0, mine)
        .battlefield(1, theirs)
}

/// The cost as the engine writes it.
fn cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

#[rstest]
#[case::no_creature_no_discount(0, 0, "{6}{W}{W}")]
#[case::my_creatures_count(2, 0, "{4}{W}{W}")]
#[case::so_do_theirs(0, 3, "{3}{W}{W}")]
#[case::both_together(2, 3, "{1}{W}{W}")]
#[case::down_to_the_colored_symbols(3, 3, "{W}{W}")]
#[case::never_below_the_colored_symbols(5, 5, "{W}{W}")]
fn a_reduction_for_each_creature_on_the_battlefield_follows_the_count(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let scenario = table(
        "Vanquish the Horde",
        plains,
        8,
        copies("Grizzly Bears", mine),
        copies("Grizzly Bears", theirs),
    );
    assert_eq!(cost_of(scenario, "Vanquish the Horde")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::none_of_theirs(3, 0, "{10}{G}")]
#[case::each_of_theirs_counts(0, 4, "{6}{G}")]
#[case::mine_do_not(3, 4, "{6}{G}")]
#[case::all_the_generic_mana(0, 10, "{G}")]
#[case::and_no_more(0, 12, "{G}")]
fn a_reduction_for_each_creature_your_opponents_control_counts_only_theirs(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let scenario = table(
        "Primeval Protector",
        forest,
        11,
        copies("Grizzly Bears", mine),
        copies("Grizzly Bears", theirs),
    );
    assert_eq!(cost_of(scenario, "Primeval Protector")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::an_empty_graveyard(0, 0, 0, "{10}{G}")]
#[case::my_land_cards(3, 0, 0, "{7}{G}")]
#[case::their_land_cards_do_not_count(3, 5, 0, "{7}{G}")]
#[case::other_cards_do_not_count(2, 0, 4, "{8}{G}")]
#[case::enough_to_leave_the_color(11, 0, 0, "{G}")]
fn a_reduction_for_each_land_card_in_your_graveyard_counts_only_your_own(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] spells: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut graveyard = vec!["Forest"; mine];
    graveyard.extend(vec!["Lightning Bolt"; spells]);
    let scenario = table("Rumbleweed", forest, 11, Vec::new(), Vec::new())
        .graveyard_named(0, graveyard)
        .graveyard_named(1, vec!["Forest"; theirs]);
    assert_eq!(cost_of(scenario, "Rumbleweed")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::no_creature(0, 0, "{10}{G}{G}")]
#[case::each_point_of_power(3, 0, "{4}{G}{G}")]
#[case::their_creatures_do_not_count(2, 4, "{6}{G}{G}")]
#[case::to_the_colored_symbols(6, 0, "{G}{G}")]
fn a_reduction_by_the_total_power_of_your_creatures_adds_their_power(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    // A Grizzly Bears has two power.
    let scenario = table(
        "Ghalta, Primal Hunger",
        forest,
        12,
        copies("Grizzly Bears", mine),
        copies("Grizzly Bears", theirs),
    );
    assert_eq!(cost_of(scenario, "Ghalta, Primal Hunger")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::no_enchantment(&[], "{10}{G}{G}")]
#[case::one_of_value_one(&["Authority of the Consuls"], "{9}{G}{G}")]
#[case::two_add_up(&["Authority of the Consuls", "Authority of the Consuls"], "{8}{G}{G}")]
#[case::a_creature_enchantment_does_not_count(&["Nyxborn Behemoth"], "{10}{G}{G}")]
fn a_reduction_by_the_mana_value_of_your_noncreature_enchantments_adds_them_up(
    #[case] enchantments: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let more = enchantments
        .iter()
        .flat_map(|name| copies(name, 1))
        .collect();
    let scenario = table("Nyxborn Behemoth", forest, 12, more, Vec::new());
    assert_eq!(cost_of(scenario, "Nyxborn Behemoth")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::the_increase_comes_first(1, 2, "{5}{W}{W}")]
#[case::two_increases(2, 2, "{6}{W}{W}")]
#[case::a_reduction_that_exceeds_the_increase(1, 8, "{W}{W}")]
fn a_counting_reduction_comes_after_the_increases_of_other_permanents(
    #[case] taxes: usize,
    #[case] creatures: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let rocks = (0..taxes).map(|_| ready(modifier("Tax Rock").unwrap()));
    let scenario = table(
        "Vanquish the Horde",
        plains,
        12,
        copies("Grizzly Bears", creatures),
        Vec::new(),
    )
    .battlefield(1, rocks.collect::<Vec<_>>());
    assert_eq!(cost_of(scenario, "Vanquish the Horde")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::one(1, "{9}")]
#[case::three(3, "{7}")]
fn a_reduction_for_each_attacking_creature_counts_the_attackers_of_the_turn(
    #[case] attackers: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut mine = lands(10, forest);
    mine.extend(copies("Grizzly Bears", 4));
    let scenario = Scenario::new(2)
        .at(1, 0, Step::DeclareAttackers)
        .hand_named(0, ["Ancient Stone Idol"])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&vec![("Grizzly Bears", 1); attackers])?;
    driver.begin_cast("Ancient Stone Idol")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost(expected));
    Ok(())
}

/// Seat 0 has Faithless Looting in the graveyard, a Bears in the hand, `count` Mountains and `sages` Sages of the Beyond;
/// the other seat has `theirs`.
fn flashback(sages: usize, theirs: usize, count: usize) -> Scenario {
    let mut mine = lands(count, mountain);
    mine.extend(lands(2, forest));
    mine.extend(copies("Sage of the Beyond", sages));
    Scenario::new(2)
        .graveyard_named(0, ["Faithless Looting"])
        .hand_named(0, ["Grizzly Bears"])
        .battlefield(0, mine)
        .battlefield(1, copies("Sage of the Beyond", theirs))
}

#[rstest]
#[case::no_sage(0, 0, "{2}{R}")]
#[case::my_sage(1, 0, "{R}")]
#[case::two_sages_cannot_go_lower(2, 0, "{R}")]
#[case::their_sage_is_not_mine(0, 1, "{2}{R}")]
fn spells_cast_from_a_graveyard_cost_less_under_a_sage_of_the_beyond(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    assert_eq!(
        cost_of(flashback(mine, theirs, 3), "Faithless Looting")?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::no_sage(0)]
#[case::one(1)]
#[case::two(2)]
fn spells_cast_from_a_hand_cost_what_they_cost_under_a_sage_of_the_beyond(
    #[case] sages: usize,
) -> Result<(), ScriptError> {
    assert_eq!(
        cost_of(flashback(sages, 0, 3), "Grizzly Bears")?,
        cost("{1}{G}")
    );
    Ok(())
}

/// Whether the Bears the seat `active` casts enter tapped while seat 0 holds `holder`.
fn bears_enter_tapped(holder: &str, seats: usize, active: usize) -> Result<bool, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .battlefield(0, copies(holder, 1))
        .hand_named(active, ["Grizzly Bears"])
        .battlefield(
            active,
            if active == 0 {
                let mut both = copies(holder, 1);
                both.extend(lands(2, forest));
                both
            } else {
                lands(2, forest)
            },
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let bears = driver.id_of("Grizzly Bears")?;
    let permanent = driver.state().object(bears).unwrap();
    assert_eq!(permanent.controller(), pid(active));
    Ok(permanent.permanent().unwrap().tapped())
}

#[rstest]
fn creatures_of_the_opponents_enter_tapped_and_those_of_the_holder_do_not(
    #[values(
        "Authority of the Consuls",
        "Kinjalli's Sunwing",
        "Urabrask the Hidden"
    )]
    holder: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert!(bears_enter_tapped(holder, seats, seats - 1)?);
    assert!(!bears_enter_tapped(holder, seats, 0)?);
    Ok(())
}

#[test]
fn a_creature_enters_untapped_without_the_group() -> Result<(), ScriptError> {
    assert!(!bears_enter_tapped("Llanowar Elves", 2, 1)?);
    Ok(())
}

#[test]
fn a_group_that_names_creatures_leaves_the_lands_of_the_opponents_untapped()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .at(1, 1, Step::Main1)
        .battlefield(0, copies("Authority of the Consuls", 1))
        .hand_named(1, ["Forest"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Forest")?;
    let forest = driver.id_of("Forest")?;
    assert!(
        !driver
            .state()
            .object(forest)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(names(driver.state(), Zone::Battlefield).len(), 2);
    Ok(())
}
