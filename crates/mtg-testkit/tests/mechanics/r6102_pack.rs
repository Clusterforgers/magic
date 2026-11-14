//! Goal: the r61-02 counter-kind pieces run in play at two to four seats: another Dinosaur enters with one counter of
//! each kind on Blue, Loyal Raptor, and Crystalline Giant gets one counter of a kind it lacks at the beginning of combat
//! on its controller's turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;

/// Priority passes until the stack is empty; payments are automatic and other questions get a passive answer.
fn r6102_settle(driver: &mut Driver) {
    for _ in 0..120 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// The counters on the permanent named, by kind name.
fn r6102_counters(driver: &Driver, name: &str) -> BTreeMap<String, u32> {
    let id = driver.id_of(name).unwrap();
    let state = driver.state();
    state
        .object(id)
        .and_then(|object| object.permanent())
        .map(|permanent| {
            permanent
                .counters()
                .map(|(kind, n)| (kind.name().to_owned(), n))
                .collect()
        })
        .unwrap_or_default()
}

fn r6102_kind(name: &str) -> CounterKind {
    match name {
        "+1/+1" => CounterKind::PlusOnePlusOne,
        other => CounterKind::Other(other.to_owned()),
    }
}

#[rstest]
#[case::two_kinds(&[("+1/+1", 2), ("flying", 1)])]
#[case::one_kind(&[("shield", 3)])]
#[case::no_counters(&[])]
fn r6102_another_dinosaur_enters_with_the_raptors_kinds(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &[(&str, u32)],
) {
    let mut raptor = ready(card("Blue, Loyal Raptor").unwrap());
    for (kind, count) in held {
        raptor = with_counters(raptor, r6102_kind(kind), *count);
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                raptor,
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, [card("Bonded Horncrest").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bonded Horncrest").unwrap();
    r6102_settle(&mut driver);
    let expected: BTreeMap<String, u32> = held
        .iter()
        .map(|(kind, _)| ((*kind).to_owned(), 1))
        .collect();
    assert_eq!(r6102_counters(&driver, "Bonded Horncrest"), expected);
}

#[rstest]
#[case::none_held(&[])]
#[case::two_held(&["flying", "reach"])]
#[case::all_but_one(&["flying", "first strike", "deathtouch", "hexproof", "lifelink", "menace", "reach", "trample", "vigilance"])]
fn r6102_the_giant_gets_a_kind_it_lacks(#[values(2, 3, 4)] seats: usize, #[case] held: &[&str]) {
    let mut giant = ready(card("Crystalline Giant").unwrap());
    for kind in held {
        giant = with_counters(giant, r6102_kind(kind), 1);
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [giant]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::BeginCombat).unwrap();
    r6102_settle(&mut driver);
    let counters = r6102_counters(&driver, "Crystalline Giant");
    assert_eq!(counters.len(), held.len() + 1, "{counters:?}");
    assert!(counters.values().all(|n| *n == 1), "{counters:?}");
    if held.len() == 9 {
        assert!(counters.contains_key("+1/+1"));
    }
}
