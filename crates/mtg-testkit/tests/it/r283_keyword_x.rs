//! Goal: an `{X}` that only a keyword cost holds ("Foretell {X}{X}{W}") is announced when the spell is cast that way,
//! paid once for each `{X}` and read by the spell, while a cast for the mana cost has X = 0 and asks nothing, at tables
//! of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::plains;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The id of the card called `name` in a zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The Angel Warrior tokens seat 0 controls.
fn angels(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(0) && o.card().name == "Angel Warrior")
        .count()
}

/// Casts the card, answering X with `x`, and resolves it; returns whether X was asked.
fn cast_and_resolve(driver: &mut Driver, id: ObjectId, x: u32) -> Result<bool, ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    let (mut asked, mut paid) = (false, false);
    for _ in 0..80 {
        if paid
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                asked = true;
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::PayMana(_) => {
                paid = true;
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(asked)
}

#[rstest]
#[case::foretold_for_two(Some(2), 2, 5)]
#[case::foretold_for_three(Some(3), 3, 7)]
#[case::foretold_for_zero(Some(0), 0, 1)]
#[case::cast_from_the_hand(None, 1, 4)]
fn r283_starnheim_unleashed_reads_the_x_of_its_foretell_cost(
    #[case] foretold_x: Option<u32>,
    #[case] made: usize,
    #[case] spent: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .library(0, vec![plains(); 12])
            .hand_named(0, ["Starnheim Unleashed"])
            .battlefield(0, (0..12).map(|_| ready(plains())).collect::<Vec<_>>()),
    )?;
    let untapped = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|o| {
                o.card().name != "Angel Warrior" && o.permanent().is_some_and(|p| !p.tapped())
            })
            .count()
    };
    if let Some(x) = foretold_x {
        let id = find(&driver, Zone::Hand(pid(0)), "Starnheim Unleashed");
        driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        // The foretold card is cast on its owner's next turn.
        driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
        while driver.decision()?.player != pid(0) {
            driver.pass()?;
        }
        let id = find(&driver, Zone::Exile, "Starnheim Unleashed");
        let before = untapped(&driver);
        let asked = cast_and_resolve(&mut driver, id, x)?;
        assert!(asked, "X is announced for the foretell cost");
        assert_eq!(before - untapped(&driver), spent);
    } else {
        let id = find(&driver, Zone::Hand(pid(0)), "Starnheim Unleashed");
        let before = untapped(&driver);
        let asked = cast_and_resolve(&mut driver, id, 5)?;
        assert!(!asked, "the mana cost has no X");
        assert_eq!(before - untapped(&driver), spent);
    }
    assert_eq!(angels(&driver), made);
    Ok(())
}
