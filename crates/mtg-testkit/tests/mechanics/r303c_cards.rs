//! Goal: the r30-3c corpus cards play as their rules say at tables of two to four seats: Archelos has other permanents enter tapped while it is tapped and untapped while it is untapped, Hundred-Battle Veteran gets +2/+4 only with three kinds of counters among its controller's creatures, Enkira must be blocked only while equipped, Sanwell is spared damage only while an artifact creature of its controller attacks, Kaervek's Spite costs every permanent and card in hand of its caster, Pact Weapon keeps its controller in the game at 0 life only while attached, and Camel is spared Desert damage only while attacking; each game replays to the state it reached.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped, with_counters};
use rstest::rstest;

/// The state replays from the log to the hash the game reached.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Every seat has a library to draw from.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    scenario
}

/// A free creature that enters tapped by its own rules.
fn sleepy() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"Tapped"}}"#,
    )
    .unwrap();
    let mut card = (*free_creature("Sleepy", 1, 1)).clone();
    card = card.with_rules(rules);
    std::sync::Arc::new(card)
}

#[rstest]
fn archelos_sets_how_other_permanents_enter(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] archelos_tapped: bool,
    #[values(0, 1)] owner: usize,
    #[values(true, false)] own_tapped: bool,
) -> Result<(), ScriptError> {
    let archelos = card("Archelos, Lagoon Mystic").unwrap();
    let archelos = if archelos_tapped {
        tapped(archelos)
    } else {
        ready(archelos)
    };
    let entering = if own_tapped {
        sleepy()
    } else {
        free_creature("Sleepy", 1, 1)
    };
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(owner, vec![archelos])
        .hand(0, vec![entering]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Sleepy")?;
    for _ in 0..seats {
        if driver.id_of("Sleepy").is_ok() {
            break;
        }
        driver.pass()?;
    }
    let id = driver.id_of("Sleepy")?;
    let object = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(
        object.tapped(),
        archelos_tapped,
        "Archelos tapped: {archelos_tapped}"
    );
    let archelos = driver.id_of("Archelos, Lagoon Mystic")?;
    assert_eq!(
        driver
            .state()
            .object(archelos)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped(),
        archelos_tapped
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::three_kinds(vec![CounterKind::PlusOnePlusOne, CounterKind::Other("shield".to_owned()), CounterKind::Other("stun".to_owned())], 6, 6)]
#[case::two_kinds(vec![CounterKind::PlusOnePlusOne, CounterKind::Other("shield".to_owned())], 4, 2)]
#[case::one_kind_thrice(vec![CounterKind::Other("shield".to_owned()), CounterKind::Other("shield".to_owned()), CounterKind::Other("shield".to_owned())], 4, 2)]
fn hundred_battle_veteran_counts_kinds_of_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] kinds: Vec<CounterKind>,
    #[case] power: i32,
    #[case] toughness: i32,
) -> Result<(), ScriptError> {
    let others: Vec<_> = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| {
            with_counters(
                ready(free_creature(&format!("Bearer {i}"), 0, 5)),
                kind.clone(),
                1,
            )
        })
        .collect();
    // The same kinds among an opponent's creatures do not count.
    let theirs: Vec<_> = ["stun", "shield", "lore"]
        .into_iter()
        .map(|name| CounterKind::Other(name.to_owned()))
        .map(|kind| with_counters(ready(free_creature("Foe", 0, 5)), kind, 1))
        .collect();
    let mut mine = vec![ready(card("Hundred-Battle Veteran").unwrap())];
    mine.extend(others);
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(seats - 1, theirs);
    let driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of("Hundred-Battle Veteran")?;
    let seen = characteristics(driver.state(), id).unwrap();
    assert_eq!(
        (seen.power.unwrap(), seen.toughness.unwrap()),
        (power, toughness)
    );
    replays(&driver);
    Ok(())
}

#[rstest]
fn enkira_must_be_blocked_only_while_equipped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] equipped: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Enkira, Hostile Scavenger").unwrap())];
    if equipped {
        mine.push(attached_to(ready(card("The Masamune").unwrap()), 0, 0));
    }
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, vec![ready(free_creature("Guard", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Enkira, Hostile Scavenger", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    let no_block = driver.block(&[]);
    assert_eq!(no_block.is_err(), equipped, "equipped: {equipped}");
    if equipped {
        driver.block(&[("Guard", "Enkira, Hostile Scavenger")])?;
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn sanwell_is_spared_while_an_artifact_creature_attacks(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] artifact: bool,
) -> Result<(), ScriptError> {
    let attacker = if artifact {
        let base = (*free_creature("Raider", 1, 1)).clone();
        std::sync::Arc::new(base.with_type(mtg_core::card::CardType::Artifact))
    } else {
        free_creature("Raider", 1, 1)
    };
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                ready(card("Sanwell, Avenger Ace").unwrap()),
                ready(attacker),
            ],
        )
        .battlefield(1, vec![ready(free_creature("Wall", 9, 9))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Sanwell, Avenger Ace", 1), ("Raider", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.block(&[("Wall", "Sanwell, Avenger Ace")])?;
    driver.pass_until(Step::EndCombat)?;
    let alive = driver.id_of("Sanwell, Avenger Ace").is_ok();
    assert_eq!(alive, artifact, "artifact attacker: {artifact}");
    replays(&driver);
    Ok(())
}

/// The corpus card, free to cast.
fn free(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let card = (*card(name).unwrap()).clone();
    std::sync::Arc::new(card.with_mana_cost(mtg_testkit::cards::cost(0, &[])))
}

/// Casts the named spell at seat 1 where it asks for a target, picking the first cards a pick asks for.
fn cast_at_seat_one(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..20 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::ChooseTargets(_) => {
                let seat = driver.target_player(1);
                driver.choose_targets(&[seat])?;
            }
            DecisionKind::ChooseCards(options) => {
                let picks = options
                    .from
                    .iter()
                    .take(usize::try_from(options.min).unwrap())
                    .copied()
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => return Ok(()),
        }
    }
    Ok(())
}

#[rstest]
fn kaerveks_spite_takes_every_permanent_and_card_in_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] permanents: usize,
    #[values(0, 2)] cards: usize,
) -> Result<(), ScriptError> {
    let mut hand = vec![free("Kaervek's Spite")];
    hand.extend((0..cards).map(|_| forest()));
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            (0..permanents).map(|_| ready(forest())).collect::<Vec<_>>(),
        )
        .battlefield(1, vec![ready(forest())])
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    cast_at_seat_one(&mut driver, "Kaervek's Spite")?;
    let seat = mtg_core::ids::PlayerId::from_index(0);
    assert!(driver.state().zone(Zone::Hand(seat)).is_empty());
    assert_eq!(names_on(&driver, seat), 0);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(seat)).len(),
        permanents + cards
    );
    for _ in 0..seats {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    let other = mtg_core::ids::PlayerId::from_index(1);
    assert_eq!(driver.state().player(other).unwrap().life(), 15);
    replays(&driver);
    Ok(())
}

/// How many permanents the seat controls.
fn names_on(driver: &Driver, seat: mtg_core::ids::PlayerId) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == seat)
        .count()
}

/// A free instant whose caster loses the life.
fn drain(life: u32) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"LoseLife":{{"amount":{life}}}}}]}}}}"#
    ))
    .unwrap();
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Drain")
            .with_type(mtg_core::card::CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(rules),
    )
}

#[rstest]
fn pact_weapon_keeps_its_controller_in_the_game_while_attached(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] attached: bool,
) -> Result<(), ScriptError> {
    let weapon = card("Pact Weapon").unwrap();
    let weapon = if attached {
        attached_to(ready(weapon), 0, 0)
    } else {
        ready(weapon)
    };
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .life(0, 2)
        .battlefield(0, vec![ready(free_creature("Bearer", 1, 1)), weapon])
        .hand(0, vec![drain(5)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Drain")?;
    for _ in 0..seats {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    let seat = mtg_core::ids::PlayerId::from_index(0);
    assert_eq!(
        driver.state().is_alive(seat),
        attached,
        "attached: {attached}"
    );
    replays(&driver);
    Ok(())
}

/// A Desert whose ability deals 1 damage to any creature.
fn desert() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
    )
    .unwrap();
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Desert")
            .with_type(mtg_core::card::CardType::Land)
            .with_subtype("Desert")
            .with_rules(rules),
    )
}

#[rstest]
fn camel_is_spared_desert_damage_while_attacking(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] attacking: bool,
) -> Result<(), ScriptError> {
    let scenario = libraries(Scenario::new(seats), seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(card("Camel").unwrap())])
        .battlefield(1, vec![ready(desert())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    if attacking {
        driver.attack(&[("Camel", 1)])?;
    } else {
        driver.attack(&[])?;
    }
    for _ in 0..10 {
        let decision = driver.decision()?.clone();
        if decision.player == mtg_core::ids::PlayerId::from_index(1)
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.begin_activation("Desert")?;
    let camel = driver.id_of("Camel")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let camel = driver.state().object(camel).unwrap().reference();
        driver.choose_targets(&[mtg_engine::choose::Target::Object(camel)])?;
    }
    for _ in 0..=seats {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(
        driver.id_of("Camel").is_ok(),
        attacking,
        "attacking: {attacking}"
    );
    replays(&driver);
    Ok(())
}
