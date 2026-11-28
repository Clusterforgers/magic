//! Goal: copies of exiled objects and a token with the triggered abilities of exiled cards play whole, at two to four
//! seats: Niko's Shards are copies of the exiled creature until the next end step of whichever player (activated in an
//! end step, the next turn's), and the creature returns as that step begins; Olag becomes a copy of a creature card
//! exiled by Ludevic's cost, with its exceptions and a +1/+1 counter for each such card; The Book of Vile Darkness makes
//! Vecna with the triggered abilities of the three exiled cards, whose enters trigger and end step trigger fire; each log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, creature, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

const NIKO: &str = "Niko, Light of Hope";
const LUDEVIC: &str = "Ludevic, Necrogenius";
const OLAG: &str = "Olag, Ludevic's Hubris";
const BOOK: &str = "The Book of Vile Darkness";

fn p158a_replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Answers passively until nothing is on the stack or waits to go there, taking every card a choice of cards offers
/// (Ludevic's cost) and `x` for a number.
fn p158a_settle(driver: &mut Driver, x: u32) {
    for _ in 0..300 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
            && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            return;
        }
        p158a_answer(driver, x);
    }
    panic!("the stack does not empty");
}

fn p158a_answer(driver: &mut Driver, x: u32) {
    let decision = driver.decision().unwrap().clone();
    let answer = match &decision.kind {
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
        DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
            options
                .from
                .iter()
                .copied()
                .take((options.max as usize).max(options.min as usize).max(1))
                .collect(),
        )),
        _ => Driver::passive_answer(&decision),
    };
    driver.answer(answer).unwrap();
}

/// Passes until seat 0 has priority in the current step.
fn p158a_priority_to_seat_zero(driver: &mut Driver) {
    for _ in 0..20 {
        let decision = driver.decision().unwrap();
        if decision.player == pid(0) && matches!(decision.kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("seat 0 never had priority");
}

/// The permanent's name, power and toughness as they are now.
fn p158a_seen(driver: &Driver, id: ObjectId) -> (String, Option<i32>, Option<i32>) {
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    let name = driver.state().object(id).unwrap().card().name.clone();
    (name, seen.power, seen.toughness)
}

fn p158a_named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .collect()
}

fn p158a_shard() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shard")
            .with_type(CardType::Enchantment)
            .with_subtype("Shard"),
    )
}

#[rstest]
#[case::own_main_phase(0, Step::Main1, 1)]
#[case::an_opponents_main_phase(1, Step::Main1, 1)]
#[case::own_end_step(0, Step::End, 2)]
fn p158a_niko_shards_copy_until_the_next_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[case] active: usize,
    #[case] step: Step,
    #[case] ends_in_turn: u32,
) {
    let mut board = vec![
        ready(card(NIKO).unwrap()),
        ready(p158a_shard()),
        ready(p158a_shard()),
        ready(bear()),
    ];
    board.extend((0..2).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .at(1, active, step)
        .battlefield(0, board)
        .battlefield(1, [ready(p158a_shard())]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let shards = p158a_named(&driver, Zone::Battlefield, "Shard");
    p158a_priority_to_seat_zero(&mut driver);
    driver.begin_activation(NIKO).unwrap();
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let bear = driver.state().object(bear).unwrap().reference();
    driver.choose_targets(&[Target::Object(bear)]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    p158a_settle(&mut driver, 0);
    assert_eq!(p158a_named(&driver, Zone::Exile, "Grizzly Bears").len(), 1);
    let theirs = shards
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().controller() == pid(1));
    for shard in theirs {
        assert_eq!(
            p158a_seen(&driver, shard).0,
            "Shard",
            "only seat 0's Shards copy"
        );
    }
    let mine: Vec<ObjectId> = shards
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().controller() == pid(0))
        .collect();
    assert_eq!(mine.len(), 2);
    for shard in &mine {
        assert_eq!(
            p158a_seen(&driver, *shard),
            ("Grizzly Bears".to_owned(), Some(2), Some(2))
        );
    }
    if ends_in_turn > 1 {
        driver.idle_until(ends_in_turn, Step::Upkeep).unwrap();
        assert_eq!(p158a_seen(&driver, mine[0]).0, "Grizzly Bears");
        assert_eq!(p158a_named(&driver, Zone::Exile, "Grizzly Bears").len(), 1);
    }
    driver.idle_until(ends_in_turn, Step::End).unwrap();
    for shard in &mine {
        assert_eq!(
            p158a_seen(&driver, *shard),
            ("Shard".to_owned(), None, None)
        );
    }
    p158a_settle(&mut driver, 0);
    assert_eq!(
        p158a_named(&driver, Zone::Battlefield, "Grizzly Bears").len(),
        1,
        "the creature returns at the beginning of the next end step"
    );
    p158a_replays(&scenario, &driver);
}

/// A green 1/1 creature card with flying.
fn p158a_flyer() -> Arc<CardDefinition> {
    let flyer = (*creature("Wind Beast", 1, &[Color::Green], 1, 1)).clone();
    Arc::new(flyer.with_rules(Ability::Keyword(Keyword::Flying)))
}

#[rstest]
#[case::one_card(1)]
#[case::two_cards(2)]
#[case::three_cards(3)]
fn p158a_olag_copies_a_creature_card_exiled_with_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
) {
    let mut board = vec![ready(
        card("Ludevic, Necrogenius // Olag, Ludevic's Hubris").unwrap(),
    )];
    board.extend((0..2 + x).map(|_| ready(island())));
    board.extend((0..2).map(|_| ready(swamp())));
    // X creature cards and a land the cost cannot take.
    let mut graveyard: Vec<_> = (0..x).map(|_| p158a_flyer()).collect();
    graveyard.push(island());
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .graveyard(0, graveyard);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let ludevic = driver.id_of(LUDEVIC).unwrap();
    driver.begin_activation(LUDEVIC).unwrap();
    p158a_settle(&mut driver, x);
    let exiled = driver.state().zone(Zone::Exile).len();
    assert_eq!(exiled, x as usize, "the cost exiles X creature cards");
    assert_eq!(
        p158a_seen(&driver, ludevic),
        (
            OLAG.to_owned(),
            Some(4 + i32::try_from(x).unwrap()),
            Some(4 + i32::try_from(x).unwrap())
        )
    );
    let olag = driver.state().object(ludevic).unwrap();
    assert_eq!(
        olag.permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        x
    );
    let seen = mtg_engine::characteristics::characteristics(driver.state(), ludevic).unwrap();
    assert!(mtg_engine::characteristics::has_supertype(
        driver.state(),
        ludevic,
        Supertype::Legendary
    ));
    let printed = &driver.state().object(ludevic).unwrap().card().subtypes;
    assert!(
        printed
            .iter()
            .chain(&seen.subtypes)
            .any(|s| s.0 == "Zombie")
    );
    for color in [Color::Blue, Color::Black] {
        assert!(seen.colors.contains(color), "{color:?}");
    }
    // Every card exiled is a flyer, so Olag copies one.
    assert!(seen.colors.contains(Color::Green));
    let flies = mtg_engine::ability_reader::abilities_of(driver.state(), ludevic)
        .iter()
        .any(|(_, rules)| matches!(&**rules, Ability::Keyword(Keyword::Flying)));
    assert!(flies, "Olag has the abilities of the card it copies");
    p158a_replays(&scenario, &driver);
}

#[rstest]
fn p158a_book_makes_vecna_with_the_exiled_triggers(#[values(2, 3, 4)] seats: usize) {
    let mut board = vec![
        ready(card(BOOK).unwrap()),
        ready(card("Eye of Vecna").unwrap()),
        ready(card("Hand of Vecna").unwrap()),
    ];
    board.extend((0..2).map(|_| ready(swamp())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .library(0, vec![island(); 5]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_activation(BOOK).unwrap();
    p158a_settle(&mut driver, 0);
    for exiled in [BOOK, "Eye of Vecna", "Hand of Vecna"] {
        assert_eq!(
            p158a_named(&driver, Zone::Exile, exiled).len(),
            1,
            "{exiled}"
        );
    }
    let vecna = driver.id_of("Vecna").unwrap();
    assert_eq!(
        p158a_seen(&driver, vecna),
        ("Vecna".to_owned(), Some(8), Some(8))
    );
    let abilities = mtg_engine::ability_reader::abilities_of(driver.state(), vecna);
    let triggered = abilities
        .iter()
        .filter(|(_, rules)| matches!(&**rules, Ability::Triggered { .. }))
        .count();
    assert_eq!(triggered, 4, "the Book's, the Eye's two and the Hand's");
    assert!(
        abilities
            .iter()
            .any(|(_, rules)| matches!(&**rules, Ability::Keyword(Keyword::Indestructible)))
    );
    // The Eye's enters trigger fires as Vecna enters: draw a card and lose 2 life.
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + 1);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life - 2);
    // Having lost 2 life, the Book's end step trigger on Vecna makes a Zombie.
    driver.idle_until(1, Step::End).unwrap();
    p158a_settle(&mut driver, 0);
    assert_eq!(p158a_named(&driver, Zone::Battlefield, "Zombie").len(), 1);
    p158a_replays(&scenario, &driver);
}
