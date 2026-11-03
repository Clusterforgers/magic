//! Goal: a mana ability of any shape (a mana cost without {T}, an X that picks permanents) is a mana ability: it pays for a spell only inside a payment, the castable list and the payment agree, and a cast that the list offers always completes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// An artifact with one ability written in the JSON of the card files.
fn source(rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Source")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Builds the table, checks the list against `affordable` and, when offered, that the cast completes.
fn check(
    seats: usize,
    cost: &str,
    mine: Vec<Arc<CardDefinition>>,
    affordable: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [spell(cost)])
        .battlefield(0, mine.into_iter().map(ready));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| mountain()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(castable(&driver), affordable);
    if affordable {
        driver.cast("Spell")?;
        assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 0);
    }
    Ok(())
}

const REFRACTOR: &str =
    r#"{"Activated":{"cost":"{2}","effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
const PAIR: &str =
    r#"{"Activated":{"cost":"{1}","effects":[{"AddMana":{"mana":{"Fixed":["Red","Red"]}}}]}}"#;

#[rstest]
#[case::two_lands_fix_a_colour(REFRACTOR, "{R}", 2, true)]
#[case::one_land_is_short(REFRACTOR, "{R}", 1, false)]
#[case::the_source_pays_for_nothing_alone(REFRACTOR, "{R}", 0, false)]
#[case::a_land_beside_the_fixing(REFRACTOR, "{R}{1}", 3, true)]
#[case::the_mana_it_adds_never_pays_its_own_cost(REFRACTOR, "{R}{R}", 2, false)]
#[case::a_cost_of_one_makes_two(PAIR, "{R}{R}", 1, true)]
#[case::a_cost_of_one_and_two_lands(PAIR, "{R}{R}{1}", 2, true)]
#[case::the_other_land_is_green(PAIR, "{R}{R}{R}", 2, false)]
#[case::a_pair_is_no_more_than_that(PAIR, "{R}{R}{R}{R}", 2, false)]
fn a_tapless_source_with_a_mana_cost_is_a_source_of_the_payment(
    #[case] rules: &str,
    #[case] cost: &str,
    #[case] lands: usize,
    #[case] affordable: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![source(rules)];
    mine.extend((0..lands).map(|_| forest()));
    check(seats, cost, mine, affordable)
}

const TAP_X: &str = r#"{"Activated":{"cost":[{"TapPermanents":{"count":"X","filter":[{"Type":"Creature"}]}}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":"X"}}}}]}}"#;
const SACRIFICE_X: &str = r#"{"Activated":{"cost":[{"Sacrifice":{"count":"X","filter":[{"Type":"Creature"}]}}],"effects":[{"AddMana":{"mana":{"AnyColor":"X"}}}]}}"#;
const TAP_X_AND_LIFE: &str = r#"{"Activated":{"cost":["Tap",{"PayLife":2},{"TapPermanents":{"count":"X","filter":[{"Type":"Creature"}]}}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":"X"}}}}]}}"#;

#[rstest]
#[case::tap_two_for_two(TAP_X, "{R}{G}", 2, true)]
#[case::tap_three_for_three(TAP_X, "{R}{G}{W}", 3, true)]
#[case::one_creature_is_one_mana(TAP_X, "{R}{G}", 1, false)]
#[case::no_creature_no_mana(TAP_X, "{R}", 0, false)]
#[case::sacrifice_two_for_two_of_a_colour(SACRIFICE_X, "{R}{R}", 2, true)]
#[case::sacrifice_cannot_make_two_colours(SACRIFICE_X, "{R}{G}", 2, false)]
#[case::sacrifice_one_for_one(SACRIFICE_X, "{R}", 1, true)]
#[case::the_source_taps_too(TAP_X_AND_LIFE, "{R}{G}", 2, true)]
fn a_source_that_takes_x_permanents_adds_x_mana(
    #[case] rules: &str,
    #[case] cost: &str,
    #[case] creatures: usize,
    #[case] affordable: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![source(rules)];
    mine.extend((0..creatures).map(|_| free_creature("Bear", 1, 1)));
    check(seats, cost, mine, affordable)
}
