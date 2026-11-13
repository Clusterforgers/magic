//! Goal: an alternative cost that reads the X of the mana cost announces that X at two to four seats: a Shoal offers
//! only the values of X a red card in hand has as its mana value and exiles that card, Conflagrate's flashback offers X
//! up to the cards left in hand and discards that many, the spell reads the X announced, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, ways};
use crate::common::pid;

/// What the announcement asked: the values of X offered (least, most, left out).
#[derive(Debug, Default)]
struct R379Asked {
    offered: Option<(u32, Option<u32>, Vec<u32>)>,
}

/// Casts `name` for the way whose label starts with `way` (when a way is asked), announcing `x`, targeting `target`,
/// and lets it resolve.
fn r379_cast(
    driver: &mut Driver,
    name: &str,
    way: &str,
    x: u32,
    target: Target,
) -> Result<R379Asked, ScriptError> {
    let mut asked = R379Asked::default();
    driver.begin_cast(name)?;
    for _ in 0..16 {
        if !driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) if ways(driver).is_ok() => choose_way(driver, way)?,
            DecisionKind::ChooseNumber(options) => {
                asked.offered = Some((options.min, options.max, options.except.clone()));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let take = usize::try_from(options.min.max(1))
                    .unwrap()
                    .min(options.from.len());
                let cards = options.from[..take].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(asked)
}

fn r379_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn r379_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

#[rstest]
#[case::the_three(3, "Ember", "Spark")]
#[case::the_one(1, "Spark", "Ember")]
fn r379_a_shoal_is_cast_by_exiling_a_card_of_mana_value_x(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
    #[case] exiled: &str,
    #[case] kept: &str,
) -> Result<(), ScriptError> {
    // No land: only the alternative cost can be paid. A red 3/3 for {2}{R} and a red 1/1 for {R}; a green card of
    // mana value 2 does not count.
    let table = || {
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(
                0,
                [
                    card("Blazing Shoal").unwrap(),
                    creature("Ember", 2, &[Color::Red], 3, 3),
                    creature("Spark", 0, &[Color::Red], 1, 1),
                    bear(),
                ],
            )
            .battlefield(0, [ready(bear())])
    };
    let mut driver = Driver::from_scenario(table())?;
    let target = driver.target_named("Grizzly Bears")?;
    let asked = r379_cast(&mut driver, "Blazing Shoal", "Cast for", x, target)?;
    assert_eq!(
        asked.offered,
        Some((1, Some(3), vec![2])),
        "only the mana values of red cards in hand"
    );
    assert!(r379_in(&driver, Zone::Exile, exiled), "{exiled} is exiled");
    assert!(r379_in(&driver, Zone::Hand(pid(0)), kept), "{kept} stays");
    let id = driver.id_of("Grizzly Bears")?;
    let power = mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .power
        .unwrap();
    assert_eq!(power, 2 + i32::try_from(x).unwrap(), "+X/+0");
    r379_replays(&driver, table());
    Ok(())
}

#[rstest]
fn r379_a_shoal_without_a_red_card_is_not_cast_for_free(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Blazing Shoal").unwrap(), bear()])
        .battlefield(0, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(
        driver.begin_cast("Blazing Shoal").is_err(),
        "nothing pays either cost"
    );
    Ok(())
}

#[rstest]
fn r379_conflagrate_flashback_discards_x(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] x: u32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .graveyard(0, [card("Conflagrate").unwrap()])
            .hand(0, [bear(), bear(), bear()])
            .battlefield(0, (0..2).map(|_| ready(mountain())).collect::<Vec<_>>())
    };
    let mut driver = Driver::from_scenario(table())?;
    let foe = driver.target_player(1);
    let asked = r379_cast(&mut driver, "Conflagrate", "Flashback", x, foe)?;
    assert_eq!(
        asked.offered.map(|o| (o.0, o.1)),
        Some((0, Some(3))),
        "up to the hand"
    );
    let state = driver.state();
    assert_eq!(
        state.zone(Zone::Hand(pid(0))).len(),
        3 - x as usize,
        "X cards discarded"
    );
    assert_eq!(
        state.player(pid(1)).unwrap().life(),
        20 - i32::try_from(x).unwrap(),
        "X damage to the one target"
    );
    assert!(
        r379_in(&driver, Zone::Exile, "Conflagrate"),
        "flashback exiles it"
    );
    r379_replays(&driver, table());
    Ok(())
}
