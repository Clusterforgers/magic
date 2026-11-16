//! Goal: Zack Fair plays as printed at two to four seats: it enters with a +1/+1 counter, and sacrificing it gives the
//! target creature indestructible until end of turn, every kind of counter Zack had as it was sacrificed, and the
//! Equipment that was attached to Zack (judged as Zack last existed, CR 608.2h, though the Equipment has become
//! unattached since); with no Equipment nothing is attached; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{attached_to, with_counters};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn r6601_zack_passes_its_counters_and_equipment(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] more: u32,
    #[values(false, true)] equipped: bool,
) {
    let zack = with_counters(
        with_counters(
            ready(card("Zack Fair").unwrap()),
            CounterKind::PlusOnePlusOne,
            1 + more,
        ),
        CounterKind::Other("charge".into()),
        more,
    );
    let mut mine = vec![zack, ready(free_creature("Ally", 1, 1)), ready(forest())];
    if equipped {
        mine.push(attached_to(ready(card("Bonesplitter").unwrap()), 0, 0));
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.begin_activation("Zack Fair").unwrap();
    let ally = driver.target_named("Ally").unwrap();
    driver.choose_targets(&[ally]).unwrap();
    for _ in 0..30 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert!(driver.id_of("Zack Fair").is_err(), "Zack was sacrificed");
    let ally = driver.id_of("Ally").unwrap();
    let held = driver.state().object(ally).unwrap().permanent().unwrap();
    assert_eq!(held.counters_of(&CounterKind::PlusOnePlusOne), 1 + more);
    assert_eq!(held.counters_of(&CounterKind::Other("charge".into())), more);
    assert!(keywords_of(driver.state(), ally).contains(&&Keyword::Indestructible));
    let equipment = driver.id_of("Bonesplitter").ok();
    assert_eq!(equipment.is_some(), equipped);
    if let Some(equipment) = equipment {
        assert_eq!(driver.state().host_of(equipment), Some(ally));
    }
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r6601_zack_enters_with_a_counter(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Zack Fair").unwrap()])
        .battlefield(0, [ready(plains())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Zack Fair").unwrap();
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let zack = driver.id_of("Zack Fair").unwrap();
    let counters = driver
        .state()
        .object(zack)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 1);
}
