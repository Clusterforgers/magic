//! Goal: Ulamog, the Defiler plays as printed at two to four seats: its cast trigger makes the target opponent exile
//! the top half of their library rounded up (counted in that player's library, whatever its size), it enters with
//! +1/+1 counters equal to the greatest mana value among cards in exile, and it has annihilator X counted from its
//! +1/+1 counters as they change (`Modification::R6601KeywordX`, CR 702.86); the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Passes and answers until the stack is empty, aiming every target at the seat `aim`.
fn r6601_settle(driver: &mut Driver, aim: usize) {
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(aim);
                driver.choose_targets(&[target]).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// The +1/+1 counters on Ulamog.
fn r6601_counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Ulamog, the Defiler").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[test]
fn r6601_ulamog_plays_whole() {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support("Ulamog, the Defiler")
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{support:?}"
    );
}

#[rstest]
#[case::empty_library(0, 0, 0)]
#[case::one_card(1, 1, 4)]
#[case::two_cards(2, 1, 2)]
#[case::five_cards(5, 3, 2)]
#[case::eight_cards(8, 4, 2)]
fn r6601_ulamog_exiles_half_the_targets_library(
    #[values(2, 3, 4)] seats: usize,
    #[case] size: usize,
    #[case] exiled: usize,
    #[case] counters: u32,
) {
    let aim = seats - 1;
    // Top card first: a mana value 2 card on top and a mana value 4 card at the bottom.
    let mut library = Vec::new();
    for at in 0..size {
        library.push(if at + 1 == size {
            card("Tempt with Discovery").unwrap()
        } else if at == 0 {
            card("Rampant Growth").unwrap()
        } else {
            forest()
        });
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Ulamog, the Defiler").unwrap()])
        .battlefield(0, (0..10).map(|_| ready(forest())))
        .library(aim, library);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Ulamog, the Defiler").unwrap();
    r6601_settle(&mut driver, aim);
    assert_eq!(driver.state().zone(Zone::Exile).len(), exiled);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(aim))).len(),
        size - exiled
    );
    assert_eq!(r6601_counters(&driver), counters);
    let id = driver.id_of("Ulamog, the Defiler").unwrap();
    assert!(keywords_of(driver.state(), id).contains(&&Keyword::Annihilator(counters)));
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::three_counters(3, 2)]
#[case::one_counter(1, 4)]
#[case::no_counter(0, 5)]
fn r6601_ulamog_annihilates_by_its_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] left: usize,
) {
    let defender = seats - 1;
    let ulamog = with_counters(
        ready(card("Ulamog, the Defiler").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ulamog]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            (0..5)
                .map(|i| ready(free_creature(&format!("Pawn{seat}{i}"), 1, 1)))
                .collect::<Vec<_>>(),
        );
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.attack(&[("Ulamog, the Defiler", defender)]).unwrap();
    r6601_settle(&mut driver, defender);
    let held = |seat: usize| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
            .count()
    };
    assert_eq!(held(defender), left);
    for seat in 1..defender {
        assert_eq!(held(seat), 5, "seat {seat}");
    }
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
