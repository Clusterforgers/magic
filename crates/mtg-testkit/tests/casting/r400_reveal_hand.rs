//! Goal: Land Grant is cast by revealing its caster's hand in place of its mana cost at tables of two, three and four
//! seats while that hand holds no land card: every card of the hand is shown at once with nothing to choose, and the spell
//! resolves and finds a Forest; with a land card in hand the way is not offered (a spell with no mana to pay for it is
//! not castable); the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Event;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::{choose_way, pay, ways};
use crate::common::pid;

fn r400_offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

fn r400_names_in_hand(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

/// Seat 0 holds Land Grant and `beside` in hand and no lands on the battlefield; its library holds Forests.
#[rstest]
#[case::a_creature_beside(&["Grizzly Bears"], true)]
#[case::nothing_beside(&[], true)]
#[case::a_land_beside(&["Forest"], false)]
#[case::a_land_among_others(&["Grizzly Bears", "Forest"], false)]
fn r400_land_grant_reveals_a_hand_without_lands(
    #[values(2, 3, 4)] seats: usize,
    #[case] beside: &[&str],
    #[case] open: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec!["Land Grant"];
    hand.extend_from_slice(beside);
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand_named(0, hand.clone())
        .library_named(0, ["Forest", "Forest", "Grizzly Bears"]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r400_offered(&driver, "Land Grant")?, open);
    if !open {
        return Ok(());
    }
    let log = driver.engine().events().len();
    driver.begin_cast("Land Grant")?;
    for _ in 0..20 {
        let resolved = driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Land Grant");
        if resolved {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => {
                assert!(
                    ways(&driver)?
                        .iter()
                        .any(|way| way.starts_with("Cast for Reveal your hand"))
                );
                choose_way(&mut driver, "Cast for Reveal your hand")?;
            }
            DecisionKind::PayMana(_) => {
                pay(&mut driver)?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from.iter().copied().take(1).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let state = driver.state();
    let revealed: Vec<Vec<String>> = driver.engine().events()[log..]
        .iter()
        .filter_map(|event| match event {
            Event::Revealed(shown) if shown.player == pid(0) => Some(
                shown
                    .cards
                    .iter()
                    .map(|id| state.object(*id).unwrap().card().name.clone())
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    // The spell is on the stack as its cost is paid (CR 601.2a): the rest of the hand is revealed at once, and an empty
    // one shows nothing.
    let mut shown: Vec<Vec<String>> = Vec::new();
    if !beside.is_empty() {
        shown.push(beside.iter().map(|name| (*name).to_owned()).collect());
    }
    shown.push(vec!["Forest".to_owned()]);
    assert_eq!(revealed, shown, "the hand at once, then the Forest found");
    let mut expected: Vec<String> = beside.iter().map(|name| (*name).to_owned()).collect();
    expected.push("Forest".to_owned());
    let mut found = r400_names_in_hand(&driver);
    found.sort();
    expected.sort();
    assert_eq!(found, expected);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
