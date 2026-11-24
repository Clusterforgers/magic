//! Goal: Circling Vultures may be discarded from its owner's hand whenever they hold priority, on any turn and with a
//! spell on the stack, as a special action that uses no stack and counts as a discard; it is offered to no other player;
//! its upkeep trigger, and Barrow Ghoul's, exiles the top creature card of the graveyard or sacrifices it; at two to four seats, and each game
//! replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn vultures_in(driver: &Driver, zone: Zone) -> Option<ObjectId> {
    driver.state().zone(zone).iter().copied().find(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == "Circling Vultures")
    })
}

#[rstest]
fn circling_vultures_is_discarded_as_a_special_action(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] active: usize,
) {
    let scenario = Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(0, [ready(island())])
        .hand(0, [card("Circling Vultures").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // Seat 0 gets priority on its own turn first, or once the active player passes.
    for _ in 0..10 {
        let decision = driver.decision().unwrap();
        if decision.player == PlayerId(0) {
            break;
        }
        driver.pass().unwrap();
    }
    let vultures = vultures_in(&driver, Zone::Hand(PlayerId(0))).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    assert_eq!(options.r120_discardable, vec![vultures]);
    let stack = driver.state().stack().len();
    driver
        .answer(Answer::Priority(PriorityAction::R120Discard(vultures)))
        .unwrap();
    assert_eq!(driver.state().stack().len(), stack, "no stack");
    assert!(vultures_in(&driver, Zone::Graveyard(PlayerId(0))).is_some());
    assert!(driver.engine().events().iter().any(|event| matches!(
        event,
        mtg_engine::Event::CardDiscarded { player, object } if *player == PlayerId(0) && *object == vultures
    )));
    replays(&driver);
}

#[rstest]
fn circling_vultures_is_offered_to_its_owner_only(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .hand(0, [card("Circling Vultures").unwrap()]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, PlayerId(1));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("priority was expected");
    };
    assert_eq!(options.r120_discardable, Vec::<ObjectId>::new());
    let refused = Answer::Priority(PriorityAction::R120Discard(
        vultures_in(&driver, Zone::Hand(PlayerId(0))).unwrap(),
    ));
    let mut driver = driver;
    assert!(driver.answer(refused).is_err());
}

#[rstest]
#[case::exiles_the_top_creature(true)]
#[case::sacrificed_without_one(false)]
fn the_upkeep_exiles_the_top_creature_card_or_sacrifices(
    #[values(2, 3, 4)] seats: usize,
    #[values("Circling Vultures", "Barrow Ghoul")] name: &str,
    #[case] creature_card: bool,
) {
    let mut scenario = Scenario::new(seats).battlefield(0, [ready(card(name).unwrap())]);
    if creature_card {
        scenario = scenario.graveyard(0, [free_creature("Bones", 1, 1), island()]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    for _ in 0..60 {
        match &driver.decision().unwrap().kind {
            DecisionKind::YesNo(options) => {
                let yes = !options.only_no;
                driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let kept = driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    });
    assert_eq!(kept, creature_card);
    if creature_card {
        let exiled = driver.state().zone(Zone::Exile);
        assert_eq!(exiled.len(), 1);
        assert_eq!(
            driver.state().object(exiled[0]).unwrap().card().name,
            "Bones"
        );
    }
    replays(&driver);
}
