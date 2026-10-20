//! Goal: tokens play whole at tables of two, three and four seats: Dockside Extortionist makes one Treasure for each artifact and enchantment its opponents control, all in one batch, and a Treasure is activated at priority without the stack, sacrificed, and its mana of any colour pays for a spell of that colour; Hero of Bladehold's two Soldiers enter tapped and attacking without being declared, the defender is chosen when there is more than one, and they deal their damage there; a token that dies or is bounced ceases to exist at once, after the triggers of the watchers (and of its own) have seen it go; a token sacrificed for the cost of its own ability still resolves that ability; the log replays to the live state and an engine rebuilt at each decision asks the same thing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::OptionChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn costing(name: &str, cost: &str) -> CardDefinition {
    CardDefinition::new(name).with_mana_cost(ManaCost::parse(cost).unwrap())
}

/// A 1/1 creature that costs the mana given.
fn sprite(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        costing("Sprite", cost)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// Gains 1 life whenever a token its controller controls goes to a graveyard.
fn mourner() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Mourner")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(
                r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"ControlledBy":"You"},"Token"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    )
}

/// Gains 1 life whenever a token enters the battlefield.
fn cheerer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Cheerer")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(
                r#"{"Triggered":{"when":{"ZoneChange":{"what":["Token"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    )
}

/// Makes a Pest, a token with a trigger of its own, when it enters.
fn hatcher() -> Arc<CardDefinition> {
    let pest = r#"{"Custom":{"name":"Pest","colors":["White"],"types":["Creature"],"subtypes":["Pest"],"power":1,"toughness":1,
        "abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}]}}"#;
    Arc::new(
        costing("Hatcher", "{1}")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(&format!(
                r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{pest}}}}}]}}}}"#
            ))),
    )
}

/// Makes a Gem when it enters: a token that sacrifices itself for a card.
fn jeweller() -> Arc<CardDefinition> {
    let gem = r#"{"Custom":{"name":"Gem","types":["Artifact"],"subtypes":["Gem"],
        "abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Draw":{}}]}}]}}"#;
    Arc::new(
        costing("Jeweller", "{1}")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(&format!(
                r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{gem}}}}}]}}}}"#
            ))),
    )
}

/// An instant that returns a creature to its owner's hand.
fn unsummon() -> Arc<CardDefinition> {
    Arc::new(
        costing("Unsummon", "{U}")
            .with_type(CardType::Instant)
            .with_rules(ability(
                r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
            )),
    )
}

fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The permanents on the battlefield that are tokens named `name`.
fn tokens_named(driver: &Driver, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token && o.card().name == name)
        })
        .collect()
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Seat 0 holds Dockside Extortionist and `hand`, and controls two Mountains and a Swamp; the other seats control `foes`.
fn dockside_table(
    foes: &[&[&str]],
    own: &[&str],
    hand: Vec<Arc<CardDefinition>>,
    extra: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    let mut mine = vec![ready(mountain()), ready(mountain()), ready(island())];
    mine.extend(extra);
    mine.extend(own.iter().map(|name| ready(card(name).unwrap())));
    let mut scenario = Scenario::new(foes.len() + 1)
        .hand(
            0,
            [vec![card("Dockside Extortionist").unwrap()], hand].concat(),
        )
        .battlefield(0, mine);
    for (at, list) in foes.iter().enumerate() {
        scenario = scenario.battlefield_named(at + 1, list.iter().copied());
    }
    scenario
}

#[rstest]
#[case::two_seats_an_artifact_and_an_enchantment(&[&["Sol Ring", "Pacifism"][..]], &[], 2)]
#[case::creatures_do_not_count(&[&["Grizzly Bears"][..]], &[], 0)]
#[case::nothing_to_count(&[&[][..]], &[], 0)]
#[case::three_seats_the_opponents_add_up(&[&["Sol Ring"][..], &["Pacifism", "Sol Ring"][..]], &[], 3)]
#[case::four_seats_and_your_own_do_not_count(&[&["Sol Ring"][..], &[][..], &["Pacifism"][..]], &["Sol Ring"], 2)]
fn dockside_extortionist_makes_a_treasure_for_each_artifact_and_enchantment_its_opponents_control(
    #[case] foes: &[&[&str]],
    #[case] own: &[&str],
    #[case] treasures: usize,
) -> Result<(), ScriptError> {
    let setup = dockside_table(foes, own, Vec::new(), Vec::new())
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(dockside_table(foes, own, Vec::new(), Vec::new()))?;
    driver.cast_tapping("Dockside Extortionist", &["Mountain", "Mountain"])?;
    resolve_the_stack(&mut driver)?;
    let made = tokens_named(&driver, "Treasure");
    assert_eq!(made.len(), treasures);
    for id in &made {
        let treasure = driver.state().object(*id).unwrap();
        assert_eq!(treasure.controller(), pid(0));
        assert_eq!(treasure.owner(), pid(0));
        assert!(treasure.base_types().contains(CardType::Artifact));
        assert!(!treasure.permanent().unwrap().tapped());
    }
    // All of them entered in the one step of the trigger's resolution.
    let log = driver.engine().events();
    let entered: Vec<usize> = log
        .iter()
        .enumerate()
        .filter_map(|(at, event)| matches!(event, Event::TokenCreated(_)).then_some(at))
        .collect();
    assert_eq!(entered.len(), treasures);
    assert!(entered.windows(2).all(|pair| pair[1] == pair[0] + 1));
    assert_replays(&driver, &setup)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_watcher_sees_every_token_enter_together(#[case] seats: usize) -> Result<(), ScriptError> {
    let foes: Vec<&[&str]> = std::iter::once(&["Sol Ring", "Sol Ring", "Pacifism"][..])
        .chain(std::iter::repeat_n(&[][..], seats - 2))
        .collect();
    let mut driver = Driver::from_scenario(dockside_table(
        &foes,
        &[],
        Vec::new(),
        vec![ready(cheerer())],
    ))?;
    let before = life(&driver, 0);
    driver.cast_tapping("Dockside Extortionist", &["Mountain", "Mountain"])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(tokens_named(&driver, "Treasure").len(), 3);
    assert_eq!(life(&driver, 0), before + 3, "one trigger for each token");
    Ok(())
}

#[rstest]
#[case::white(ManaType::White, "{W}")]
#[case::blue(ManaType::Blue, "{U}")]
#[case::black(ManaType::Black, "{B}")]
#[case::red(ManaType::Red, "{R}")]
#[case::green(ManaType::Green, "{G}")]
fn a_treasure_is_spent_for_any_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] mana: ManaType,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let table = || {
        let mut scenario = Scenario::new(seats)
            .hand(0, [card("Dockside Extortionist").unwrap(), sprite(cost)])
            .battlefield(0, [ready(mountain()), ready(mountain())])
            .battlefield_named(1, ["Sol Ring"]);
        for seat in 2..seats {
            scenario = scenario.battlefield(seat, [ready(swamp())]);
        }
        scenario
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.cast_tapping("Dockside Extortionist", &["Mountain", "Mountain"])?;
    resolve_the_stack(&mut driver)?;
    let treasure = tokens_named(&driver, "Treasure")[0];
    // Nothing but the Treasure is left to pay with, and a spell is not offered on its promise.
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert_eq!(options.castable.len(), 0);
    let colors: Vec<ManaType> = options.mana_abilities.iter().map(|m| m.produces).collect();
    assert_eq!(
        colors,
        [
            ManaType::White,
            ManaType::Blue,
            ManaType::Black,
            ManaType::Red,
            ManaType::Green
        ],
        "one source for each color"
    );
    let events = driver.tap_for_mana_of("Treasure", mana)?;
    assert!(matches!(
        events.as_slice(),
        [
            Event::PermanentTapped { .. },
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            },
            Event::ManaAdded { .. },
            ..
        ]
    ));
    assert!(
        driver.state().has_ceased(treasure),
        "the Treasure is gone, not in a graveyard"
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 0);
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().get(mana), 1);
    driver.cast("Sprite")?;
    resolve_the_stack(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Sprite".to_owned()));
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 0);
    assert_replays(&driver, &setup)
}

/// Seat 0 controls Hero of Bladehold, ready to attack on turn 2, and `extra`.
fn hero_table(
    seats: usize,
    extra: Vec<mtg_engine::PresetPermanent>,
    hand: Vec<Arc<CardDefinition>>,
) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [vec![ready(card("Hero of Bladehold").unwrap())], extra].concat(),
        )
        .hand(0, hand)
}

/// Passes until the stack is empty, choosing option `option` when the controller is asked, and checks that an engine
/// rebuilt from the log asks the same at each of those decisions.
fn resolve_choosing(
    driver: &mut Driver,
    setup: &GameSetup,
    option: usize,
) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec()).unwrap();
        assert_eq!(resumed.decision(), Some(&decision));
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::ChooseOption(_) => {
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(option)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the trigger did not resolve".into()))
}

#[rstest]
#[case::two_seats(2, 0, 1)]
#[case::three_seats_the_first_defender(3, 0, 1)]
#[case::three_seats_the_other_defender(3, 1, 2)]
#[case::four_seats_the_last_defender(4, 2, 3)]
fn hero_of_bladeholds_soldiers_enter_tapped_and_attacking_the_defender_chosen(
    #[case] seats: usize,
    #[case] option: usize,
    #[case] defender: usize,
) -> Result<(), ScriptError> {
    let setup = hero_table(seats, Vec::new(), Vec::new()).setup().unwrap();
    let mut driver = Driver::from_scenario(hero_table(seats, Vec::new(), Vec::new()))?;
    let events = driver.attack(&[("Hero of Bladehold", 1)])?;
    // Only Hero was declared; the Soldiers are not attackers that were declared (CR 508.4).
    let declared: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::AttackersDeclared { attackers, .. } => Some(attackers.len()),
            _ => None,
        })
        .collect();
    assert_eq!(declared, [1]);
    let asked = resolve_choosing(&mut driver, &setup, option)?;
    assert_eq!(
        asked,
        usize::from(seats > 2),
        "asked only when there is a choice"
    );
    let soldiers = tokens_named(&driver, "Soldier");
    assert_eq!(soldiers.len(), 2);
    for soldier in &soldiers {
        let object = driver.state().object(*soldier).unwrap();
        assert!(object.permanent().unwrap().tapped());
        assert_eq!(object.controller(), pid(0));
        let attacker = driver
            .state()
            .combat()
            .attackers()
            .iter()
            .find(|a| a.object() == *soldier)
            .expect("it attacks");
        assert_eq!(attacker.target(), AttackTarget::Player(pid(defender)));
        assert!(!attacker.blocked());
    }
    // Damage: Hero's three at the player it was declared at, the Soldiers' one each at the one chosen.
    driver.pass_until(Step::Main2)?;
    let hero_at_defender = if defender == 1 { 3 } else { 0 };
    assert_eq!(
        life(&driver, 1),
        20 - hero_at_defender
            - if defender == 1 { 2 } else { 0 }
            - if defender == 1 { 0 } else { 3 }
    );
    assert_eq!(
        life(&driver, defender),
        20 - if defender == 1 { 5 } else { 2 }
    );
    assert_replays(&driver, &setup)
}

#[rstest]
#[case::two(2)]
#[case::four(4)]
fn a_token_that_dies_is_seen_by_its_watchers_and_then_ceases_to_exist(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        hero_table(
            seats,
            vec![ready(mourner()), ready(swamp()), ready(swamp())],
            vec![card("Doom Blade").unwrap()],
        )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Hero of Bladehold", 1)])?;
    resolve_choosing(&mut driver, &setup, 0)?;
    let soldiers = tokens_named(&driver, "Soldier");
    assert_eq!(soldiers.len(), 2);
    let before = life(&driver, 0);
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(soldiers[0]).unwrap().reference());
    let events = driver.cast_at("Doom Blade", &[target])?;
    drop(events);
    resolve_the_stack(&mut driver)?;
    assert!(driver.state().has_ceased(soldiers[0]));
    assert!(driver.state().object(soldiers[0]).is_none());
    assert_eq!(tokens_named(&driver, "Soldier").len(), 1);
    assert_eq!(
        life(&driver, 0),
        before + 1,
        "the watcher saw it go to the graveyard"
    );
    assert!(
        !names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Soldier".to_owned()),
        "a token is no card in the graveyard"
    );
    let log = driver.engine().events();
    assert!(log.iter().any(|event| matches!(event, Event::ZoneChanged { object, cause: ZoneCause::Destroyed, .. } if *object == soldiers[0])));
    assert_replays(&driver, &setup)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
fn a_token_with_a_trigger_of_its_own_still_sees_itself_die(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .hand(0, [hatcher(), card("Doom Blade").unwrap()])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.cast_tapping("Hatcher", &["Mountain"])?;
    resolve_the_stack(&mut driver)?;
    let pests = tokens_named(&driver, "Pest");
    assert_eq!(pests.len(), 1);
    let before = life(&driver, 0);
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(pests[0]).unwrap().reference());
    driver.cast_at("Doom Blade", &[target])?;
    resolve_the_stack(&mut driver)?;
    // Its own trigger went on the stack after it had ceased to exist, and resolved.
    assert!(driver.state().has_ceased(pests[0]));
    assert_eq!(life(&driver, 0), before + 1);
    assert_replays(&driver, &setup)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_token_bounced_to_its_owners_hand_ceases_to_exist(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        hero_table(
            seats,
            vec![ready(island()), ready(mourner())],
            vec![unsummon()],
        )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Hero of Bladehold", 1)])?;
    resolve_choosing(&mut driver, &setup, 0)?;
    let soldier = tokens_named(&driver, "Soldier")[0];
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(soldier).unwrap().reference());
    driver.cast_at("Unsummon", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert!(driver.state().has_ceased(soldier));
    let after = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(
        after.iter().filter(|name| *name == "Soldier").count(),
        0,
        "the hand holds no card of it: {hand:?} -> {after:?}"
    );
    assert_replays(&driver, &setup)
}

#[test]
fn a_token_sacrificed_for_its_own_ability_still_resolves_it() -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(2)
            .hand(0, [jeweller()])
            .library(0, [island(), island(), island()])
            .battlefield(0, [ready(mountain())])
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Jeweller")?;
    resolve_the_stack(&mut driver)?;
    let gem = tokens_named(&driver, "Gem")[0];
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Gem")?;
    // The cost is paid: the Gem is gone, and its ability waits on the stack with the rules it was activated from.
    assert!(driver.state().has_ceased(gem));
    let [ability] = driver.state().stack() else {
        panic!("the ability is on the stack");
    };
    let ability = driver.state().object(*ability).unwrap();
    assert_eq!(ability.kind(), ObjectKind::Ability);
    assert_eq!(name_of(driver.state(), ability.id()), "Gem");
    resolve_the_stack(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + 1);
    assert_replays(&driver, &setup)
}

#[test]
fn a_treasure_sacrificed_as_mana_is_seen_leaving_by_a_sacrifice_trigger() -> Result<(), ScriptError>
{
    let watcher = Arc::new(
        CardDefinition::new("Collector")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability(
                r#"{"Triggered":{"when":{"Sacrificed":{"what":[{"Subtype":"Treasure"}],"by":"You"}},"effects":[{"GainLife":{"amount":2}}]}}"#,
            )),
    );
    let table = || {
        dockside_table(
            &[&["Sol Ring"][..]],
            &[],
            Vec::new(),
            vec![ready(watcher.clone())],
        )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.cast_tapping("Dockside Extortionist", &["Mountain", "Mountain"])?;
    resolve_the_stack(&mut driver)?;
    let before = life(&driver, 0);
    driver.tap_for_mana_of("Treasure", ManaType::Black)?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(life(&driver, 0), before + 2);
    assert_replays(&driver, &setup)
}

#[test]
fn only_the_controller_sees_a_treasure_as_a_source_of_mana() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dockside_table(
        &[&["Sol Ring"][..]],
        &[],
        Vec::new(),
        Vec::new(),
    ))?;
    driver.cast_tapping("Dockside Extortionist", &["Mountain", "Mountain"])?;
    resolve_the_stack(&mut driver)?;
    let treasure = tokens_named(&driver, "Treasure")[0];
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(
        options
            .mana_abilities
            .iter()
            .any(|m| m.source.object == treasure)
    );
    // The opponent holds priority after seat 0 passes and is offered nothing of it.
    driver.pass()?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, PlayerId::from_index(1));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("priority was expected");
    };
    assert!(
        options
            .mana_abilities
            .iter()
            .all(|m| m.source.object != treasure)
    );
    Ok(())
}
