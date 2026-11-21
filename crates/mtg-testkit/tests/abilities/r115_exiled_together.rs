//! Goal: a batch trigger over cards put into exile from a hand or permanents a spell or ability of the controller exiles
//! (CR 603.2c, 603.10a) at two to four seats: Ranar the Ever-Watchful makes one Spirit when its controller foretells a
//! card and one when their Swords to Plowshares exiles a creature, and none when an opponent's Swords exiles one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Ranar and two Plains and holds Augury Raven and Swords to Plowshares; seat 1 controls a Grizzly Bears
/// and a Plains and holds Swords to Plowshares; `active` takes its first main phase.
fn r115_table(seats: usize, active: usize) -> Scenario {
    Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Ranar the Ever-Watchful").unwrap()),
                ready(plains()),
                ready(plains()),
            ],
        )
        .hand(
            0,
            [
                card("Augury Raven").unwrap(),
                card("Swords to Plowshares").unwrap(),
            ],
        )
        .battlefield(1, [ready(bear()), ready(plains())])
        .hand(1, [card("Swords to Plowshares").unwrap()])
}

/// The Spirit tokens seat 0 controls.
fn r115_spirits(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Spirit" && object.controller() == PlayerId(0))
        .count()
}

/// The card called `name` in the zone.
fn r115_find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

/// Answers the payment with the engine's own and lets the stack resolve.
fn r115_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Ok(())
}

#[rstest]
fn r115_foretelling_a_card_makes_a_spirit(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r115_table(seats, 0))?;
    let raven = r115_find(&driver, Zone::Hand(PlayerId(0)), "Augury Raven");
    driver.answer(Answer::Priority(PriorityAction::Foretell(raven)))?;
    r115_settle(&mut driver)?;
    assert_eq!(r115_spirits(&driver), 1);
    Ok(())
}

#[rstest]
#[case::their_creature_by_my_spell(0, 1, 1)]
#[case::my_creature_by_their_spell(1, 0, 0)]
fn r115_a_permanent_exiled_by_a_spell_of_mine_makes_a_spirit(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] victim: usize,
    #[case] spirits: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r115_table(seats, caster))?;
    let victim_name = if victim == 0 {
        "Ranar the Ever-Watchful"
    } else {
        "Grizzly Bears"
    };
    let target = driver.target_named(victim_name)?;
    let caster_id = PlayerId::from_index(caster);
    let swords = r115_find(&driver, Zone::Hand(caster_id), "Swords to Plowshares");
    driver.answer(Answer::Priority(PriorityAction::CastSpell(swords)))?;
    for _ in 0..8 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target])?;
            }
            _ => break,
        }
    }
    r115_settle(&mut driver)?;
    let state = driver.state();
    let exiled = state
        .zone(Zone::Exile)
        .iter()
        .any(|id| state.object(*id).unwrap().card().name == victim_name);
    assert!(exiled, "{victim_name} is exiled");
    assert_eq!(r115_spirits(&driver), spirits);
    Ok(())
}
