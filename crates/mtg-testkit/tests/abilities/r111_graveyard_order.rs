//! Goal: "reorder your graveyard as you choose" (Fossil Find, CR 404.2) at 2 to 4 seats: the caster is shown the cards
//! left in their graveyard top card first and the graveyard takes the order they answer, a graveyard of one card asks
//! nothing, the other graveyards keep their order, and each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::OrderChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

const GRAVEYARD: [&str; 4] = [
    "Grizzly Bears",
    "Lightning Bolt",
    "Llanowar Elves",
    "Giant Growth",
];

fn table(seats: usize, cards: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Fossil Find"])
        .battlefield(0, [ready(mountain())])
        .graveyard_named(0, &GRAVEYARD[..cards]);
    for seat in 1..seats {
        scenario = scenario.graveyard_named(seat, GRAVEYARD);
    }
    scenario
}

#[rstest]
#[case::three_left_are_reordered(4, true)]
#[case::one_left_asks_nothing(2, false)]
fn fossil_find_reorders_the_casters_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] cards: usize,
    #[case] asks: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, cards).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, cards))?;
    let others: Vec<_> = (1..seats)
        .map(|seat| driver.state().zone(Zone::Graveyard(pid(seat))).to_vec())
        .collect();
    driver.cast("Fossil Find")?;
    let mut answered = None;
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::OrderCards(options) => {
                assert_eq!(decision.player, pid(0));
                // Shown top card first: the graveyard as it lies, reversed.
                let mut shown = driver.state().zone(Zone::Graveyard(pid(0))).to_vec();
                shown.reverse();
                assert_eq!(options.cards, shown);
                let mut wanted = shown;
                wanted.rotate_left(1);
                driver.answer(Answer::OrderCards(OrderChoice(wanted.clone())))?;
                answered = Some(wanted);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(answered.is_some(), asks);
    let graveyard = driver.state().zone(Zone::Graveyard(pid(0))).to_vec();
    // One card went to the hand; Fossil Find lies on top of what was ordered.
    assert_eq!(graveyard.len(), cards);
    if let Some(mut wanted) = answered {
        wanted.reverse();
        assert_eq!(graveyard[..cards - 1], wanted[..]);
    }
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(driver.state().zone(Zone::Graveyard(pid(seat))), &before[..]);
    }
    replays(&driver, &setup);
    Ok(())
}
