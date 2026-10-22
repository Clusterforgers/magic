//! Goal: improvise plays as the rules say at two to four seats. A spell with the keyword is offered exactly when mana and artifacts can pay its modified cost (an artifact pays {1} of generic mana and no colour, summoning sick or not, a tapped artifact and a creature that is no artifact do not, a mana artifact taps for mana or for improvise and not both), the payment taps the artifacts it names before the mana, every wrong payment is refused with nothing changed (an artifact not offered, named twice, also tapped for mana or convoked, more than the generic cost asks), an automatic payment taps artifacts only when mana cannot pay and then as few as it can, cost reductions apply first, and improvise stands beside convoke and delve.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{IllegalAnswer, PaymentError};
use mtg_engine::help::Convoked;
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::object::PermanentState;
use mtg_engine::{EngineError, Event, PresetPermanent};
use mtg_testkit::cards::{creature, forest, island, swamp};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::cost_cards::modifier;
use mtg_testkit::improvise_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

fn land(name: &str) -> PresetPermanent {
    ready(match name {
        "Swamp" => swamp(),
        "Forest" => forest(),
        _ => island(),
    })
}

/// A permanent by spec: a card name, with `sick ` or `tapped ` before it; `Bear` is a green creature and no artifact.
fn permanent(spec: &str) -> PresetPermanent {
    let (state, name) = spec.split_once(' ').unwrap_or(("", spec));
    let (state, name) = match state {
        "sick" | "tapped" => (state, name),
        _ => ("", spec),
    };
    let card = match name {
        "Bear" => creature("Bear", 1, &[Color::Green], 2, 2),
        "Sol Ring" => corpus_card("Sol Ring").unwrap(),
        other => improvise_card(other).unwrap(),
    };
    match state {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// Seat 0 holds `hand` with `lands` and `others` in play, and the other seats just exist.
fn table(seats: usize, hand: &str, lands: &[&str], others: &[&str]) -> Scenario {
    let mut seat: Vec<PresetPermanent> = lands.iter().map(|name| land(name)).collect();
    seat.extend(others.iter().map(|spec| permanent(spec)));
    Scenario::new(seats)
        .hand(0, [improvise_card(hand).unwrap()])
        .battlefield(0, seat)
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

/// How many permanents of seat 0 are tapped, counting only those called `name` when it is given.
fn tapped_count(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .filter(|object| object.permanent().is_some_and(PermanentState::tapped))
        .count()
}

/// The ids of the permanents of seat 0 called `name`, in play order.
fn ids_of(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .collect()
}

/// Starts casting `name` and stops at the payment.
fn begin(driver: &mut Driver, name: &str) {
    driver.begin_cast(name).unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        let target = driver.target_player(1);
        driver.choose_targets(&[target]).unwrap();
    }
}

/// Answers a payment that must be refused, and checks that nothing changed and the payment is asked again.
fn refused(driver: &mut Driver, payment: Payment) -> PaymentError {
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(payment)));
    let Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Payment(error)))) = result
    else {
        panic!("a payment error was expected, not {result:?}");
    };
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
    driver.expect_decision(0, "pay-mana").unwrap();
    error
}

// Offered when mana and artifacts can pay

#[rstest]
#[case::three_artifacts_pay_the_generic_part("Reverse Engineer", &["Island", "Island"], &["Trinket"; 3], true)]
#[case::two_artifacts_are_short("Reverse Engineer", &["Island", "Island"], &["Trinket"; 2], false)]
#[case::artifacts_cannot_pay_blue("Reverse Engineer", &["Island"], &["Trinket"; 4], false)]
#[case::lands_alone_pay("Reverse Engineer", &["Island"; 5], &[], true)]
#[case::a_tapped_artifact_does_not_pay("Reverse Engineer", &["Island", "Island"], &["tapped Trinket", "Trinket", "Trinket"], false)]
#[case::a_summoning_sick_artifact_pays("Reverse Engineer", &["Island", "Island"], &["sick Clockwork", "sick Trinket", "Trinket"], true)]
#[case::a_creature_that_is_no_artifact_does_not("Reverse Engineer", &["Island", "Island"], &["Bear"; 3], false)]
#[case::more_artifacts_than_the_cost("Reverse Engineer", &["Island", "Island"], &["Trinket"; 6], true)]
#[case::a_mana_rock_and_an_artifact("Reverse Engineer", &["Island", "Island"], &["Sol Ring", "Trinket"], true)]
#[case::a_mana_rock_taps_for_mana_or_improvises("Reverse Engineer", &["Island", "Island"], &["Sol Ring"], false)]
#[case::a_spell_without_improvise_ignores_artifacts("Plain Bear", &[], &["Trinket"; 4], false)]
fn a_spell_with_improvise_is_offered_when_mana_and_artifacts_can_pay(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] lands: &[&str],
    #[case] others: &[&str],
    #[case] expected: bool,
) {
    let scenario = if spell == "Plain Bear" {
        Scenario::new(seats)
            .hand(0, [creature("Plain Bear", 2, &[Color::Green], 2, 2)])
            .battlefield(0, others.iter().map(|spec| permanent(spec)))
    } else {
        table(seats, spell, lands, others)
    };
    assert_eq!(castable(&driver(scenario)), expected);
}

#[rstest]
// The Helm is an artifact too: it improvises beside the trinkets.
#[case::two_artifacts_pay_the_reduced_generic_part("My Helm", &["Island", "Island"], &["Trinket"], true)]
#[case::one_artifact_is_short_under_the_reduction("My Helm", &["Island", "Island"], &[], false)]
#[case::a_tax_adds_to_the_generic_part("Sphere", &["Island", "Island"], &["Trinket"; 2], false)]
#[case::the_artifacts_pay_the_taxed_cost("Sphere", &["Island", "Island"], &["Trinket"; 3], true)]
fn cost_changes_apply_before_the_artifacts_pay(
    #[values(2, 3, 4)] seats: usize,
    #[case] modifier_name: &str,
    #[case] lands: &[&str],
    #[case] others: &[&str],
    #[case] expected: bool,
) {
    let mut seat: Vec<PresetPermanent> = lands.iter().map(|name| land(name)).collect();
    seat.extend(others.iter().map(|spec| permanent(spec)));
    seat.push(ready(modifier(modifier_name).unwrap()));
    let scenario = table(seats, "Reverse Engineer", &[], &[]).battlefield(0, seat);
    assert_eq!(castable(&driver(scenario)), expected);
}

// The payment

#[rstest]
fn the_payment_taps_the_artifacts_named_before_the_mana(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Trinket", "Gadget", "Gizmo"],
    ));
    begin(&mut driver, "Reverse Engineer");
    assert_eq!(driver.pay_options().unwrap().help.improvise.len(), 3);
    let events = driver
        .pay_helped_by(
            &["Island", "Island"],
            &[],
            &["Trinket", "Gadget", "Gizmo"],
            &[],
            0,
        )
        .unwrap();
    let artifacts: Vec<ObjectId> = ["Trinket", "Gadget", "Gizmo"]
        .iter()
        .flat_map(|name| ids_of(&driver, name))
        .collect();
    let at_tap = |object: &ObjectId| {
        events
            .iter()
            .position(|e| matches!(e, Event::PermanentTapped { object: o } if o == object))
            .unwrap()
    };
    let first_mana = events
        .iter()
        .position(|e| matches!(e, Event::ManaAdded { .. }))
        .unwrap();
    for artifact in &artifacts {
        assert!(
            at_tap(artifact) < first_mana,
            "the artifact taps before the mana"
        );
    }
    assert_eq!(tapped_count(&driver, "Island"), 2);
    assert_eq!(
        artifacts
            .iter()
            .filter(|id| driver
                .state()
                .object(**id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped())
            .count(),
        3
    );
    assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
}

#[rstest]
fn an_artifact_that_taps_for_mana_improvises_for_one_instead(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Sol Ring", "Trinket", "Gadget"],
    ));
    begin(&mut driver, "Reverse Engineer");
    driver
        .pay_helped_by(
            &["Island", "Island"],
            &[],
            &["Sol Ring", "Trinket", "Gadget"],
            &[],
            0,
        )
        .unwrap();
    assert_eq!(tapped_count(&driver, "Sol Ring"), 1);
    assert_eq!(driver.state().stack().len(), 1);
}

// Wrong payments

#[rstest]
fn an_artifact_that_is_not_offered_is_refused(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Trinket", "Trinket", "Trinket", "Bear"],
    ));
    begin(&mut driver, "Reverse Engineer");
    let bear = ids_of(&driver, "Bear")[0];
    let trinkets = ids_of(&driver, "Trinket");
    let mut payment = Payment {
        improvise: vec![trinkets[0], trinkets[1], bear],
        ..Payment::default()
    };
    payment.taps = driver
        .pay_options()
        .unwrap()
        .sources
        .iter()
        .map(|o| o.source)
        .collect();
    assert_eq!(
        refused(&mut driver, payment),
        PaymentError::NotOffered(bear)
    );
}

#[rstest]
fn an_artifact_named_twice_is_refused(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Trinket"; 3],
    ));
    begin(&mut driver, "Reverse Engineer");
    let trinkets = ids_of(&driver, "Trinket");
    let payment = Payment {
        improvise: vec![trinkets[0], trinkets[0], trinkets[1]],
        taps: driver
            .pay_options()
            .unwrap()
            .sources
            .iter()
            .map(|o| o.source)
            .collect(),
        ..Payment::default()
    };
    assert_eq!(
        refused(&mut driver, payment),
        PaymentError::HelpedTwice(trinkets[0])
    );
}

#[rstest]
fn more_artifacts_than_the_generic_cost_asks_are_refused(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Trinket"; 4],
    ));
    begin(&mut driver, "Reverse Engineer");
    let payment = Payment {
        improvise: ids_of(&driver, "Trinket"),
        taps: driver
            .pay_options()
            .unwrap()
            .sources
            .iter()
            .map(|o| o.source)
            .collect(),
        ..Payment::default()
    };
    assert_eq!(refused(&mut driver, payment), PaymentError::HelpedTooMuch);
}

#[rstest]
fn an_artifact_also_tapped_for_mana_is_refused(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(table(
        seats,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Sol Ring", "Trinket"],
    ));
    begin(&mut driver, "Reverse Engineer");
    let ring = ids_of(&driver, "Sol Ring")[0];
    let mut taps: Vec<ManaSource> = driver
        .pay_options()
        .unwrap()
        .sources
        .iter()
        .map(|o| o.source)
        .collect();
    taps.push(ManaSource {
        object: ring,
        ability: 0,
    });
    let payment = Payment {
        improvise: vec![ring, ids_of(&driver, "Trinket")[0]],
        taps,
        ..Payment::default()
    };
    assert_eq!(
        refused(&mut driver, payment),
        PaymentError::ObjectTappedTwice(ring)
    );
}

#[rstest]
fn an_artifact_creature_taps_for_convoke_or_improvise_and_not_both(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = driver(table(
        seats,
        "Improvise Convoke Bear",
        &["Island"],
        &["Clockwork", "Trinket"],
    ));
    begin(&mut driver, "Improvise Convoke Bear");
    let clockwork = ids_of(&driver, "Clockwork")[0];
    let options = driver.pay_options().unwrap();
    assert!(options.help.improvise.contains(&clockwork));
    assert!(options.help.convoke.iter().any(|o| o.creature == clockwork));
    let payment = Payment {
        convoke: vec![Convoked {
            creature: clockwork,
            pays: None,
        }],
        improvise: vec![clockwork],
        taps: options.sources.iter().map(|o| o.source).collect(),
        ..Payment::default()
    };
    assert_eq!(
        refused(&mut driver, payment),
        PaymentError::HelpedTwice(clockwork)
    );
}

// Automatic payment

#[rstest]
#[case::mana_pays_when_it_can(&["Island"; 5], 3, 0)]
#[case::artifacts_pay_what_mana_cannot(&["Island"; 3], 3, 2)]
#[case::as_few_as_it_can(&["Island", "Island"], 5, 3)]
#[case::all_the_cost_asks(&["Island", "Island"], 3, 3)]
fn automatic_payment_taps_artifacts_only_when_mana_cannot_pay(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] artifacts: usize,
    #[case] tapped_after: usize,
) {
    let others: Vec<&str> = vec!["Trinket"; artifacts];
    let mut driver = driver(table(seats, "Reverse Engineer", lands, &others));
    begin(&mut driver, "Reverse Engineer");
    assert!(driver.pay_options().unwrap().auto_available);
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_eq!(tapped_count(&driver, "Trinket"), tapped_after);
    assert_eq!(driver.state().stack().len(), 1);
}

// Beside convoke and delve

#[rstest]
#[case::artifacts_and_a_creature("Improvise Convoke Bear", &["Island"], &["Trinket", "Bear"], 0, true)]
#[case::a_creature_alone_is_short("Improvise Convoke Bear", &["Island"], &["Bear"], 0, false)]
#[case::artifacts_and_a_card("Improvise Delve Bear", &["Island"], &["Trinket"], 1, true)]
#[case::a_card_alone_is_short("Improvise Delve Bear", &["Island"], &[], 1, false)]
#[case::a_creature_does_not_improvise("Improvise Bear", &["Island"], &["Bear", "Bear"], 0, false)]
#[case::artifacts_improvise("Improvise Bear", &["Island"], &["Trinket", "Gadget"], 0, true)]
fn improvise_stands_beside_convoke_and_delve(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] lands: &[&str],
    #[case] others: &[&str],
    #[case] graveyard: usize,
    #[case] expected: bool,
) {
    let dead = (0..graveyard)
        .map(|n| creature(&format!("Dead {n}"), 0, &[], 1, 1))
        .collect::<Vec<_>>();
    let scenario = table(seats, spell, lands, others).graveyard(0, dead);
    assert_eq!(castable(&driver(scenario)), expected);
}

#[test]
fn a_game_with_artifacts_tapped_for_improvise_replays_to_the_same_state() {
    let mut driver = driver(table(
        2,
        "Reverse Engineer",
        &["Island", "Island"],
        &["Trinket"; 3],
    ));
    begin(&mut driver, "Reverse Engineer");
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(pid(0), driver.decision().unwrap().player);
}
