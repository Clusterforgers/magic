//! Goal: "whenever an artifact becomes tapped or a player activates an artifact's ability without {T} in its activation cost" plays at tables of two to four seats: an ability with {T} triggers once as its artifact becomes tapped, an ability without {T} triggers on its activation, a mana ability included, and the opponent-only version ignores its controller's own artifacts; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with "{T}: Add {C}.", "Sacrifice this artifact: Add {C}.", "{0}: You gain 1 life." and "{T}: You gain 1
/// life.", in that order.
fn trinket() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
        r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
        r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
        r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#,
    ];
    let mut definition = CardDefinition::new("Trinket").with_type(CardType::Artifact);
    for rule in rules {
        definition = definition.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(definition)
}

/// Passes priority and answers passively until the stack and the queue are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

/// Activates the trinket's ability with this index: the mana abilities straight from priority.
fn activate(driver: &mut Driver, ability: usize) -> Result<(), ScriptError> {
    if ability < 2 {
        let trinket = driver.id_of("Trinket")?;
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority");
        };
        let Some(source) = options
            .mana_abilities
            .iter()
            .find(|m| m.source.object == trinket && m.tapless == (ability == 1))
            .map(|m| m.source)
        else {
            panic!("the mana ability is offered: {:?}", options.mana_abilities);
        };
        driver.answer(Answer::Priority(PriorityAction::ActivateMana(source)))?;
    } else {
        driver.begin_ability("Trinket", ability)?;
    }
    settle(driver)
}

#[rstest]
#[case::tapped_for_mana(0)]
#[case::mana_without_tap(1)]
#[case::without_tap(2)]
#[case::tapped(3)]
fn each_use_of_an_artifact_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] ability: usize,
) -> Result<(), ScriptError> {
    let wind = ready(card("Haunting Wind").unwrap());
    let leech = ready(card("Powerleech").unwrap());
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(trinket())]);
    scenario = if seats == 2 {
        scenario.battlefield(1, [wind, leech])
    } else {
        scenario
            .battlefield(1, [wind])
            .battlefield(seats - 1, [leech])
    };
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let gained = u32::from(ability >= 2);
    activate(&mut driver, ability)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(
        i64::from(life(0)),
        20 - 1 + i64::from(gained),
        "Haunting Wind deals 1 to the artifact's controller, once"
    );
    assert_eq!(
        life(seats - 1),
        21,
        "Powerleech gains 1 for an opponent's artifact"
    );
    for seat in 1..seats - 1 {
        assert_eq!(life(seat), 20);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn powerleech_ignores_its_controllers_own_artifacts(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] ability: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(trinket()), ready(card("Powerleech").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    activate(&mut driver, ability)?;
    let gained = i32::from(ability >= 2);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20 + gained);
    Ok(())
}

/// An Elemental creature with "{T}: Add {C}." and "{0}: You gain 1 life."
fn elemental() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
        r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
    ];
    let mut definition = CardDefinition::new("Trinket")
        .with_type(CardType::Creature)
        .with_subtype("Elemental")
        .with_power_toughness(1, 1);
    for rule in rules {
        definition = definition.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(definition)
}

#[rstest]
#[case::mana_ability(0)]
#[case::other_ability(2)]
fn searblades_grow_for_every_ability_of_an_elemental(
    #[values(2, 3, 4)] seats: usize,
    #[case] ability: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(elemental()),
            ready(card("Ceaseless Searblades").unwrap()),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    // The elemental's second ability is its index 1; `activate` reads 2 as "the {0} ability" of the trinket.
    if ability == 0 {
        activate(&mut driver, 0)?;
    } else {
        driver.begin_ability("Trinket", 1)?;
        settle(&mut driver)?;
    }
    let blades = driver.id_of("Ceaseless Searblades")?;
    let power = mtg_engine::characteristics::characteristics(driver.state(), blades)
        .unwrap()
        .power;
    let printed = card("Ceaseless Searblades").unwrap().power.unwrap_or(0);
    assert_eq!(power, Some(printed + 1));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
