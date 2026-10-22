//! Goal: affinity plays as the rules say at two to four seats. A spell costs {1} less for each permanent of the kind its caster controls (tapped, summoning sick or an artifact creature alike, the opponents' permanents and other kinds never), never less than no generic mana, the colours of the cost stay, a tax or a reduction of another permanent applies beside it, and improvise stands beside affinity: the artifacts first make the cost less and then pay what is left.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::PermanentState;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{creature, island, plains, swamp};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::cost_cards::modifier;
use mtg_testkit::improvise_card;
use mtg_testkit::{Driver, Scenario, ready, sick, tapped};
use rstest::rstest;

fn land(name: &str) -> PresetPermanent {
    ready(match name {
        "Swamp" => swamp(),
        "Plains" => plains(),
        _ => island(),
    })
}

/// A permanent by spec: a card name, with `sick ` or `tapped ` before it; `Bear` is a green creature and no artifact.
fn permanent(spec: &str) -> PresetPermanent {
    let (state, name) = match spec.split_once(' ') {
        Some((state @ ("sick" | "tapped"), name)) => (state, name),
        _ => ("", spec),
    };
    let card = match name {
        "Bear" => creature("Bear", 1, &[Color::Green], 2, 2),
        other => improvise_card(other).unwrap(),
    };
    match state {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// Seat 0 holds `hand` with `lands` and `others` in play; seat 1 controls `theirs`.
fn table(seats: usize, hand: &str, lands: &[&str], others: &[&str], theirs: &[&str]) -> Scenario {
    let mut seat: Vec<PresetPermanent> = lands.iter().map(|name| land(name)).collect();
    seat.extend(others.iter().map(|spec| permanent(spec)));
    let hand = improvise_card(hand).or_else(|_| corpus_card(hand)).unwrap();
    Scenario::new(seats)
        .hand(0, [hand])
        .battlefield(0, seat)
        .battlefield(1, theirs.iter().map(|spec| permanent(spec)))
}

fn driver(scenario: Scenario) -> Driver {
    Driver::from_scenario(scenario).unwrap()
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, not {}", other.name()),
    }
}

/// The cost the payment asks for when `name` is cast.
fn announced(driver: &mut Driver, name: &str) -> String {
    driver.begin_cast(name).unwrap();
    driver.pay_options().unwrap().cost.to_string()
}

#[rstest]
#[case::no_artifacts_pay_the_whole_cost("Thoughtcast", &["Island"; 5], &[], &[], true)]
#[case::one_land_short("Thoughtcast", &["Island"; 4], &[], &[], false)]
#[case::each_artifact_takes_one_off("Thoughtcast", &["Island"], &["Trinket"; 4], &[], true)]
#[case::three_artifacts_are_short("Thoughtcast", &["Island"], &["Trinket"; 3], &[], false)]
#[case::the_colour_stays("Thoughtcast", &[], &["Trinket"; 4], &[], false)]
#[case::tapped_artifacts_count("Thoughtcast", &["Island"], &["tapped Trinket"; 4], &[], true)]
#[case::artifact_creatures_count("Thoughtcast", &["Island"], &["sick Clockwork"; 4], &[], true)]
#[case::other_permanents_do_not("Thoughtcast", &["Island"], &["Bear"; 4], &[], false)]
#[case::the_opponents_artifacts_do_not("Thoughtcast", &["Island"], &[], &["Trinket"; 4], false)]
#[case::more_artifacts_than_the_cost_has("Thoughtcast", &["Island"], &["Trinket"; 9], &[], true)]
#[case::a_creature_with_affinity_floors_at_zero("Affinity Golem", &[], &["Trinket"; 8], &[], true)]
#[case::six_artifacts_make_it_free("Affinity Golem", &[], &["Trinket"; 6], &[], true)]
#[case::five_artifacts_leave_one("Affinity Golem", &[], &["Trinket"; 5], &[], false)]
#[case::islands_count_for_affinity_for_islands("Island Sphinx", &["Island"; 3], &[], &[], true)]
#[case::two_islands_are_short("Island Sphinx", &["Island"; 2], &[], &[], false)]
#[case::other_lands_do_not_count("Island Sphinx", &["Island", "Swamp", "Swamp"], &[], &[], false)]
#[case::other_lands_still_pay("Island Sphinx", &["Island", "Swamp", "Swamp", "Swamp"], &[], &[], true)]
fn a_spell_with_affinity_costs_one_less_for_each_permanent_of_the_kind_you_control(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] lands: &[&str],
    #[case] others: &[&str],
    #[case] theirs: &[&str],
    #[case] expected: bool,
) {
    let driver = driver(table(seats, spell, lands, others, theirs));
    assert_eq!(castable(&driver), expected);
}

#[rstest]
#[case::none("Thoughtcast", &[], "{4}{U}")]
#[case::one("Thoughtcast", &["Trinket"], "{3}{U}")]
#[case::three("Thoughtcast", &["Trinket", "tapped Gadget", "sick Clockwork"], "{1}{U}")]
#[case::all_of_it("Thoughtcast", &["Trinket"; 4], "{U}")]
#[case::beyond_the_generic_part("Thoughtcast", &["Trinket"; 7], "{U}")]
#[case::a_creature_without_a_colour("Affinity Golem", &["Trinket"; 2], "{4}")]
fn the_cost_the_payment_asks_for_is_the_reduced_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] others: &[&str],
    #[case] expected: &str,
) {
    let mut driver = driver(table(seats, spell, &["Island"; 5], others, &[]));
    assert_eq!(announced(&mut driver, spell), expected);
}

#[rstest]
#[case::a_tax_adds_to_the_reduced_cost("Sphere", &["Island"; 5], true)]
#[case::one_land_short_under_the_tax("Sphere", &["Island"; 4], false)]
#[case::a_reduction_beside_it("My Helm", &["Island"; 3], true)]
#[case::two_lands_short_under_both("My Helm", &["Island"; 2], false)]
fn a_tax_or_a_reduction_of_another_permanent_applies_beside_affinity(
    #[values(2, 3, 4)] seats: usize,
    #[case] other: &str,
    #[case] lands: &[&str],
    #[case] expected: bool,
) {
    // The artifact that changes the cost is one of the artifacts the affinity counts.
    let mut seat: Vec<PresetPermanent> = lands.iter().map(|name| land(name)).collect();
    seat.push(ready(modifier(other).unwrap()));
    let scenario = table(seats, "Thoughtcast", &[], &[], &[]).battlefield(0, seat);
    assert_eq!(castable(&driver(scenario)), expected);
}

#[rstest]
#[case::affinity_alone_is_short("Improvise Affinity Golem", &["Swamp"], &["Trinket"; 2], false)]
#[case::affinity_and_improvise_together("Improvise Affinity Golem", &["Swamp"], &["Trinket"; 3], true)]
#[case::the_colour_still_needs_a_land("Improvise Affinity Golem", &[], &["Trinket"; 9], false)]
fn improvise_pays_what_affinity_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] lands: &[&str],
    #[case] others: &[&str],
    #[case] expected: bool,
) {
    let driver = driver(table(seats, spell, lands, others, &[]));
    assert_eq!(castable(&driver), expected);
}

#[rstest]
fn automatic_payment_after_affinity_taps_as_few_artifacts_as_it_can(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = driver(table(
        seats,
        "Improvise Affinity Golem",
        &["Swamp"],
        &["Trinket"; 3],
        &[],
    ));
    // {5}{B} less three for affinity is {2}{B}: the Swamp pays {B} and two artifacts the {2}.
    assert_eq!(announced(&mut driver, "Improvise Affinity Golem"), "{2}{B}");
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let state = driver.state();
    let tapped_trinkets = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == "Trinket")
        .filter(|o| o.permanent().is_some_and(PermanentState::tapped))
        .count();
    assert_eq!(tapped_trinkets, 2);
    assert_eq!(state.stack().len(), 1);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(driver.decision().unwrap().player, pid(0));
}
