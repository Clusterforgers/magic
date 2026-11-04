//! Goal: the manual actions that turn a double-faced permanent over, put a double-faced card onto the battlefield back face up, and give a creature a combat permission until end of turn each do what the card text says through the layers, refuse what is not theirs, and replay from the log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const DELVER: &str = "Delver of Secrets // Insectile Aberration";

fn refused(result: Result<Vec<Event>, ScriptError>) -> ManualRefusal {
    match result {
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal)))) => refusal,
        other => panic!("not refused by the manual check: {other:?}"),
    }
}

fn shown(driver: &Driver, id: ObjectId) -> (String, i32) {
    let power = characteristics(driver.state(), id).unwrap().power.unwrap();
    (name_of(driver.state(), id), power)
}

fn table() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 8])
            .library(1, vec![forest(); 8])
            .hand_named(0, [DELVER])
            .battlefield(
                0,
                [
                    ready(card(DELVER).unwrap()),
                    ready(free_creature("Plain", 2, 2)),
                ],
            )
            .battlefield(1, [ready(free_creature("Theirs", 2, 2))]),
    )
    .unwrap()
}

fn in_hand(driver: &Driver) -> ObjectId {
    *driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .find(|id| name_of(driver.state(), **id).starts_with("Delver"))
        .unwrap()
}

#[rstest]
#[case::turns_to_the_back(1, ("Insectile Aberration", 3))]
#[case::turns_back_to_the_front(2, ("Delver of Secrets", 1))]
fn transform_turns_the_permanent_over(#[case] times: usize, #[case] want: (&str, i32)) {
    let mut driver = table();
    let delver = driver.id_of("Delver of Secrets").unwrap();
    for _ in 0..times {
        let events = driver
            .manual(ManualAction::Transform { object: delver })
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Transformed { .. }))
        );
    }
    let (name, power) = shown(&driver, delver);
    assert_eq!((name.as_str(), power), want);
}

#[test]
fn transform_refuses_what_is_not_a_transforming_permanent_of_the_seat() {
    let mut driver = table();
    let delver = driver.id_of("Delver of Secrets").unwrap();
    let plain = driver.id_of("Plain").unwrap();
    let theirs = driver.id_of("Theirs").unwrap();
    let hand = in_hand(&driver);
    for (object, refusal) in [
        (plain, ManualRefusal::NotTransformable(plain)),
        (theirs, ManualRefusal::NotYours),
        (hand, ManualRefusal::NotTransformable(hand)),
    ] {
        let got = refused(driver.manual(ManualAction::Transform { object }));
        assert_eq!(got, refusal);
    }
    assert_eq!(shown(&driver, delver).1, 1);
}

#[test]
fn move_back_puts_a_double_faced_card_onto_the_battlefield_back_face_up() {
    let mut driver = table();
    let card = in_hand(&driver);
    driver
        .manual(ManualAction::MoveBack { object: card })
        .unwrap();
    let (name, power) = shown(&driver, card);
    assert_eq!((name.as_str(), power), ("Insectile Aberration", 3));
    assert_eq!(
        driver.state().object(card).unwrap().zone(),
        Zone::Battlefield
    );
}

/// A card crafted by hand: exiled, then returned back face up (Spring-Loaded Sawblades).
#[test]
fn a_crafted_card_reaches_its_back_face_through_exile() {
    let mut driver = table();
    let delver = driver.id_of("Delver of Secrets").unwrap();
    driver
        .manual(ManualAction::Move {
            object: delver,
            to: ManualZone::Exile,
        })
        .unwrap();
    driver
        .manual(ManualAction::MoveBack { object: delver })
        .unwrap();
    assert_eq!(shown(&driver, delver).0, "Insectile Aberration");
}

/// Passes priority to the declare-attackers decision of `turn`.
fn to_attackers(driver: &mut Driver, turn: u32) {
    driver.idle_until(turn, Step::BeginCombat).unwrap();
    while matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
        driver.pass().unwrap();
    }
}

fn combat_table() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .battlefield(
                0,
                [
                    ready(keyworded("Mobile Fort", 3, 5, &[Keyword::Defender])),
                    ready(free_creature("A1", 2, 2)),
                    ready(free_creature("A2", 2, 2)),
                    ready(forest()),
                ],
            )
            .battlefield(1, [ready(free_creature("Kemba's Legion", 3, 3))]),
    )
    .unwrap()
}

#[rstest]
#[case::with_the_action(true)]
#[case::without_it(false)]
fn a_defender_attacks_only_after_the_action(#[case] acts: bool) {
    let mut driver = combat_table();
    let fort = driver.id_of("Mobile Fort").unwrap();
    if acts {
        let events = driver
            .manual(ManualAction::AttackDespiteDefender { object: fort })
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::RuleCreated { .. }))
        );
    }
    to_attackers(&mut driver, 1);
    assert_eq!(driver.attack(&[("Mobile Fort", 1)]).is_ok(), acts);
}

#[test]
fn the_attack_permission_ends_with_the_turn() {
    let mut driver = combat_table();
    let fort = driver.id_of("Mobile Fort").unwrap();
    driver
        .manual(ManualAction::AttackDespiteDefender { object: fort })
        .unwrap();
    to_attackers(&mut driver, 3);
    assert!(driver.attack(&[("Mobile Fort", 1)]).is_err());
}

#[rstest]
#[case::one_more(1, true)]
#[case::none(0, false)]
fn a_blocker_blocks_extra_creatures_after_the_action(#[case] extra: u32, #[case] works: bool) {
    let mut driver = combat_table();
    let legion = driver.id_of("Kemba's Legion").unwrap();
    if extra > 0 {
        let action = ManualAction::BlockAdditional {
            object: legion,
            count: extra,
        };
        driver.manual_by(pid(1), &action).unwrap();
    }
    to_attackers(&mut driver, 1);
    driver.attack(&[("A1", 1), ("A2", 1)]).unwrap();
    while !matches!(
        driver.decision().unwrap().kind,
        DecisionKind::DeclareBlockers(_)
    ) {
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    let blocked = driver.block(&[("Kemba's Legion", "A1"), ("Kemba's Legion", "A2")]);
    assert_eq!(blocked.is_ok(), works);
}

#[rstest]
#[case::zero(0)]
#[case::too_many(100)]
fn extra_blocks_are_limited_to_one_to_ninety_nine(#[case] count: u32) {
    let mut driver = combat_table();
    let object = driver.id_of("Kemba's Legion").unwrap();
    let action = ManualAction::BlockAdditional { object, count };
    let got = refused(driver.manual_by(pid(1), &action));
    assert_eq!(got, ManualRefusal::OutOfRange("the number of extra blocks"));
}

#[rstest]
#[case::a_land("Forest", false)]
#[case::an_opponents_creature("Kemba's Legion", true)]
fn combat_permissions_refuse_what_is_not_the_seats_creature(
    #[case] name: &str,
    #[case] theirs: bool,
    #[values(true, false)] attack: bool,
) {
    let mut driver = combat_table();
    let object = driver.id_of(name).unwrap();
    let action = if attack {
        ManualAction::AttackDespiteDefender { object }
    } else {
        ManualAction::BlockAdditional { object, count: 1 }
    };
    let want = if theirs {
        ManualRefusal::NotYours
    } else {
        ManualRefusal::NotACreature(object)
    };
    assert_eq!(refused(driver.manual(action)), want);
}

#[test]
fn move_back_refuses_a_single_faced_card_and_a_permanent() {
    let mut driver = table();
    let plain = driver.id_of("Plain").unwrap();
    let delver = driver.id_of("Delver of Secrets").unwrap();
    for object in [plain, delver] {
        assert_eq!(
            refused(driver.manual(ManualAction::MoveBack { object })),
            ManualRefusal::NotDoubleFaced(object)
        );
    }
}
