//! Goal: a manual action that takes the card of a cast, or the source of an activation, away from where it was announced rewinds the announcement without a trace, so the answers still on their way are refused and nothing that no longer applies reaches the log; a replay of the log reaches the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::manual::{CommonToken, ManualAction, ManualToken, ManualZone};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event};
use mtg_testkit::cards::{burn, creature, free_creature, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// What seat 0 has begun when the manual action comes.
#[derive(Debug, Clone, Copy)]
enum Begun {
    /// A Bolt cast from the hand, its target not chosen yet.
    Bolt,
    /// A Bolt cast from the hand, its target chosen and its mana not paid yet.
    BoltAtPayment,
    /// A creature cast from the hand, its mana not paid yet.
    Creature,
    /// The `{T}` ability of a Pinger on the battlefield, its target not chosen yet.
    Activation,
}

/// Where the subject of `begun` can be put by hand: a spell that is no permanent cannot go onto the battlefield, and a
/// permanent already there stays.
fn destinations(begun: Begun) -> Vec<ManualZone> {
    let elsewhere = vec![
        ManualZone::Graveyard,
        ManualZone::Exile,
        ManualZone::LibraryTop,
        ManualZone::LibraryBottom,
    ];
    match begun {
        Begun::Creature => [vec![ManualZone::Battlefield], elsewhere].concat(),
        _ => elsewhere,
    }
}

fn table() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [burn("Bolt", 3), creature("Ogre", 1, &[Color::Red], 3, 3)],
            )
            .library(0, vec![free_creature("Deep", 1, 1); 3])
            .library(1, vec![free_creature("Far", 1, 1); 3])
            .battlefield(
                0,
                [
                    ready(pinger("Pinger", 1)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, [ready(free_creature("Theirs", 3, 3))]),
    )
    .unwrap()
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The table with `begun` under way, and the object it is about.
fn start(begun: Begun) -> (Driver, ObjectId) {
    let mut driver = table();
    let hand = Zone::Hand(pid(0));
    let subject = match begun {
        Begun::Bolt => {
            driver.begin_cast("Bolt").unwrap();
            find(&driver, hand, "Bolt")
        }
        Begun::BoltAtPayment => {
            driver.begin_cast("Bolt").unwrap();
            let face = driver.target_player(1);
            driver.choose_targets(&[face]).unwrap();
            find(&driver, hand, "Bolt")
        }
        Begun::Creature => {
            driver.begin_cast("Ogre").unwrap();
            find(&driver, hand, "Ogre")
        }
        Begun::Activation => {
            driver.begin_activation("Pinger").unwrap();
            find(&driver, Zone::Battlefield, "Pinger")
        }
    };
    (driver, subject)
}

#[rstest]
fn a_cast_or_activation_whose_subject_was_moved_by_hand_is_rewound(
    #[values(Begun::Bolt, Begun::BoltAtPayment, Begun::Creature, Begun::Activation)] begun: Begun,
) {
    for to in destinations(begun) {
        let (mut driver, subject) = start(begun);
        let what = format!("{begun:?} moved to {to:?}");
        driver
            .manual_by(
                pid(0),
                &ManualAction::Move {
                    object: subject,
                    to,
                },
            )
            .unwrap_or_else(|e| panic!("{what}: {e:?}"));
        let decision = driver.decision().unwrap();
        assert_eq!(decision.player, pid(0), "{what}");
        assert!(
            matches!(decision.kind, DecisionKind::Priority(_)),
            "{what}: {:?}",
            decision.kind
        );
        assert_eq!(driver.state().zone(Zone::Stack), [], "{what}");
        let (hash, length) = (driver.engine().state_hash(), driver.engine().events().len());
        let face = driver.target_player(1);
        assert!(driver.choose_targets(&[face]).is_err(), "{what}");
        assert!(driver.pay_tapping(&["Mountain"], 0).is_err(), "{what}");
        assert_eq!(driver.engine().state_hash(), hash, "{what}");
        assert_eq!(driver.engine().events().len(), length, "{what}");
        let rebuilt = Engine::resume(
            driver.engine().setup().clone(),
            driver.engine().events().to_vec(),
        )
        .unwrap();
        assert_eq!(rebuilt.state_hash(), hash, "{what}");
    }
}

#[test]
fn a_cost_the_action_made_unpayable_rewinds_the_announcement_when_it_is_answered() {
    let (mut driver, pinger) = start(Begun::Activation);
    driver
        .manual_by(pid(0), &ManualAction::Tap { object: pinger })
        .unwrap();
    let face = driver.target_player(1);
    driver.choose_targets(&[face]).unwrap();
    let decision = driver.decision().unwrap();
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    assert_eq!(decision.player, pid(0));
    assert_eq!(driver.state().zone(Zone::Stack), []);
    let tapped = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::PermanentTapped { object } if *object == pinger))
        .count();
    assert_eq!(tapped, 1, "only the tap by hand");
    replays(&driver);
}

#[test]
fn a_payment_answered_after_the_action_tapped_the_source_rewinds_the_activation() {
    let mut driver = table();
    let make = ManualAction::CreateToken {
        token: ManualToken::Common(CommonToken::Food),
        count: 1,
    };
    driver.manual_by(pid(0), &make).unwrap();
    driver.begin_activation("Food").unwrap();
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::PayMana(_)
    ));
    let food = find(&driver, Zone::Battlefield, "Food");
    driver
        .manual_by(pid(1), &ManualAction::Tap { object: food })
        .unwrap();
    driver.pay_tapping(&["Mountain", "Mountain"], 0).unwrap();
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
    assert_eq!(driver.state().zone(Zone::Stack), []);
    assert_eq!(find(&driver, Zone::Battlefield, "Food"), food);
    replays(&driver);
}

#[test]
fn an_action_on_another_object_leaves_the_cast_alone() {
    let (mut driver, _) = start(Begun::BoltAtPayment);
    driver
        .manual_by(
            pid(1),
            &ManualAction::Life {
                player: pid(1),
                delta: -2,
            },
        )
        .unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    replays(&driver);
}

/// A rebuilt engine reaches the state the live one is in.
fn replays(driver: &Driver) {
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
}
