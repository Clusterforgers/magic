//! Goal: static abilities and effects that change abilities, control and restrictions change what the game lets players do, whatever the modification: a granted activated ability is offered, activated and resolved like a printed one and ends with its grant; a granted triggered ability, and one a token made in play prints, trigger in a game where no card began with one; attack and block restrictions hold back attackers and blockers; a creature controlled through a static ability attacks for its new controller once they have had it since their turn began; an anthem and Humility change the damage combat deals; and the log of every such game replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::EffectId;
use mtg_core::mana::ManaCost;
use mtg_engine::abilities::{AbilitySource, Grant};
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact whose only ability is `json`.
fn totem(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability(json)),
    )
}

/// A 2/2 that has `json` as its only ability.
fn bearing(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(json)),
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn offered(driver: &Driver) -> usize {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.abilities.len(),
        other => panic!("priority was expected, got {}", other.name()),
    }
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
}

const DRAW_FOR_TAP: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;
const GAIN_ON_ATTACK: &str =
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#;

#[test]
fn a_granted_activated_ability_is_offered_activated_and_resolved_like_a_printed_one()
-> Result<(), ScriptError> {
    let grant = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Named":"Bear"}}]}},"mods":[{{"GrantAbility":{DRAW_FOR_TAP}}}]}}}}"#
    );
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(totem("Totem", &grant)),
            ready(free_creature("Bear", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        offered(&driver),
        1,
        "the Bear has no ability of its own and is offered the one it was given"
    );
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let events = driver.begin_activation("Bear")?;
    let given = events
        .iter()
        .find_map(|event| match event {
            Event::AbilityActivated { origin, .. } => Some(*origin),
            _ => None,
        })
        .expect("the activation is logged");
    assert!(
        matches!(given, AbilitySource::Granted(Grant::Static { .. })),
        "and logged under the address of the grant: {given:?}"
    );
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1,
        "it drew a card"
    );
    assert_eq!(offered(&driver), 0, "the Bear is tapped");
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn an_ability_an_effect_grants_ends_with_the_effect() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(free_creature("Bear", 2, 2))]),
    )?;
    assert_eq!(offered(&driver), 0);
    let bear = driver.id_of("Bear")?;
    driver.add_effect(
        driver.on(bear),
        Modification::GrantAbility(Arc::new(ability(DRAW_FOR_TAP))),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(offered(&driver), 1);
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(
        offered(&driver),
        0,
        "the effect lasted until the end of the turn it began in"
    );
    Ok(())
}

#[test]
fn a_triggered_ability_granted_in_a_game_where_nothing_triggered_triggers()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2).battlefield(0, [ready(free_creature("Bear", 2, 2))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.id_of("Bear")?;
    driver.add_effect(
        driver.on(bear),
        Modification::GrantAbility(Arc::new(ability(GAIN_ON_ATTACK))),
        Duration::UntilEndOfTurn,
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    let events = driver.attack(&[("Bear", 1)])?;
    let triggered = events.iter().find_map(|event| match event {
        Event::AbilityTriggered(trigger) => Some(trigger.origin),
        _ => None,
    });
    assert_eq!(
        triggered,
        Some(AbilitySource::Granted(Grant::Effect(EffectId(0)))),
        "it triggered under the address of the effect"
    );
    resolve_the_stack(&mut driver)?;
    assert_eq!(life(&driver, 0), 21, "it resolved like a printed one");
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::in_the_order_they_triggered(&[0, 1])]
#[case::the_other_way_round(&[1, 0])]
fn two_abilities_given_to_one_creature_by_two_statics_are_two_triggers_that_each_resolve_as_given(
    #[case] order: &[usize],
) -> Result<(), ScriptError> {
    let banner = |name: &str, life: i32| {
        let grant = format!(
            r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"mods":[{{"GrantAbility":{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"GainLife":{{"amount":{life}}}}}]}}}}}}]}}}}"#
        );
        ready(totem(name, &grant))
    };
    let scenario = Scenario::new(2)
        .battlefield(
            0,
            [
                banner("Banner of Three", 3),
                banner("Banner of Five", 5),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 4, 4))])
        .at(1, 1, Step::Main1);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 0)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.block(&[("Bear", "Ogre")])?;
    while driver.trigger_options().is_err() {
        driver.pass()?;
    }
    assert_eq!(
        driver.trigger_options()?.triggers.len(),
        2,
        "the abilities are not the same ability: the controller orders them"
    );
    driver.order_triggers(order)?;
    // The stack, bottom first, holds what each trigger said: the life of the static that gave it.
    let state = driver.state();
    let on_the_stack: Vec<i32> = state
        .stack()
        .iter()
        .map(|id| {
            let rules = state.object(*id).unwrap().ability().unwrap().rules();
            match rules.effects() {
                [
                    mtg_cards::effect::Effect::GainLife {
                        amount: mtg_cards::value::Amount::Fixed(life),
                        ..
                    },
                ] => *life,
                other => panic!("a trigger that gains life was expected, got {other:?}"),
            }
        })
        .collect();
    let wanted: Vec<i32> = order.iter().map(|at| [3, 5][*at]).collect();
    assert_eq!(on_the_stack, wanted, "each trigger kept the ability it was");
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        28,
        "each gave the life its own static said, whatever the order"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_triggered_ability_a_static_ability_grants_triggers_and_survives_the_death_of_the_creature()
-> Result<(), ScriptError> {
    // "Creatures you control have 'When this creature dies, you gain 3 life.'"
    let grant = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":3}}]}}}]}}"#;
    let scenario = Scenario::new(2)
        .battlefield(
            0,
            [
                ready(totem("Banner", grant)),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 4, 4))])
        .at(1, 1, Step::Main1);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    // Seat 1 has the turn but no way to attack: seat 0's Bear dies to the Ogre when it blocks.
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 0)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.block(&[("Bear", "Ogre")])?;
    driver.pass_until(Step::End)?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        23,
        "the Bear died with the ability it was given and it still triggered"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_token_made_in_play_with_a_triggered_ability_triggers_in_a_game_where_no_card_had_one()
-> Result<(), ScriptError> {
    let spawn = Arc::new(
        CardDefinition::new("Spawn")
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_type(CardType::Sorcery)
            .with_rules(ability(
                r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Imp","types":["Creature"],"power":1,"toughness":1,"abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"GainLife":{"amount":2}}]}}]}}}}]}}"#,
            )),
    );
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(forest())])
        .hand(0, [spawn]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Spawn", &["Forest"])?;
    resolve_the_stack(&mut driver)?;
    assert!(driver.id_of("Imp").is_ok(), "the token is in play");
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::AbilityTriggered(_))),
        "nothing has triggered yet"
    );
    driver.idle_until(3, Step::Upkeep)?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        22,
        "the token's own ability triggered at the upkeep"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_token_that_triggers_as_it_enters_is_seen_by_the_batch_that_made_it() -> Result<(), ScriptError>
{
    let spawn = Arc::new(
        CardDefinition::new("Spawn")
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_type(CardType::Sorcery)
            .with_rules(ability(
                r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Imp","types":["Creature"],"power":1,"toughness":1,"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":2}}]}}]}}}}]}}"#,
            )),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(forest())])
            .hand(0, [spawn]),
    )?;
    driver.cast_tapping("Spawn", &["Forest"])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    Ok(())
}

#[test]
fn a_creature_that_cannot_attack_is_not_offered_and_one_that_cannot_block_does_not_block()
-> Result<(), ScriptError> {
    let pacifist = bearing(
        "Pacifist",
        r#"{"Static":{"affected":"This","mods":["CantAttack"]}}"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(pacifist), ready(free_creature("Bear", 2, 2))])
            .battlefield(1, [ready(card("Jungle Lion").unwrap())]),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    assert!(
        driver.attack(&[("Pacifist", 1)]).is_err(),
        "a creature that can't attack is not among the legal attackers"
    );
    driver.attack(&[("Bear", 1)])?;
    driver.pass_until(Step::End)?;
    assert_eq!(
        life(&driver, 1),
        18,
        "the Lion could not block, and the Bear was not stopped"
    );
    Ok(())
}

#[test]
fn a_creature_controlled_through_a_static_ability_attacks_for_its_new_controller_after_their_turn_began()
-> Result<(), ScriptError> {
    let steal =
        r#"{"Static":{"affected":{"Each":[{"Named":"Ogre"}]},"mods":[{"SetController":"You"}]}}"#;
    let scenario = Scenario::new(2)
        .battlefield(
            0,
            [
                ready(totem("Thief", steal)),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 4, 4))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    // Seat 1's Ogre has been theirs since before seat 0's turn began, and it is now seat 0's: it is held back.
    driver.pass_until(Step::DeclareAttackers)?;
    assert!(
        driver.attack(&[("Ogre", 1)]).is_err(),
        "it changed hands during seat 0's turn"
    );
    driver.attack(&[("Bear", 1)])?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1), ("Bear", 1)])?;
    driver.pass_until(Step::End)?;
    assert_eq!(
        life(&driver, 1),
        20 - 2 - 4 - 2,
        "the Ogre dealt its damage for seat 0 on their next turn"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn act_of_treason_takes_a_creature_for_the_turn_and_it_attacks_at_once() -> Result<(), ScriptError>
{
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())])
        .battlefield(1, [tapped(free_creature("Ogre", 4, 4))])
        .hand(0, [card("Act of Treason").unwrap()]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.id_of("Ogre")?;
    let at = driver.target_named("Ogre")?;
    driver.cast_at("Act of Treason", &[at])?;
    resolve_the_stack(&mut driver)?;
    let state = driver.state();
    assert_eq!(
        controller_of(state, ogre),
        Some(pid(0)),
        "seat 0 controls it"
    );
    assert_eq!(
        state.object(ogre).unwrap().controller(),
        pid(1),
        "the object itself did not change hands"
    );
    assert!(
        !state.object(ogre).unwrap().permanent().unwrap().tapped(),
        "it was untapped"
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1)])?;
    driver.pass_until(Step::End)?;
    assert_eq!(
        life(&driver, 1),
        16,
        "it attacked the turn it was taken, with the haste it was given, and dealt its damage for seat 0"
    );
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(
        controller_of(driver.state(), ogre),
        Some(pid(1)),
        "it went back at the end of the turn"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::nothing(&[], 2)]
#[case::an_anthem(&["Glorious Anthem"], 3)]
#[case::humility(&["Humility"], 1)]
#[case::an_anthem_and_humility(&["Glorious Anthem", "Humility"], 2)]
#[case::flying_changes_nothing_here(&["Levitation"], 2)]
fn the_cards_of_the_corpus_change_what_combat_deals(
    #[case] enchantments: &[&str],
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let mut board = vec![ready(free_creature("Bear", 2, 2))];
    board.extend(enchantments.iter().map(|name| ready(card(name).unwrap())));
    let scenario = Scenario::new(2).battlefield(0, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    driver.pass_until(Step::End)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_creature_with_flying_from_a_static_ability_cannot_be_blocked_by_one_without_reach()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(card("Levitation").unwrap()),
                    ready(free_creature("Bear", 2, 2)),
                ],
            )
            .battlefield(1, [ready(free_creature("Wolf", 2, 2))]),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    driver.pass_until(Step::End)?;
    assert_eq!(
        life(&driver, 1),
        18,
        "the Wolf could not block a creature with flying"
    );
    Ok(())
}

#[test]
fn a_creature_that_just_arrived_is_held_back_whoever_controls_the_static_ability()
-> Result<(), ScriptError> {
    // A sick creature stays sick: a static ability that pumps it does not make it ready.
    let anthem = card("Glorious Anthem").unwrap();
    let driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(anthem), sick(free_creature("Bear", 2, 2))]),
    )?;
    let bear = driver.id_of("Bear")?;
    assert!(
        mtg_engine::keywords::summoning_sick(driver.state(), bear),
        "an anthem does not cure summoning sickness"
    );
    Ok(())
}
