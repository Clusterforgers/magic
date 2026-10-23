//! Goal: a static ability of a permanent that gives improvise or convoke to the spells it names (Inspiring Statuary: nonartifact spells you cast; Chief Engineer: artifact spells you cast) plays as the rules say at two to four seats. The keyword is on the spells of the permanent's controller that the selector names and on no other (not the opponent's spells, not the artifact spells of Inspiring Statuary), the helpers it offers are exactly those of the printed keyword (untapped artifacts or creatures, the granting permanent among them), a spell with the keyword of its own does not have it twice, the payment taps what it names, a helper the spell does not offer is refused, and the keyword is gone with the permanent that gave it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{IllegalAnswer, PaymentError};
use mtg_engine::mana::Payment;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::object::PermanentState;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, PresetPermanent};
use mtg_testkit::cards::{creature, island};
use mtg_testkit::improvise_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

/// A permanent by spec: a card name, with `sick ` or `tapped ` before it; `Bear` is a green creature and no artifact.
fn permanent(spec: &str) -> PresetPermanent {
    let (state, name) = match spec.split_once(' ') {
        Some((state @ ("sick" | "tapped"), name)) => (state, name),
        _ => ("", spec),
    };
    let card = match name {
        "Island" => island(),
        "Bear" => creature("Bear", 1, &[Color::Green], 2, 2),
        other => improvise_card(other).unwrap(),
    };
    match state {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// Seat 0 holds `spell` with `mine` in play, seat 1 controls `theirs`, and any other seat just exists.
fn table(seats: usize, spell: &str, mine: &[&str], theirs: &[&str]) -> Scenario {
    Scenario::new(seats)
        .hand(0, [improvise_card(spell).unwrap()])
        .battlefield(0, mine.iter().map(|spec| permanent(spec)))
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

/// How many permanents called `name` are tapped.
fn tapped_count(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .filter(|object| object.permanent().is_some_and(PermanentState::tapped))
        .count()
}

const STATUARY: &str = "Inspiring Statuary";
const ENGINEER: &str = "Chief Engineer";

// Improvise given to the nonartifact spells of its controller

#[rstest]
// Inspiring Statuary is an artifact too: it pays {1} beside the trinkets.
#[case::the_statuary_and_two_artifacts_pay_the_generic_part("Statuary Bear", &["Island", STATUARY, "Trinket", "Trinket"], &[], true)]
#[case::the_statuary_and_one_artifact_are_short("Statuary Bear", &["Island", STATUARY, "Trinket"], &[], false)]
#[case::the_statuary_alone_is_short("Statuary Bear", &["Island", STATUARY], &[], false)]
#[case::many_artifacts_pay_no_more_than_the_cost("Statuary Bear", &["Island", STATUARY, "Trinket", "Trinket", "Trinket", "Trinket"], &[], true)]
#[case::a_summoning_sick_artifact_pays("Statuary Bear", &["Island", STATUARY, "sick Clockwork", "sick Trinket"], &[], true)]
#[case::tapped_artifacts_do_not_pay("Statuary Bear", &["Island", STATUARY, "tapped Trinket", "tapped Trinket"], &[], false)]
#[case::a_creature_that_is_no_artifact_does_not_pay("Statuary Bear", &["Island", STATUARY, "Bear", "Bear"], &[], false)]
#[case::the_colour_still_needs_a_land("Statuary Bear", &[STATUARY, "Trinket", "Trinket", "Trinket", "Trinket"], &[], false)]
#[case::lands_alone_pay_as_before("Statuary Bear", &["Island"; 4], &[], true)]
#[case::without_the_statuary_artifacts_do_not_pay("Statuary Bear", &["Island", "Trinket", "Trinket", "Trinket"], &[], false)]
#[case::the_statuary_of_an_opponent_gives_nothing("Statuary Bear", &["Island", "Trinket", "Trinket", "Trinket"], &[STATUARY], false)]
#[case::the_statuary_of_an_opponent_takes_nothing_away("Statuary Bear", &["Island"; 4], &[STATUARY], true)]
#[case::an_artifact_spell_is_left_out("Pricey Relic", &[STATUARY, "Trinket", "Trinket", "Trinket"], &[], false)]
#[case::an_artifact_spell_is_paid_by_lands("Pricey Relic", &[STATUARY, "Island", "Island", "Island", "Island"], &[], true)]
#[case::a_spell_with_improvise_of_its_own_has_it_once("Improvise Bear", &["Island", STATUARY, "Trinket"], &[], true)]
#[case::and_gains_no_second_payment("Improvise Bear", &["Island", STATUARY], &[], false)]
fn the_spells_a_static_ability_names_have_improvise(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] expected: bool,
) {
    assert_eq!(
        castable(&driver(table(seats, spell, mine, theirs))),
        expected
    );
}

// Convoke given to the artifact spells of its controller

#[rstest]
// Chief Engineer is a creature: it convokes beside the others.
#[case::the_engineer_and_three_creatures_pay("Pricey Relic", &[ENGINEER, "Bear", "Bear", "Bear"], &[], true)]
#[case::the_engineer_and_two_creatures_are_short("Pricey Relic", &[ENGINEER, "Bear", "Bear"], &[], false)]
#[case::summoning_sick_creatures_pay("Pricey Relic", &[ENGINEER, "sick Bear", "sick Bear", "sick Bear"], &[], true)]
#[case::tapped_creatures_do_not_pay("Pricey Relic", &[ENGINEER, "tapped Bear", "tapped Bear", "tapped Bear"], &[], false)]
#[case::a_nonartifact_spell_is_left_out("Statuary Bear", &[ENGINEER, "Bear", "Bear", "Bear"], &[], false)]
#[case::the_engineer_of_an_opponent_gives_nothing("Pricey Relic", &["Bear", "Bear", "Bear", "Bear"], &[ENGINEER], false)]
#[case::both_abilities_apply_beside_each_other("Pricey Relic", &[ENGINEER, STATUARY, "Bear"], &[], false)]
fn the_artifact_spells_of_the_engineer_have_convoke(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] expected: bool,
) {
    assert_eq!(
        castable(&driver(table(seats, spell, mine, theirs))),
        expected
    );
}

// The payment

#[rstest]
fn the_payment_taps_the_artifacts_the_given_keyword_offers(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Statuary Bear",
        &["Island", STATUARY, "Trinket", "Gadget"],
        &[],
    ));
    driver.begin_cast("Statuary Bear").unwrap();
    assert_eq!(driver.pay_options().unwrap().help.improvise.len(), 3);
    driver
        .pay_helped_by(&["Island"], &[], &[STATUARY, "Trinket", "Gadget"], &[], 0)
        .unwrap();
    assert_eq!(tapped_count(&driver, "Island"), 1);
    assert_eq!(tapped_count(&driver, STATUARY), 1);
    assert_eq!(tapped_count(&driver, "Trinket"), 1);
    assert_eq!(tapped_count(&driver, "Gadget"), 1);
    assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
    assert_eq!(driver.decision().unwrap().player, pid(0));
}

#[rstest]
fn an_automatic_payment_taps_as_few_artifacts_as_it_can(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Statuary Bear",
        &["Island", "Island", STATUARY, "Trinket", "Trinket"],
        &[],
    ));
    driver.begin_cast("Statuary Bear").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    // {3}{U}: the Islands pay two of the four symbols and two artifacts pay the other two.
    let artifacts = tapped_count(&driver, STATUARY) + tapped_count(&driver, "Trinket");
    assert_eq!(tapped_count(&driver, "Island"), 2);
    assert_eq!(artifacts, 2);
    assert_eq!(driver.state().stack().len(), 1);
}

#[rstest]
fn the_spell_that_is_left_out_offers_no_helper(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Pricey Relic",
        &[STATUARY, "Island", "Island", "Island", "Island", "Trinket"],
        &[],
    ));
    driver.begin_cast("Pricey Relic").unwrap();
    assert!(driver.pay_options().unwrap().help.improvise.is_empty());
    assert!(driver.pay_options().unwrap().help.convoke.is_empty());
    let trinket = driver.id_of("Trinket").unwrap();
    let payment = Payment {
        improvise: vec![trinket],
        ..Payment::default()
    };
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(payment)));
    let Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Payment(error)))) = result
    else {
        panic!("a payment error was expected, not {result:?}");
    };
    assert_eq!(error, PaymentError::NotOffered(trinket));
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
}

// The permanent that gives the keyword leaves

#[rstest]
#[case::to_the_graveyard(ManualZone::Graveyard)]
#[case::to_exile(ManualZone::Exile)]
#[case::to_the_hand(ManualZone::Hand)]
fn the_keyword_is_gone_when_the_permanent_that_gave_it_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[case] to: ManualZone,
) {
    let mut driver = driver(table(
        seats,
        "Statuary Bear",
        &["Island", STATUARY, "Trinket", "Trinket"],
        &[],
    ));
    assert!(castable(&driver), "castable while it is in play");
    let statuary = driver.id_of(STATUARY).unwrap();
    driver
        .manual(ManualAction::Move {
            object: statuary,
            to,
        })
        .unwrap();
    assert!(
        !castable(&driver),
        "two artifacts and a land do not pay {{3}}{{U}} without it"
    );
}
