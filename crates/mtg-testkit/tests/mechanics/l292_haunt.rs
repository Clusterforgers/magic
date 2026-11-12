//! Goal: a haunt creature's "When ~ enters or the creature it haunts dies" works for both events at tables of two to four seats: once as the card enters, once from exile as the creature it haunts dies, never for another creature's death, and an enters-only ability does not wait in exile.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::fight_on;

/// "When ~ enters or the creature it haunts dies, you gain 3 life."
const BOTH: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"GainLife":{"amount":3}}]}}"#;
/// "When ~ enters, you gain 3 life."
const ENTERS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":3}}]}}"#;
const SLAY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

fn card(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        def = def.with_power_toughness(2, 2);
    }
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    def.rules = rules
        .iter()
        .map(|json| Arc::new(serde_json::from_str::<Ability>(json).unwrap()))
        .collect();
    Arc::new(def)
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn slay(driver: &mut Driver, spell: &str, at: &str) -> Result<(), ScriptError> {
    let id = driver.id_of(at)?;
    let named = driver.state().object(id).unwrap().reference();
    driver.cast_at(spell, &[Target::Object(named)])?;
    fight_on(driver, Step::Main1)
}

fn in_exile(driver: &Driver, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
}

/// Seat 0 casts the haunt creature, kills it, then kills the creature it does not haunt and the one it haunts; the life
/// gained at each of the three steps.
fn gains(seats: usize, rules: &[&str]) -> Result<[i32; 3], ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            seats - 1,
            vec![
                ready(free_creature("Victim", 1, 1)),
                ready(free_creature("Bystander", 1, 1)),
            ],
        )
        .hand(
            0,
            vec![
                card("Spirit", CardType::Creature, rules),
                card("Slay", CardType::Sorcery, &[SLAY]),
                card("Smite", CardType::Sorcery, &[SLAY]),
                card("Strike", CardType::Sorcery, &[SLAY]),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let start = life(&driver);
    driver.cast("Spirit")?;
    fight_on(&mut driver, Step::Main1)?;
    let entered = life(&driver);
    slay(&mut driver, "Slay", "Spirit")?;
    let card = in_exile(&driver, "Spirit").expect("the card is exiled haunting");
    let haunted = driver.state().object(card).unwrap().haunting().unwrap();
    let haunted = driver
        .state()
        .object(haunted.id)
        .unwrap()
        .card()
        .name
        .clone();
    let other = if haunted == "Victim" {
        "Bystander"
    } else {
        "Victim"
    };
    let before = life(&driver);
    slay(&mut driver, "Smite", other)?;
    let after_other = life(&driver);
    slay(&mut driver, "Strike", &haunted)?;
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok([
        entered - start,
        after_other - before,
        life(&driver) - after_other,
    ])
}

#[rstest]
fn both_events_trigger_the_one_ability(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    assert_eq!(gains(seats, &[r#"{"Keyword":"Haunt"}"#, BOTH])?, [3, 0, 3]);
    Ok(())
}

#[rstest]
fn an_enters_only_ability_does_not_wait_in_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert_eq!(
        gains(seats, &[r#"{"Keyword":"Haunt"}"#, ENTERS])?,
        [3, 0, 0]
    );
    Ok(())
}
