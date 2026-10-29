//! Goal: lines that open with a flavour name play as the ability after the name, at tables of two, three and four seats: a creature put onto the battlefield from a graveyard draws and loses life (or exiles a permanent) as "enters from a graveyard" says and does nothing when cast from a hand, a legend whose second name holds its own name makes a token when it hurts a player, and each log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::GameObject;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The first ability the compiler reads from a card of this type whose rules are this text.
fn t16a_read(name: &str, type_line: &str, text: &str) -> Ability {
    let face = PrintedFace {
        name: name.to_owned(),
        type_line: type_line.to_owned(),
        oracle_text: text.to_owned(),
        ..PrintedFace::default()
    };
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789abc".to_owned(),
        name: name.to_owned(),
        faces: vec![face],
        ..PrintedCard::default()
    };
    let card = compile(&printed).card.expect("the card compiles");
    assert_eq!(card.faces[0].abilities.len(), 1, "{text}");
    card.faces[0].abilities[0].clone()
}

const T16A_RETURN: &str = "Return target creature card from your graveyard to the battlefield.";
const T16A_CODES: &str = "Dynastic Codes \u{2014} When this creature enters from a graveyard, you draw two cards and you lose 2 life.";
const T16A_CANNON: &str = "Exile Cannon \u{2014} When this creature enters from a graveyard, exile up to one target nonland permanent.";
const T16A_SQUIRRELS: &str = "Do You Like Squirrels? \u{2014} Whenever The Unbeatable Squirrel Girl enters or attacks, create a 1/1 green Squirrel creature token.";
const T16A_BLADE: &str = "Blade of Magnus \u{2014} Whenever Magnus the Red deals combat damage to a player, create a 3/3 red Spawn creature token.";

/// A free sorcery that returns a creature card from its owner's graveyard.
fn t16a_grave_call() -> Arc<CardDefinition> {
    let rules = t16a_read("Grave Call", "Sorcery", T16A_RETURN);
    Arc::new(
        CardDefinition::new("Grave Call")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn t16a_named(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .min()
        .unwrap()
}

/// The card called `name` in whatever zone it is in, as a target.
fn t16a_target(driver: &Driver, name: &str) -> Target {
    let id = t16a_named(driver, name);
    Target::Object(driver.state().object(id).unwrap().reference())
}

fn t16a_zone(driver: &Driver, name: &str) -> Zone {
    let id = t16a_named(driver, name);
    driver.state().object(id).unwrap().zone()
}

fn t16a_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers until the stack is empty: yes to every question and the named target.
fn t16a_settle(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
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
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                let chosen = driver.target_named(target.unwrap())?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![chosen])))?
            }
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn t16a_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    })
}

fn t16a_praetorian() -> Arc<CardDefinition> {
    creature_with(
        "Praetorian",
        vec![t16a_read(
            "Praetorian",
            "Creature \u{2014} Necron",
            T16A_CODES,
        )],
    )
}

#[rstest]
fn t16a_a_creature_returned_from_a_graveyard_draws_and_loses_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [t16a_grave_call()])
        .graveyard(0, [t16a_praetorian()]);
    let mut driver = Driver::from_scenario(t16a_libraries(scenario, seats))?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    let target = t16a_target(&driver, "Praetorian");
    driver.cast_at("Grave Call", &[target])?;
    t16a_settle(&mut driver, None)?;
    assert_eq!(t16a_zone(&driver, "Praetorian"), Zone::Battlefield);
    assert_eq!(driver.state().zone(Zone::Hand(PlayerId(0))).len(), 2);
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life - 2);
    t16a_replays(&driver);
    Ok(())
}

#[rstest]
fn t16a_a_creature_cast_from_a_hand_does_not_trigger(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [t16a_praetorian()]);
    let mut driver = Driver::from_scenario(t16a_libraries(scenario, seats))?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.cast("Praetorian")?;
    t16a_settle(&mut driver, None)?;
    assert_eq!(t16a_zone(&driver, "Praetorian"), Zone::Battlefield);
    assert_eq!(driver.state().zone(Zone::Hand(PlayerId(0))).len(), 0);
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life);
    t16a_replays(&driver);
    Ok(())
}

#[rstest]
fn t16a_a_creature_returned_from_a_graveyard_exiles_a_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let sentinel = creature_with(
        "Sentinel",
        vec![t16a_read(
            "Sentinel",
            "Artifact Creature \u{2014} Insect",
            T16A_CANNON,
        )],
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [t16a_grave_call()])
        .graveyard(0, [sentinel])
        .battlefield(seats - 1, [ready(free_creature("Wolf", 1, 1))]);
    let mut driver = Driver::from_scenario(t16a_libraries(scenario, seats))?;
    let target = t16a_target(&driver, "Sentinel");
    driver.cast_at("Grave Call", &[target])?;
    t16a_settle(&mut driver, Some("Wolf"))?;
    assert_eq!(t16a_zone(&driver, "Sentinel"), Zone::Battlefield);
    assert!(matches!(t16a_zone(&driver, "Wolf"), Zone::Exile));
    t16a_replays(&driver);
    Ok(())
}

#[rstest]
fn t16a_a_flavour_name_with_a_question_mark_does_not_stop_the_token(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let girl = Arc::new(
        CardDefinition::new("The Unbeatable Squirrel Girl")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(2, 2)
            .with_rules(t16a_read(
                "The Unbeatable Squirrel Girl",
                "Legendary Creature \u{2014} Human Squirrel Hero",
                T16A_SQUIRRELS,
            )),
    );
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, [girl]);
    let mut driver = Driver::from_scenario(t16a_libraries(scenario, seats))?;
    driver.cast("The Unbeatable Squirrel Girl")?;
    t16a_settle(&mut driver, None)?;
    let squirrel = t16a_named(&driver, "Squirrel");
    assert_eq!(
        driver.state().object(squirrel).unwrap().zone(),
        Zone::Battlefield
    );
    t16a_replays(&driver);
    Ok(())
}

#[rstest]
fn t16a_a_legend_with_its_own_name_in_a_flavour_name_makes_a_token(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let magnus = Arc::new(
        CardDefinition::new("Magnus the Red")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(2, 2)
            .with_rules(t16a_read(
                "Magnus the Red",
                "Legendary Creature \u{2014} Demon Primarch",
                T16A_BLADE,
            )),
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(magnus)]);
    let mut driver = Driver::from_scenario(t16a_libraries(scenario, seats))?;
    let life = driver
        .state()
        .player(PlayerId(u64::try_from(seats - 1).unwrap()))
        .unwrap()
        .life();
    driver.attack(&[("Magnus the Red", seats - 1)])?;
    driver.pass_until(Step::Main2)?;
    t16a_settle(&mut driver, None)?;
    assert_eq!(
        driver
            .state()
            .player(PlayerId(u64::try_from(seats - 1).unwrap()))
            .unwrap()
            .life(),
        life - 2
    );
    let spawn = t16a_named(&driver, "Spawn");
    assert_eq!(
        driver.state().object(spawn).unwrap().zone(),
        Zone::Battlefield
    );
    t16a_replays(&driver);
    Ok(())
}
