//! Goal: clash plays whole at tables of two, three and four seats: Lash Out deals its second 3 damage to the controller of its target only when the caster's revealed card has the greater mana value, Hoarder's Greed repeats its loss of life and draws while the caster wins, a card put on the bottom leaves the top, the opponent is chosen at three and four seats, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A vanilla creature card of mana value `value`.
fn w9b_costing(value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Cost {value}"))
            .with_type(CardType::Creature)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(1, 1),
    )
}

/// Seat 0 holds `spell` and six Swamps and Mountains; seat 0's library is `mine` (top first), every other library
/// eight cards of mana value `theirs`, and seat 1 controls a Bear.
fn w9b_table(seats: usize, spell: &str, mine: &[u32], theirs: u32) -> Scenario {
    let lands: Vec<_> = (0..6)
        .map(|n| ready(land(if n % 2 == 0 { Color::Black } else { Color::Red })))
        .collect();
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(0, lands)
        .battlefield(1, [ready(bear())])
        .library(0, mine.iter().map(|value| w9b_costing(*value)));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| w9b_costing(theirs)));
    }
    scenario
}

/// Passes, pays, picks the last opponent offered and answers each top-or-bottom question with `bottom` until the stack
/// is empty; the number of clash questions answered.
fn w9b_resolve(driver: &mut Driver, bottom: bool) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) => {
                let last = options.options.len() - 1;
                driver.answer(Answer::ChooseOption(OptionChoice(last)))?;
            }
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(bottom)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

fn w9b_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn w9b_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn lash_out_hits_the_controller_only_on_a_win(
    #[values(2, 3, 4)] seats: usize,
    #[values((5, 1, true), (1, 1, false), (0, 3, false))] clash: (u32, u32, bool),
) -> Result<(), ScriptError> {
    let (mine, theirs, won) = clash;
    let scenario = || w9b_table(seats, "Lash Out", &[mine; 4], theirs);
    let mut driver = Driver::from_scenario(scenario())?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lash Out", &[bear])?;
    let asked = w9b_resolve(&mut driver, false)?;
    assert_eq!(asked, 2, "each clashing player chooses top or bottom");
    assert!(!driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Grizzly Bears")
    }));
    assert_eq!(w9b_life(&driver, 1), if won { 17 } else { 20 });
    for seat in 2..seats {
        assert_eq!(
            w9b_life(&driver, seat),
            20,
            "only the controller of the target"
        );
    }
    w9b_replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::lost_at_once(&[0, 0, 0, 0, 0, 0], 18, 2)]
#[case::won_once(&[0, 0, 5, 0, 0, 0, 0, 0], 16, 4)]
#[case::won_twice(&[0, 0, 5, 0, 5, 0, 0, 0], 14, 6)]
fn hoarders_greed_repeats_while_the_caster_wins(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[u32],
    #[case] life: i32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let scenario = || w9b_table(seats, "Hoarder's Greed", mine, 1);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Hoarder's Greed")?;
    w9b_resolve(&mut driver, false)?;
    assert_eq!(w9b_life(&driver, 0), life);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), drawn);
    w9b_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn a_card_put_on_the_bottom_leaves_the_top(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || w9b_table(seats, "Lash Out", &[5, 0, 0, 0], 1);
    let mut driver = Driver::from_scenario(scenario())?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lash Out", &[bear])?;
    w9b_resolve(&mut driver, true)?;
    let library = driver.state().zone(Zone::Library(pid(0)));
    let bottom = driver.state().object(library[0]).unwrap();
    assert_eq!(
        bottom.card().name,
        "Cost 5",
        "the revealed card is now the bottom card"
    );
    assert_eq!(w9b_life(&driver, 1), 17);
    w9b_replays(&driver, scenario());
    Ok(())
}
