//! Goal: "[Quality] offering" casts the card at instant timing by sacrificing a permanent of the quality and paying the difference in mana costs, colour included (a coloured symbol the card lacks takes off generic mana): offered beside the mana cost in a main phase, alone on another player's turn, never without a permanent of the quality; the permanent picked decides the cost; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, ways};
use crate::common::{names, pid};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `patron`, has eight Mountains and the corpus permanents `own`; seat `active` is the active player.
fn table(seats: usize, active: usize, patron: &str, own: &[&str]) -> Scenario {
    let mut mine = lands(8, mountain);
    mine.extend(own.iter().map(|name| ready(card(name).unwrap())));
    Scenario::new(seats)
        .at(1, active, Step::Main1)
        .hand(0, [card(patron).unwrap()])
        .battlefield(0, mine)
        .battlefield(active.max(1), lands(1, plains))
}

/// Gives seat 0 priority: the players before it pass.
fn to_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..4 {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Casts `patron` by its offering, sacrificing `sacrificed` when asked; the cost asked to pay.
fn offer(driver: &mut Driver, patron: &str, sacrificed: &str) -> Result<String, ScriptError> {
    driver.begin_cast(patron)?;
    for _ in 0..6 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => choose_way(driver, "Offering")?,
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                let pick = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| {
                        state
                            .object(*id)
                            .is_some_and(|o| o.card().name == sacrificed)
                    })
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            DecisionKind::PayMana(options) => {
                let cost = options.cost.to_string();
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                return Ok(cost);
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("no payment was asked")
}

#[rstest]
#[case::a_goblin_of_one_and_red("Mogg Flunkies", "{3}{R}")]
#[case::a_goblin_of_one_and_two_red("Squee, the Immortal", "{3}")]
#[case::a_one_drop_goblin("Goblin Spy", "{4}{R}")]
fn the_mana_cost_of_the_goblin_sacrificed_comes_off(
    #[values(2, 3, 4)] seats: usize,
    #[case] goblin: &str,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let own = ["Mogg Flunkies", "Squee, the Immortal", "Goblin Spy"];
    let mut driver = Driver::from_scenario(table(seats, 0, "Patron of the Akki", &own))?;
    assert_eq!(offer(&mut driver, "Patron of the Akki", goblin)?, cost);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&goblin.to_owned()));
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}

#[rstest]
fn offering_is_offered_beside_the_mana_cost_in_a_main_phase(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, 0, "Patron of the Akki", &["Mogg Flunkies"]))?;
    driver.begin_cast("Patron of the Akki")?;
    let offered = ways(&driver)?;
    assert_eq!(offered.len(), 2, "{offered:?}");
    assert!(
        offered.iter().any(|label| label.starts_with("Offering")),
        "{offered:?}"
    );
    Ok(())
}

#[rstest]
#[case::with_a_goblin(&["Mogg Flunkies"], true)]
#[case::without_one(&[], false)]
fn on_another_players_turn_only_the_offering_casts_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] own: &[&str],
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1, "Patron of the Akki", own))?;
    to_seat_zero(&mut driver)?;
    assert_eq!(driver.decision()?.player, pid(0));
    if castable {
        assert_eq!(
            offer(&mut driver, "Patron of the Akki", "Mogg Flunkies")?,
            "{3}{R}"
        );
    } else {
        assert!(driver.begin_cast("Patron of the Akki").is_err());
    }
    Ok(())
}

#[rstest]
fn an_artifact_offering_takes_off_a_colourless_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, 0, "Blast-Furnace Hellkite", &["Sol Ring"]))?;
    assert_eq!(
        offer(&mut driver, "Blast-Furnace Hellkite", "Sol Ring")?,
        "{6}{R}{R}"
    );
    Ok(())
}
