//! Goal: after Don't Blink resolves, until end of turn a creature that would enter from exile (a flicker) or after being
//! cast from exile (an Adventure's creature) is shuffled into its owner's library instead, while a creature cast from
//! the hand enters as usual; at two to four seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::pid;

/// Passes until the stack is empty.
fn r6105_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settling".to_owned()))
}

/// Whether a card called `name` lies in the zone.
fn r6105_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .any(|object| object.card().name == name)
}

#[derive(Clone, Copy, Debug)]
enum Way {
    Flicker,
    Adventure,
    Hand,
}

#[rstest]
fn r6105_creatures_from_exile_are_shuffled_away(
    #[values(2, 3, 4)] seats: usize,
    #[values(Way::Flicker, Way::Adventure, Way::Hand)] way: Way,
    #[values(true, false)] blink: bool,
) -> Result<(), ScriptError> {
    let lands = [
        island(),
        island(),
        plains(),
        mountain(),
        mountain(),
        mountain(),
        mountain(),
        mountain(),
        forest(),
        forest(),
    ];
    let mut hand = vec![
        card("Don't Blink").unwrap(),
        card("Cloudshift").unwrap(),
        bear(),
    ];
    hand.push(card("Bonecrusher Giant // Stomp").unwrap());
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![forest(); 12])
        .hand(0, hand)
        .battlefield(
            0,
            lands
                .into_iter()
                .map(ready)
                .chain([ready(card("Serra Angel").unwrap())]),
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if blink {
        driver.cast("Don't Blink")?;
        r6105_settle(&mut driver)?;
    }
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    let (name, shuffled) = match way {
        Way::Flicker => {
            let angel = driver.target_named("Serra Angel")?;
            driver.cast_at("Cloudshift", &[angel])?;
            ("Serra Angel", blink)
        }
        Way::Adventure => {
            driver.begin_cast("Bonecrusher Giant")?;
            choose_way(&mut driver, "Stomp")?;
            let target = driver.target_player(1);
            driver.choose_targets(&[target])?;
            pay(&mut driver)?;
            r6105_settle(&mut driver)?;
            assert!(
                r6105_in(&driver, Zone::Exile, "Bonecrusher Giant"),
                "on an adventure"
            );
            driver.begin_cast("Bonecrusher Giant")?;
            pay(&mut driver)?;
            ("Bonecrusher Giant", blink)
        }
        Way::Hand => {
            driver.cast("Grizzly Bears")?;
            ("Grizzly Bears", false)
        }
    };
    r6105_settle(&mut driver)?;
    assert_eq!(
        r6105_in(&driver, Zone::Battlefield, name),
        !shuffled,
        "{way:?}"
    );
    assert_eq!(
        r6105_in(&driver, Zone::Library(pid(0)), name),
        shuffled,
        "{way:?}"
    );
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        library + usize::from(shuffled)
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
