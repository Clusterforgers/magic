//! Goal: the open predicate atoms decide whole plays at tables of two, three and four seats: a sweep of modified creatures takes only the creatures with a counter, a recall returns only the creature cards put into its caster's graveyard from the battlefield this turn, a draw waits for a card to have left the graveyard; a face-down creature is found by `FaceDown`, power is compared with a count of lands, and a counterspell takes only a spell that targets its caster; every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::object::GameObject;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// A free instant with the rules of the JSON.
fn w21a_spell(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

fn w21a_named(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .min()
        .unwrap()
}

fn w21a_zone(driver: &Driver, name: &str) -> Zone {
    let id = w21a_named(driver, name);
    driver.state().object(id).unwrap().zone()
}

fn w21a_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn w21a_hand(driver: &Driver) -> usize {
    let me = driver.state().players()[0].id();
    driver.state().zone(Zone::Hand(me)).len()
}

/// Passes until the stack is empty.
fn w21a_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn w21a_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    })
}

const W21A_PURGE: &str =
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Modified"]}}}]}}"#;
const W21A_RECALL: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},"GravedThisTurn"]},"to":"Hand"}}]}}"#;
const W21A_INSIGHT: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"CardsLeftGraveyard"}},"op":"Ge","right":1}},"then":[{"Draw":{}}]}}]}}"#;

#[rstest]
fn w21a_modified_graved_and_left_graveyard_decide_a_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let counter = mtg_cards::value::CounterKind::PlusOnePlusOne;
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [
                w21a_spell("Insight", W21A_INSIGHT),
                w21a_spell("Purge", W21A_PURGE),
                w21a_spell("Recall", W21A_RECALL),
                w21a_spell("Second Insight", W21A_INSIGHT),
            ],
        )
        .battlefield(
            0,
            [
                with_counters(ready(free_creature("Bear", 2, 2)), counter.clone(), 1),
                ready(free_creature("Wolf", 2, 2)),
            ],
        )
        .graveyard_named(0, ["Grizzly Bears"])
        .battlefield(
            seats - 1,
            [with_counters(
                ready(free_creature("Ogre", 3, 3)),
                counter,
                1,
            )],
        );
    let mut driver = Driver::from_scenario(w21a_libraries(scenario, seats))?;
    let before = w21a_hand(&driver);
    driver.cast("Insight")?;
    w21a_resolve(&mut driver)?;
    assert_eq!(
        w21a_hand(&driver),
        before - 1,
        "no card left a graveyard yet"
    );
    driver.cast("Purge")?;
    w21a_resolve(&mut driver)?;
    assert!(matches!(w21a_zone(&driver, "Bear"), Zone::Graveyard(_)));
    assert!(matches!(w21a_zone(&driver, "Ogre"), Zone::Graveyard(_)));
    assert_eq!(w21a_zone(&driver, "Wolf"), Zone::Battlefield, "no counter");
    driver.cast("Recall")?;
    w21a_resolve(&mut driver)?;
    assert!(matches!(w21a_zone(&driver, "Bear"), Zone::Hand(_)));
    assert!(
        matches!(w21a_zone(&driver, "Grizzly Bears"), Zone::Graveyard(_)),
        "there since before the turn"
    );
    assert!(
        matches!(w21a_zone(&driver, "Ogre"), Zone::Graveyard(_)),
        "another owner's graveyard"
    );
    let before = w21a_hand(&driver);
    driver.cast("Second Insight")?;
    w21a_resolve(&mut driver)?;
    assert_eq!(w21a_hand(&driver), before, "cast one, drew one");
    w21a_replays(&driver);
    Ok(())
}

const W21A_VEIL: &str = r#"{"Spell":{"effects":[{"Manifest":{}}]}}"#;
const W21A_UNMASK: &str =
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"FaceDown"]}}}]}}"#;
const W21A_MEASURE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}}]}}"#;
const W21A_JAB: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;
const W21A_WARD: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"TargetsPlayer":"You"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#;

#[rstest]
fn w21a_face_down_compared_power_and_a_spell_at_you_decide_a_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [
                w21a_spell("Veil", W21A_VEIL),
                w21a_spell("Unmask", W21A_UNMASK),
                w21a_spell("Measure", W21A_MEASURE),
                w21a_spell("Jab", W21A_JAB),
                w21a_spell("Ward Off", W21A_WARD),
            ],
        )
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(free_creature("Elf", 1, 1)),
            ],
        )
        .battlefield(seats - 1, [ready(free_creature("Ogre", 3, 3))]);
    let mut driver = Driver::from_scenario(w21a_libraries(scenario, seats))?;
    driver.cast("Veil")?;
    w21a_resolve(&mut driver)?;
    let face_down = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|object| object.face_down().is_some())
        })
        .count();
    assert_eq!(face_down, 1, "the manifested card");
    driver.cast("Unmask")?;
    w21a_resolve(&mut driver)?;
    let face_down_left = driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.face_down().is_some())
    });
    assert!(!face_down_left, "the face-down creature is destroyed");
    assert_eq!(w21a_zone(&driver, "Elf"), Zone::Battlefield);
    driver.cast("Measure")?;
    w21a_resolve(&mut driver)?;
    assert!(
        matches!(w21a_zone(&driver, "Ogre"), Zone::Graveyard(_)),
        "power 3, three lands"
    );
    assert_eq!(w21a_zone(&driver, "Elf"), Zone::Battlefield, "power 1");
    let me = driver.target_player(0);
    let life = driver.state().players()[0].life();
    driver.cast_at("Jab", &[me])?;
    let jab = driver.target_named("Jab")?;
    driver.cast_at("Ward Off", &[jab])?;
    w21a_resolve(&mut driver)?;
    assert_eq!(
        driver.state().players()[0].life(),
        life,
        "the Jab is countered"
    );
    assert!(matches!(w21a_zone(&driver, "Jab"), Zone::Graveyard(_)));
    w21a_replays(&driver);
    Ok(())
}
