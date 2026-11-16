//! Goal: "As this creature transforms into ~, ..." (`ReplaceWhen::R6603Transforms`, CR 701.28, 614.1c) runs its
//! effects as the permanent turns over, from the face it turns up only: Sephiroth's emblem is made once for each
//! transform into the back face, never for the transform back, and its trigger then drains an opponent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::definition::Layout;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const TURN: &str = r#"{"Activated":{"cost":[],"effects":[{"Transform":{"what":"This"}}]}}"#;

/// The emblem the back face makes as the permanent transforms into it (Sephiroth's Super Nova).
const SUPER_NOVA: &str = r#"{"Replacement":{"when":{"R6603Transforms":{"what":["This"]}},"by":{"AsEnters":[{"CreateEmblem":{"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}},{"GainLife":{"amount":1}}]}}]}}]}}}"#;

/// A transforming creature: the front turns over at will, the back has `back_rules` and turns back at will. With
/// `front_too` the front face also has the emblem ability, which applies only as the permanent turns the front up.
fn winged(front_too: bool) -> Arc<CardDefinition> {
    let rule = |text: &str| -> Ability { serde_json::from_str(text).unwrap() };
    let mut front = CardDefinition::new("Fabled Soldier")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3)
        .with_rules(rule(TURN));
    if front_too {
        front = front.with_rules(rule(SUPER_NOVA));
    }
    let back = CardDefinition::new("Winged Angel")
        .with_type(CardType::Creature)
        .with_power_toughness(5, 5)
        .with_rules(rule(TURN))
        .with_rules(rule(SUPER_NOVA));
    let mut whole = CardDefinition::new("Fabled Soldier")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3)
        .with_rules(rule(TURN));
    if front_too {
        whole = whole.with_rules(rule(SUPER_NOVA));
    }
    Arc::new(whole.with_faces(Layout::Transform, vec![front, back]))
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?;
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
                return Ok(());
            }
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

fn emblems(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Command(pid(0)))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind() == ObjectKind::Emblem)
        .count()
}

fn turn_over(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    settle(driver)
}

#[rstest]
#[case::only_the_back_face(false, [1, 1, 2])]
#[case::each_face_as_it_is_turned_up(true, [1, 2, 3])]
fn r6603_the_face_turned_up_makes_its_emblem(
    #[values(2, 3, 4)] seats: usize,
    #[case] front_too: bool,
    #[case] expected: [usize; 3],
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield(0, [ready(winged(front_too))]))?;
    assert_eq!(emblems(&driver), 0);
    turn_over(&mut driver, "Fabled Soldier")?;
    assert_eq!(emblems(&driver), expected[0], "into the back face");
    turn_over(&mut driver, "Winged Angel")?;
    assert_eq!(emblems(&driver), expected[1], "back to the front face");
    turn_over(&mut driver, "Fabled Soldier")?;
    assert_eq!(emblems(&driver), expected[2], "into the back face again");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r6603_the_emblem_drains_an_opponent_when_a_creature_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let smite = common::spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let bear = CardDefinition::new("Bear")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(winged(false))])
            .battlefield(1, [ready(Arc::new(bear))])
            .hand(0, [smite]),
    )?;
    turn_over(&mut driver, "Fabled Soldier")?;
    assert_eq!(emblems(&driver), 1);
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Smite", &[bear])?;
    settle(&mut driver)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let lost: i32 = (1..seats).map(|seat| 20 - life(seat)).sum();
    assert_eq!((life(0), lost), (21, 1));
    Ok(())
}
