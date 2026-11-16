//! Goal: the r67-00 pieces play as their cards print them at two to four seats: Edgar's first set of coin flips each
//! turn is won, Quantum Riddler draws one more for each draw instruction only while its controller holds one or fewer
//! cards, and The Valeyard makes each opponent face a villainous choice an additional time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Answers passively until the stack is empty and nothing resolves.
fn r6700_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The coin flips of the game so far, won or lost.
fn r6700_flips(driver: &Driver) -> Vec<bool> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

/// Seat 0 activates both Wirefly Hives, one after the other, with or without Edgar.
fn r6700_two_hives(seats: usize, seed: u64, edgar: bool) -> Result<Vec<bool>, ScriptError> {
    let mut mine = vec![
        ready(card("Wirefly Hive").unwrap()),
        ready(card("Wirefly Hive").unwrap()),
    ];
    mine.extend((0..6).map(|_| ready(island())));
    if edgar {
        mine.push(ready(card("Edgar, King of Figaro").unwrap()));
    }
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..2 {
        driver.begin_activation("Wirefly Hive")?;
        r6700_settle(&mut driver)?;
    }
    Ok(r6700_flips(&driver))
}

#[rstest]
fn r6700_edgar_wins_the_first_flip_of_the_turn_only(#[values(2, 3, 4)] seats: usize) {
    let mut seconds = Vec::new();
    let mut firsts_without = Vec::new();
    for seed in 1..=12 {
        let flips = r6700_two_hives(seats, seed, true).unwrap();
        assert_eq!(flips.len(), 2, "one flip for each Hive");
        assert!(
            flips[0],
            "Edgar wins the first flip of the turn (seed {seed})"
        );
        seconds.push(flips[1]);
        firsts_without.push(r6700_two_hives(seats, seed, false).unwrap()[0]);
    }
    // A later resolution of the same turn flips as usual, and so does the first flip without Edgar.
    assert!(seconds.contains(&false), "{seconds:?}");
    assert!(firsts_without.contains(&false), "{firsts_without:?}");
}

/// The cards in seat 0's hand after its next draw step, Quantum Riddler on its battlefield and `held` cards in its hand.
fn r6700_after_draw(seats: usize, held: usize) -> Result<usize, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, seats - 1, Step::Main1)
        .battlefield(0, [ready(card("Quantum Riddler").unwrap())])
        .hand(0, (0..held).map(|_| island()))
        .library(0, (0..10).map(|_| island()));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    Ok(driver.state().zone(Zone::Hand(pid(0))).len())
}

#[rstest]
#[case::an_empty_hand_draws_two(0, 2)]
#[case::one_card_draws_two(1, 3)]
#[case::two_cards_draw_one(2, 3)]
#[case::three_cards_draw_one(3, 4)]
fn r6700_quantum_riddler_adds_one_to_a_draw_from_a_small_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: usize,
    #[case] after: usize,
) {
    assert_eq!(r6700_after_draw(seats, held).unwrap(), after);
}

/// The creatures each opponent controls after seat 0's beginning of combat with The Dalek Emperor, and The Valeyard when
/// `valeyard`; each opponent starts with three.
fn r6700_left_after_choices(seats: usize, valeyard: bool) -> Result<Vec<usize>, ScriptError> {
    let mut mine = vec![ready(card("The Dalek Emperor").unwrap())];
    if valeyard {
        mine.push(ready(card("The Valeyard").unwrap()));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            ["A", "B", "C"].map(|name| ready(free_creature(name, 1, 1))),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    r6700_settle(&mut driver)?;
    let state = driver.state();
    Ok((1..seats)
        .map(|seat| {
            state
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| {
                    ["A", "B", "C"].contains(&name_of(state, **id).as_str())
                        && mtg_engine::characteristics::controller_of(state, **id)
                            == Some(pid(seat))
                })
                .count()
        })
        .collect())
}

#[rstest]
#[case::without_the_valeyard(false, 2)]
#[case::with_the_valeyard(true, 1)]
fn r6700_the_valeyard_makes_opponents_face_the_choice_again(
    #[values(2, 3, 4)] seats: usize,
    #[case] valeyard: bool,
    #[case] left: usize,
) {
    // The passive answer sacrifices: each time an opponent faces the choice, they lose a creature.
    let found = r6700_left_after_choices(seats, valeyard).unwrap();
    assert_eq!(found, vec![left; seats - 1]);
}
