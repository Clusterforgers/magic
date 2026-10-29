//! Goal: the reflexive follow-ups play as printed at tables of two, three and four seats: a land that is sacrificed and
//! then fetches a basic land, a creature that surveils and then draws, and an attacker that pays a
//! sacrifice and then grows and fights; the rules are the data the compiler reads from the printed sentences, and the
//! log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::GameObject;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "When this land enters, sacrifice it. When you do, search your library for a basic Mountain, Forest, or Plains card,
/// put it onto the battlefield tapped, then shuffle and you gain 1 life."
const W20B_LAND: &str = r#"{"Triggered": {"when": {"ZoneChange": {"to": "Battlefield", "what": ["This"]}}, "effects": [{"Sacrifice": {"filter": ["This"]}}, {"If": {"then": [{"Search": {"as": "land", "filter": [{"Supertype": "Basic"}, {"AnyOf": [{"Subtype": "Mountain"}, {"Subtype": "Forest"}, {"Subtype": "Plains"}]}]}}, {"Move": {"to": {"Battlefield": {"tapped": true}}, "what": {"Bound": "land"}}}, {"Shuffle": {}}, {"GainLife": {"amount": 1}}], "condition": "Performed"}}]}}"#;

/// "When this creature enters, surveil 2. When you do, draw a card."
const W20B_EATER: &str = r#"{"Triggered": {"when": {"ZoneChange": {"to": "Battlefield", "what": ["This"]}}, "effects": [{"Surveil": {"count": 2}}, {"Draw": {}}]}}"#;

/// "Whenever ~ attacks, you may sacrifice an artifact. When you do, put a +1/+1 counter on ~, then it fights up to one
/// target creature defending player controls."
const W20B_ACE: &str = r#"{"Triggered": {"when": {"Attacks": {"what": ["This"]}}, "effects": [{"Pay": {"cost": [{"Sacrifice": {"filter": [{"Type": "Artifact"}]}}], "paid": [{"PutCounters": {"on": "This", "counter": "+1/+1"}}, {"Fight": {"what": "This", "with": {"Target": 0}}}]}}], "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "Defending"}]}, "bounds": {"max": 1, "min": 0}}]}}"#;

/// A card of type `kind` with the ability the compiler reads from the sentence above.
fn w20b_card(name: &str, kind: CardType, json: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(json).unwrap();
    let card = CardDefinition::new(name)
        .with_type(kind)
        .with_rules(ability);
    Arc::new(if kind == CardType::Creature {
        card.with_mana_cost(cost(0, &[])).with_power_toughness(2, 2)
    } else {
        card
    })
}

fn w20b_named(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .min()
        .unwrap()
}

fn w20b_zone(driver: &Driver, name: &str) -> Zone {
    let id = w20b_named(driver, name);
    driver.state().object(id).unwrap().zone()
}

fn w20b_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers until the stack is empty: yes to every question, the named target, and as many cards as asked for at most.
fn w20b_settle(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => driver.answer(Answer::YesNo(YesNoChoice(true)))?,
            DecisionKind::ChooseCards(o) => driver.answer(Answer::ChooseCards(CardsChoice(
                o.from
                    .iter()
                    .copied()
                    .take(o.max.max(o.min) as usize)
                    .collect(),
            )))?,
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                let chosen = driver.target_named(target.unwrap())?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![chosen])))?
            }
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn w20b_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    })
}

#[rstest]
fn w20b_a_sacrificed_land_fetches_a_basic_land_and_gains_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20b_card("Reflex Land", CardType::Land, W20B_LAND)]);
    let mut driver = Driver::from_scenario(w20b_libraries(scenario, seats))?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.play_land("Reflex Land")?;
    w20b_settle(&mut driver, None)?;
    assert!(matches!(
        w20b_zone(&driver, "Reflex Land"),
        Zone::Graveyard(_)
    ));
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        1,
        "one Forest"
    );
    assert_eq!(driver.state().zone(Zone::Library(PlayerId(0))).len(), 11);
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life + 1);
    w20b_replays(&driver);
    Ok(())
}

#[rstest]
fn w20b_a_surveil_then_draws(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(
        0,
        [w20b_card("Reflex Eater", CardType::Creature, W20B_EATER)],
    );
    let mut driver = Driver::from_scenario(w20b_libraries(scenario, seats))?;
    driver.cast("Reflex Eater")?;
    w20b_settle(&mut driver, None)?;
    assert_eq!(w20b_zone(&driver, "Reflex Eater"), Zone::Battlefield);
    let state = driver.state();
    assert_eq!(
        state.zone(Zone::Graveyard(PlayerId(0))).len(),
        2,
        "both surveilled cards"
    );
    assert_eq!(
        state.zone(Zone::Hand(PlayerId(0))).len(),
        1,
        "the drawn card"
    );
    assert_eq!(state.zone(Zone::Library(PlayerId(0))).len(), 9);
    w20b_replays(&driver);
    Ok(())
}

#[rstest]
fn w20b_a_paid_sacrifice_then_grows_and_fights(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let ace = w20b_card("Reflex Ace", CardType::Creature, W20B_ACE);
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(ace), ready(relic)])
        .battlefield(seats - 1, [ready(free_creature("Wolf", 1, 1))]);
    let mut driver = Driver::from_scenario(w20b_libraries(scenario, seats))?;
    driver.attack(&[("Reflex Ace", seats - 1)])?;
    w20b_settle(&mut driver, Some("Wolf"))?;
    assert!(matches!(w20b_zone(&driver, "Relic"), Zone::Graveyard(_)));
    assert!(matches!(w20b_zone(&driver, "Wolf"), Zone::Graveyard(_)));
    assert_eq!(w20b_zone(&driver, "Reflex Ace"), Zone::Battlefield);
    let ace = w20b_named(&driver, "Reflex Ace");
    let counters = driver.state().object(ace).and_then(GameObject::permanent);
    assert_eq!(
        counters.map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne)),
        1
    );
    w20b_replays(&driver);
    Ok(())
}

/// With no creature to fight, the sacrifice is still paid and the counter still put (the target is "up to one").
#[rstest]
fn w20b_a_paid_sacrifice_with_no_creature_to_fight_still_grows(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let ace = w20b_card("Reflex Ace", CardType::Creature, W20B_ACE);
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(ace), ready(relic)]);
    let mut driver = Driver::from_scenario(w20b_libraries(scenario, seats))?;
    driver.attack(&[("Reflex Ace", seats - 1)])?;
    w20b_settle(&mut driver, None)?;
    assert!(matches!(w20b_zone(&driver, "Relic"), Zone::Graveyard(_)));
    let ace = w20b_named(&driver, "Reflex Ace");
    let counters = driver.state().object(ace).and_then(GameObject::permanent);
    assert_eq!(
        counters.map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne)),
        1
    );
    w20b_replays(&driver);
    Ok(())
}
