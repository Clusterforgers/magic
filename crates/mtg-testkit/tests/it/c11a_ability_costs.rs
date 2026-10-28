//! Goal: an activated ability that changes its own cost ("This ability costs {1} less to activate for each ...", "{1} more ... for each card in your hand") asks for the cost its count gives as it is activated: the reduction takes generic mana down to nothing and never a coloured symbol, counts only what its words name (the activator's graveyard, the other artifacts they control), an increase follows the hand, and the ability is offered exactly when the changed cost can be paid.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use mtg_core::mana::ManaCost;
use mtg_testkit::cards::{island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The total cost at the payment of activating the ability of `name` from the priority of seat 0.
fn activation_cost(scenario: Scenario, name: &str) -> Result<String, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation(name)?;
    Ok(driver.pay_options()?.cost.to_string())
}

/// The cost as the engine writes it.
fn cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// Seat 0 controls Pteramander and `islands` Islands, with `mine` instants and `lands_in` artifacts in their graveyard;
/// seat 1 has `theirs` instants in theirs.
fn pteramander(islands: usize, mine: usize, lands_in: usize, theirs: usize) -> Scenario {
    let mut permanents = lands(islands, island);
    permanents.push(ready(card("Pteramander").unwrap()));
    let mut graveyard = vec!["Lightning Bolt"; mine];
    graveyard.extend(vec!["Sol Ring"; lands_in]);
    Scenario::new(2)
        .battlefield(0, permanents)
        .graveyard_named(0, graveyard)
        .graveyard_named(1, vec!["Opt"; theirs])
}

#[rstest]
#[case::nothing_counted(0, 0, 0, "{7}{U}")]
#[case::each_instant_takes_one(3, 0, 0, "{4}{U}")]
#[case::other_cards_do_not_count(2, 4, 0, "{5}{U}")]
#[case::the_opponents_graveyard_does_not_count(1, 0, 5, "{6}{U}")]
#[case::down_to_the_colored_symbol(7, 0, 0, "{U}")]
#[case::never_below_it(10, 0, 0, "{U}")]
fn a_reduction_for_each_card_in_your_graveyard_follows_the_count(
    #[case] mine: usize,
    #[case] lands_in: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let scenario = pteramander(8, mine, lands_in, theirs);
    assert_eq!(activation_cost(scenario, "Pteramander")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::too_few_lands_and_nothing_counted(2, 0, false)]
#[case::enough_counted_for_two_lands(2, 6, true)]
#[case::one_land_pays_the_colored_symbol(1, 7, true)]
#[case::one_short(1, 6, false)]
fn the_ability_is_offered_when_the_reduced_cost_can_be_paid(
    #[case] islands: usize,
    #[case] mine: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(pteramander(islands, mine, 0, 0))?;
    assert_eq!(
        !driver.offered_abilities("Pteramander")?.is_empty(),
        offered
    );
    Ok(())
}

#[rstest]
#[case::an_empty_hand(0, "{3}")]
#[case::one_card(1, "{4}")]
#[case::four_cards(4, "{7}")]
fn an_increase_for_each_card_in_your_hand_follows_the_hand(
    #[case] hand: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut permanents = lands(10, plains);
    permanents.push(ready(card("Loreseeker's Stone").unwrap()));
    let scenario = Scenario::new(2)
        .battlefield(0, permanents)
        .hand_named(0, vec!["Lightning Bolt"; hand]);
    assert_eq!(
        activation_cost(scenario, "Loreseeker's Stone")?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::alone(0, 0, "{5}{U}")]
#[case::other_artifacts(2, 0, "{3}{U}")]
#[case::the_opponents_artifacts_do_not_count(1, 3, "{4}{U}")]
#[case::all_the_generic_mana(6, 0, "{U}")]
fn a_reduction_for_each_other_artifact_counts_the_activators_others(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut permanents = lands(6, island);
    permanents.push(ready(card("Tamiyo's Logbook").unwrap()));
    permanents.extend((0..mine).map(|_| ready(card("Arcane Signet").unwrap())));
    let scenario = Scenario::new(2).battlefield(0, permanents).battlefield(
        1,
        (0..theirs)
            .map(|_| ready(card("Arcane Signet").unwrap()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        activation_cost(scenario, "Tamiyo's Logbook")?,
        cost(expected)
    );
    Ok(())
}
