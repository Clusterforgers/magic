//! Goal: looking at a face-down creature (CR 708.5) lets the looker alone identify it, and only while it stays that
//! object; a face-down creature that was not looked at stays hidden, at tables of two to four seats, and the log replays
//! to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::object::FaceDown;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn ability(json: serde_json::Value) -> Ability {
    serde_json::from_value(json).unwrap()
}

/// "{T}: Look at target face-down creature."
fn teller() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Teller")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(serde_json::json!({"Activated": {
                "cost": ["Tap"],
                "targets": [{"what": {"Object": [{"Type": "Creature"}, "FaceDown"]}}],
                "effects": [{"LookAtFaceDown": {"what": {"Target": 0}}}]}}))),
    )
}

/// A free instant: "Target opponent manifests the top two cards of their library."
fn manifester() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Fog of Faces")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(serde_json::json!({"Spell": {
                "targets": [{"what": "Opponent"}],
                "effects": [{"Manifest": {"who": {"Target": 0}, "count": 2}}]}}))),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

fn face_down(driver: &Driver) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).unwrap().face_down() == Some(FaceDown::Manifest))
        .collect()
}

#[rstest]
fn the_looker_alone_identifies_the_face_down_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let game = || {
        (0..seats).fold(
            Scenario::new(seats)
                .at(1, 0, Step::Main1)
                .hand(0, [manifester()])
                .battlefield(0, [ready(teller())]),
            |scenario, seat| {
                scenario.library(
                    seat,
                    (0..6).map(|i| free_creature(&format!("Hidden {i}"), 2, 2)),
                )
            },
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let them = driver.target_player(1);
    driver.cast_at("Fog of Faces", &[them])?;
    settle(&mut driver)?;
    let made = face_down(&driver);
    assert_eq!(made.len(), 2);
    let (looked, other) = (made[0], made[1]);
    assert!(!driver.state().may_look_at(PlayerId(0), looked));
    driver.begin_activation("Teller")?;
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(looked).unwrap().reference());
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let events = driver.engine().events();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::FaceDownLooked { object, player } if *object == looked && *player == PlayerId(0)
    )));
    let state = driver.state();
    assert!(
        state.may_look_at(PlayerId(0), looked),
        "the looker identifies it"
    );
    assert!(
        state.may_look_at(PlayerId(1), looked),
        "its controller still does"
    );
    assert!(
        !state.may_look_at(PlayerId(0), other),
        "the other stays hidden"
    );
    for seat in 2..seats {
        assert!(
            !state.may_look_at(PlayerId::from_index(seat), looked),
            "seat {seat}"
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
