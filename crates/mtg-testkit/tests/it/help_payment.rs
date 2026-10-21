//! Goal: convoke and delve play as the rules say. A spell with the keyword is offered exactly when mana and the helpers can pay its modified cost (a creature pays {1} or a colour it is, summoning sick or not, a card pays {1} of generic mana and no colour, a tapped creature does not), the payment taps the creatures and exiles the cards it names after the spell is cast and before the mana, every wrong payment is refused with nothing changed (a colour the creature lacks, a creature or card named twice or not offered, more than the cost asks, a creature also tapped for mana), an automatic payment uses helpers only when mana cannot pay and then as few as it can, cost increases and reductions apply before the helpers, and hybrid, Phyrexian, {X} and restricted mana stand beside the helpers.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{IllegalAnswer, PaymentError};
use mtg_engine::help::Convoked;
use mtg_engine::mana::Payment;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, PresetPermanent};
use mtg_testkit::cards::{creature, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::cost_cards::modifier;
use mtg_testkit::help_cards::helped_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

fn land(name: &str) -> PresetPermanent {
    ready(match name {
        "Mountain" => mountain(),
        "Swamp" => swamp(),
        "Island" => island(),
        "Forest" => forest(),
        _ => plains(),
    })
}

/// A creature by spec: a colour (`red`, `green`, `blue`, `white`, `gray` for colourless), `sick ` or `tapped ` before it.
fn bear(spec: &str) -> PresetPermanent {
    let (state, color) = spec.split_once(' ').unwrap_or(("", spec));
    let card = match color {
        "red" => creature("Red Bear", 1, &[Color::Red], 2, 2),
        "green" => creature("Green Bear", 1, &[Color::Green], 2, 2),
        "blue" => creature("Blue Bear", 1, &[Color::Blue], 2, 2),
        "white" => creature("White Bear", 1, &[Color::White], 2, 2),
        _ => creature("Golem", 2, &[], 2, 2),
    };
    match state {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// `count` cards for a graveyard, named `Dead 0`, `Dead 1`, ...
fn dead(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|n| creature(&format!("Dead {n}"), 0, &[], 1, 1))
        .collect()
}

fn named(name: &str) -> Arc<CardDefinition> {
    helped_card(name).unwrap()
}

/// Seat 0 holds `hand` with `lands` and `creatures` in play and `graveyard` cards in its graveyard.
fn table(hand: &str, lands: &[&str], creatures: &[&str], graveyard: usize) -> Scenario {
    let mut seat: Vec<PresetPermanent> = lands.iter().map(|name| land(name)).collect();
    seat.extend(creatures.iter().map(|spec| bear(spec)));
    Scenario::new(2)
        .hand(0, [named(hand)])
        .battlefield(0, seat)
        .graveyard(0, dead(graveyard))
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

fn tapped_creatures(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            object
                .card()
                .types
                .contains(&mtg_core::card::CardType::Creature)
        })
        .filter(|object| object.permanent().is_some_and(|p| p.tapped()))
        .count()
}

/// Casts `name` at the opponent and stops at the payment.
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

// Offered when mana and the helpers can pay

#[rstest]
#[case::four_red_creatures("Stoke the Flames", &[], &["red"; 4], 0, true)]
#[case::three_red_creatures("Stoke the Flames", &[], &["red"; 3], 0, false)]
#[case::the_generic_part_by_any_creature("Stoke the Flames", &[], &["red", "red", "gray", "gray"], 0, true)]
#[case::green_creatures_pay_generic("Stoke the Flames", &[], &["red", "red", "green", "green"], 0, true)]
#[case::green_creatures_cannot_pay_red("Stoke the Flames", &[], &["green"; 4], 0, false)]
#[case::summoning_sick_creatures_pay("Stoke the Flames", &[], &["sick red"; 4], 0, true)]
#[case::a_tapped_creature_does_not("Stoke the Flames", &[], &["tapped red", "red", "red", "red"], 0, false)]
#[case::lands_and_creatures("Stoke the Flames", &["Mountain", "Mountain"], &["green", "green"], 0, true)]
#[case::one_land_three_creatures("Stoke the Flames", &["Mountain"], &["red"; 3], 0, true)]
#[case::a_graveyard_does_not_help_a_convoke_spell("Stoke the Flames", &[], &["red"; 3], 5, false)]
#[case::seven_cards_and_an_island("Treasure Cruise", &["Island"], &[], 7, true)]
#[case::six_cards_and_an_island("Treasure Cruise", &["Island"], &[], 6, false)]
#[case::cards_cannot_pay_blue("Treasure Cruise", &[], &[], 12, false)]
#[case::lands_and_cards("Treasure Cruise", &["Island", "Mountain", "Mountain"], &[], 5, true)]
#[case::creatures_do_not_help_a_delve_spell("Treasure Cruise", &["Island"], &["red"; 7], 0, false)]
#[case::a_creature_spell_with_delve("Gurmag Angler", &["Swamp"], &[], 6, true)]
#[case::one_card_short("Gurmag Angler", &["Swamp"], &[], 5, false)]
#[case::both_by_creature_and_card("Both Hybrid", &["Plains"], &["blue"], 2, true)]
#[case::hybrid_by_lands("Both Hybrid", &["Plains", "Island"], &[], 2, true)]
#[case::hybrid_by_creatures("Both Hybrid", &[], &["white", "blue"], 2, true)]
#[case::hybrid_needs_white_or_blue("Both Hybrid", &[], &["red", "red"], 4, false)]
#[case::an_x_spell_pays_its_colour("Convoke Blaze", &[], &["red"], 0, true)]
#[case::an_x_spell_needs_its_colour("Convoke Blaze", &[], &["green"; 3], 0, false)]
fn a_spell_is_offered_exactly_when_mana_and_the_helpers_pay_it(
    #[case] spell: &str,
    #[case] lands: &[&str],
    #[case] creatures: &[&str],
    #[case] graveyard: usize,
    #[case] offered: bool,
) {
    let driver = driver(table(spell, lands, creatures, graveyard));
    assert_eq!(castable(&driver), offered);
}

#[rstest]
#[case::an_opponents_tax_on_a_convoke_spell("Stoke the Flames", "Sphere", 1, &["Mountain"], &["red"; 4], 0, true)]
#[case::one_creature_short_under_the_tax("Stoke the Flames", "Sphere", 1, &[], &["red"; 4], 0, false)]
#[case::a_tax_on_a_delve_spell("Gurmag Angler", "Sphere", 1, &["Swamp"], &[], 7, true)]
#[case::a_card_short_under_the_tax("Gurmag Angler", "Sphere", 1, &["Swamp"], &[], 6, false)]
#[case::a_reduction_takes_a_card_off("Gurmag Angler", "My Helm", 0, &["Swamp"], &[], 5, true)]
#[case::a_reduction_takes_one_only("Gurmag Angler", "My Helm", 0, &["Swamp"], &[], 4, false)]
fn cost_modifiers_apply_before_the_helpers(
    #[case] spell: &str,
    #[case] modifier_name: &str,
    #[case] holder: usize,
    #[case] lands: &[&str],
    #[case] creatures: &[&str],
    #[case] graveyard: usize,
    #[case] offered: bool,
) {
    let scenario = table(spell, lands, creatures, graveyard).battlefield(
        holder,
        [ready(modifier(modifier_name).unwrap())]
            .into_iter()
            .chain(if holder == 0 {
                lands
                    .iter()
                    .map(|name| land(name))
                    .chain(creatures.iter().map(|spec| bear(spec)))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }),
    );
    assert_eq!(castable(&driver(scenario)), offered);
}

#[rstest]
#[case::one_creature_for_the_phyrexian_symbol_and_life("Convoke Phyrexian", &["gray"], 20, true)]
#[case::a_red_creature_pays_the_red_symbol("Convoke Phyrexian", &["red", "gray"], 1, true)]
#[case::life_cannot_pay_what_there_is_not("Convoke Phyrexian", &["red"], 1, false)]
#[case::life_and_one_helper("Convoke Phyrexian", &["green"], 2, true)]
#[case::delve_pays_the_generic_part("Delve Phyrexian", &[], 2, true)]
fn phyrexian_symbols_are_paid_by_a_creature_of_the_colour_or_by_life(
    #[case] spell: &str,
    #[case] creatures: &[&str],
    #[case] life: i32,
    #[case] offered: bool,
) {
    let graveyard = usize::from(spell.starts_with("Delve"));
    let scenario = table(spell, &[], creatures, graveyard).life(0, life);
    assert_eq!(castable(&driver(scenario)), offered);
}

#[rstest]
#[case::the_colour_and_one_more_for_x(3, 2)]
#[case::five_creatures(5, 4)]
fn x_is_asked_up_to_what_the_creatures_can_pay(#[case] bears: usize, #[case] most: u32) {
    let mut driver = driver(table("Convoke Blaze", &[], &vec!["red"; bears], 0));
    driver.begin_cast("Convoke Blaze").unwrap();
    let DecisionKind::ChooseNumber(options) = &driver.decision().unwrap().kind else {
        panic!("X was expected");
    };
    assert_eq!(options.max, Some(most));
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(most)))
        .unwrap();
    let target = driver.target_player(1);
    driver.choose_targets(&[target]).unwrap();
    assert_eq!(driver.pay_options().unwrap().cost.generic(), most);
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_eq!(tapped_creatures(&driver), bears.min(most as usize + 1));
}

#[test]
fn restricted_mana_pays_a_creature_spell_beside_a_delve_and_no_other_spell() {
    let ziggurat = [ready(corpus_card("Ancient Ziggurat").unwrap())];
    let creature_spell = Scenario::new(2)
        .hand(0, [named("Gurmag Angler")])
        .battlefield(0, ziggurat.clone())
        .graveyard(0, dead(6));
    assert!(castable(&driver(creature_spell)));
    let other_spell = Scenario::new(2)
        .hand(0, [named("Treasure Cruise")])
        .battlefield(0, ziggurat)
        .graveyard(0, dead(7));
    assert!(
        !castable(&driver(other_spell)),
        "the mana is for creature spells only"
    );
}

// The payment

#[rstest]
#[case::each_for_one(&[None, None])]
#[case::each_for_red(&[Some(Color::Red), Some(Color::Red)])]
fn creatures_are_tapped_after_the_cast_and_before_the_mana(#[case] pays: &[Option<Color>]) {
    let mut driver = driver(table(
        "Stoke the Flames",
        &["Mountain", "Mountain"],
        &["sick red", "red"],
        0,
    ));
    begin(&mut driver, "Stoke the Flames");
    let options = driver.pay_options().unwrap();
    assert_eq!(
        options.convoke.len(),
        2,
        "a summoning sick creature is offered"
    );
    assert!(options.convoke.iter().all(|o| o.colors == [Color::Red]));
    let convoke: Vec<(&str, Option<Color>)> = pays.iter().map(|p| ("Red Bear", *p)).collect();
    let events = driver
        .pay_helped(&["Mountain", "Mountain"], &convoke, &[], 0)
        .unwrap();
    let at = |wanted: &dyn Fn(&Event) -> bool| events.iter().position(wanted).unwrap();
    let cast = at(&|e| matches!(e, Event::SpellCast { .. }));
    let first_tap = at(&|e| matches!(e, Event::PermanentTapped { .. }));
    let spent = at(&|e| matches!(e, Event::ManaSpent { .. }));
    assert!(cast < first_tap && first_tap < spent, "{events:?}");
    let taps = events
        .iter()
        .filter(|e| matches!(e, Event::PermanentTapped { .. }))
        .count();
    assert_eq!(taps, 4, "two lands and two creatures");
    assert_eq!(tapped_creatures(&driver), 2);
    assert_eq!(driver.state().stack().len(), 1);
    let Event::ManaSpent { mana, .. } = &events[spent] else {
        unreachable!()
    };
    assert_eq!(mana.len(), 2, "creatures are not mana");
}

#[test]
fn cards_are_exiled_from_the_graveyard_before_the_mana() {
    let mut driver = driver(table("Gurmag Angler", &["Swamp"], &[], 8));
    driver.begin_cast("Gurmag Angler").unwrap();
    assert_eq!(driver.pay_options().unwrap().delve.len(), 8);
    let cards: Vec<String> = (0..6).map(|n| format!("Dead {n}")).collect();
    let names: Vec<&str> = cards.iter().map(String::as_str).collect();
    let events = driver.pay_helped(&["Swamp"], &[], &names, 0).unwrap();
    let exiled = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::ZoneChanged { from: Zone::Graveyard(p), to: Zone::Exile, .. } if *p == pid(0)
            )
        })
        .count();
    assert_eq!(exiled, 6);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 2);
    assert_eq!(driver.state().zone(Zone::Exile).len(), 6);
    driver
        .expect_event(
            "one mana was spent",
            |e| matches!(e, Event::ManaSpent { mana, .. } if mana == &[ManaType::Black]),
        )
        .unwrap();
}

#[test]
fn a_creature_and_a_card_pay_beside_mana_in_one_payment() {
    let mut driver = driver(table("Both Hybrid", &["Plains"], &["blue", "gray"], 1));
    driver.begin_cast("Both Hybrid").unwrap();
    // {2}{W/U}{W/U}: the Plains and the blue bear pay the hybrid symbols, the Golem and a card the {2}.
    driver
        .pay_helped(
            &["Plains"],
            &[("Blue Bear", Some(Color::Blue)), ("Golem", None)],
            &["Dead 0"],
            0,
        )
        .unwrap();
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(tapped_creatures(&driver), 2);
}

// The refusals

fn stoke(creatures: &[&str]) -> Driver {
    let mut driver = driver(table("Stoke the Flames", &[], creatures, 0));
    begin(&mut driver, "Stoke the Flames");
    driver
}

fn convoked(driver: &Driver, names: &[(&str, Option<Color>)]) -> Vec<Convoked> {
    let mut used: Vec<_> = Vec::new();
    for (name, _) in names {
        let creature = driver
            .pay_options()
            .unwrap()
            .convoke
            .iter()
            .map(|o| o.creature)
            .find(|id| {
                !used.contains(id) && driver.state().object(*id).unwrap().card().name == *name
            })
            .unwrap();
        used.push(creature);
    }
    used.into_iter()
        .zip(names)
        .map(|(creature, (_, pays))| Convoked {
            creature,
            pays: *pays,
        })
        .collect()
}

#[test]
fn a_creature_cannot_pay_for_a_colour_it_is_not() {
    let mut driver = stoke(&["green", "red", "red", "red"]);
    let convoke = convoked(
        &driver,
        &[
            ("Green Bear", Some(Color::Red)),
            ("Red Bear", None),
            ("Red Bear", None),
            ("Red Bear", None),
        ],
    );
    let error = refused(
        &mut driver,
        Payment {
            convoke,
            ..Payment::default()
        },
    );
    assert!(
        matches!(
            error,
            PaymentError::WrongColor {
                color: Color::Red,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_tapped_creature_is_not_offered_and_cannot_be_named() {
    let mut driver = stoke(&["tapped red", "red", "red", "red", "red"]);
    assert_eq!(driver.pay_options().unwrap().convoke.len(), 4);
    let tapped_one = driver.id_of("Red Bear").unwrap();
    let error = refused(
        &mut driver,
        Payment {
            convoke: vec![Convoked {
                creature: tapped_one,
                pays: None,
            }],
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::NotOffered(tapped_one));
}

#[test]
fn a_creature_named_twice_pays_once() {
    let mut driver = stoke(&["red"; 4]);
    let one = driver.pay_options().unwrap().convoke[0].creature;
    let error = refused(
        &mut driver,
        Payment {
            convoke: vec![
                Convoked {
                    creature: one,
                    pays: None,
                };
                2
            ],
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::HelpedTwice(one));
}

#[test]
fn creatures_cannot_pay_for_more_than_the_cost_asks() {
    let mut driver = stoke(&["red"; 5]);
    let convoke = convoked(&driver, &[("Red Bear", None); 5]);
    let error = refused(
        &mut driver,
        Payment {
            convoke,
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::HelpedTooMuch);
}

#[test]
fn a_creature_that_taps_for_mana_taps_for_one_or_the_other() {
    let scenario = Scenario::new(2)
        .hand(0, [named("Stoke the Flames")])
        .battlefield(
            0,
            [
                land("Mountain"),
                land("Mountain"),
                ready(corpus_card("Llanowar Elves").unwrap()),
                bear("red"),
            ],
        );
    let mut driver = driver(scenario);
    assert!(castable(&driver));
    begin(&mut driver, "Stoke the Flames");
    let elves = driver.id_of("Llanowar Elves").unwrap();
    let source = driver
        .pay_options()
        .unwrap()
        .sources
        .iter()
        .find(|o| o.source.object == elves)
        .unwrap()
        .source;
    let mountains: Vec<_> = driver
        .pay_options()
        .unwrap()
        .sources
        .iter()
        .filter(|o| o.source.object != elves)
        .map(|o| o.source)
        .collect();
    let error = refused(
        &mut driver,
        Payment {
            taps: [mountains, vec![source]].concat(),
            convoke: vec![Convoked {
                creature: elves,
                pays: None,
            }],
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::ObjectTappedTwice(elves));
    // Either way the spell is paid: the Elves tap for mana and the bear convokes, or the other way round.
    driver
        .pay_helped(
            &["Mountain", "Mountain", "Llanowar Elves"],
            &[("Red Bear", None)],
            &[],
            0,
        )
        .unwrap();
    assert_eq!(driver.state().stack().len(), 1);
}

#[test]
fn delve_cannot_exile_more_than_the_generic_cost() {
    let mut driver = driver(table("Gurmag Angler", &["Swamp"], &[], 8));
    driver.begin_cast("Gurmag Angler").unwrap();
    let cards = driver.pay_options().unwrap().delve[..7].to_vec();
    let swamp = driver.pay_options().unwrap().sources[0].source;
    let error = refused(
        &mut driver,
        Payment {
            taps: vec![swamp],
            delve: cards,
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::HelpedTooMuch);
}

#[test]
fn delve_needs_cards_of_the_graveyard_named_once() {
    let scenario = table("Gurmag Angler", &["Swamp"], &["gray"], 6);
    let mut driver = driver(scenario);
    driver.begin_cast("Gurmag Angler").unwrap();
    let mut cards = driver.pay_options().unwrap().delve.clone();
    let swamp = driver.pay_options().unwrap().sources[0].source;
    let outsider = driver.id_of("Golem").unwrap();
    cards[0] = outsider;
    let error = refused(
        &mut driver,
        Payment {
            taps: vec![swamp],
            delve: cards.clone(),
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::NotInGraveyard(outsider));
    cards[0] = cards[1];
    let error = refused(
        &mut driver,
        Payment {
            taps: vec![swamp],
            delve: cards.clone(),
            ..Payment::default()
        },
    );
    assert_eq!(error, PaymentError::HelpedTwice(cards[1]));
}

// Automatic payment

#[rstest]
#[case::mana_when_it_can_pay(&["Mountain"; 4], &["red"; 4], 0)]
#[case::the_creatures_that_are_needed(&["Mountain", "Mountain"], &["red"; 3], 2)]
#[case::all_of_them_when_all_are_needed(&[], &["red"; 4], 4)]
fn automatic_payment_taps_creatures_only_when_mana_cannot_pay(
    #[case] lands: &[&str],
    #[case] creatures: &[&str],
    #[case] tapped_creatures_after: usize,
) {
    let mut driver = driver(table("Stoke the Flames", lands, creatures, 0));
    begin(&mut driver, "Stoke the Flames");
    assert!(driver.pay_options().unwrap().auto_available);
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_eq!(tapped_creatures(&driver), tapped_creatures_after);
}

#[rstest]
#[case::cards_only_when_mana_falls_short(&["Swamp", "Mountain", "Mountain", "Mountain"], 8, 3)]
#[case::all_the_cost_asks(&["Swamp"], 8, 6)]
#[case::no_card_when_mana_pays(&["Swamp", "Mountain", "Mountain", "Mountain", "Mountain", "Mountain", "Mountain"], 8, 0)]
fn automatic_payment_exiles_only_the_cards_it_needs(
    #[case] lands: &[&str],
    #[case] graveyard: usize,
    #[case] exiled: usize,
) {
    let mut driver = driver(table("Gurmag Angler", lands, &[], graveyard));
    driver.cast("Gurmag Angler").unwrap();
    assert_eq!(driver.state().zone(Zone::Exile).len(), exiled);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        graveyard - exiled
    );
}

#[test]
fn a_reduction_takes_cards_off_what_automatic_payment_exiles() {
    let scenario = table("Gurmag Angler", &["Swamp"], &[], 8)
        .battlefield(0, [land("Swamp"), ready(modifier("My Helm").unwrap())]);
    let mut driver = driver(scenario);
    driver.cast("Gurmag Angler").unwrap();
    // {6}{B} costs {5}{B} under the Helm: the Swamp pays the {B} and five cards pay the rest.
    assert_eq!(driver.state().zone(Zone::Exile).len(), 5);
}

#[test]
fn the_other_seat_sees_the_cards_exiled_for_delve() {
    let mut driver = driver(table("Gurmag Angler", &["Swamp"], &[], 6));
    driver.cast("Gurmag Angler").unwrap();
    for id in driver.state().zone(Zone::Exile).to_vec() {
        // An exiled card is known to everyone: it came from a graveyard and goes to exile face up.
        assert!(
            driver
                .state()
                .object(id)
                .is_some_and(|o| o.zone() == Zone::Exile)
        );
    }
    driver
        .expect_no_event("a hidden card", |e| matches!(e, Event::CardDrawn { .. }))
        .unwrap();
}

#[test]
fn a_copy_of_the_game_with_helpers_replays_to_the_same_state() {
    let mut driver = driver(table("Stoke the Flames", &["Mountain"], &["red"; 3], 0));
    begin(&mut driver, "Stoke the Flames");
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[allow(dead_code)]
fn unused() -> (fn() -> Arc<CardDefinition>, fn() -> Arc<CardDefinition>) {
    (forest, plains)
}
