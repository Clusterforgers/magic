//! Goal: in a party's shared combat (CR 805, CR 508.1) each hero is asked in turn order for the attacks of their own creatures only, seeing what the teammates before them chose, and the party makes one declaration of every choice: "attacks alone" (exalted) reads the whole declaration, a per-combat limit counts every hero's attackers, and a choice that breaks a rule only together (a creature that can't attack alone) is refused with its reason and asked again, until the engine declares the plain legal attack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, AttackOptions, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "This creature can't attack alone."
const ALONE: &str = r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"CantAttackAlone"}}}}"#;

/// `heroes` heroes each with a ready Bear, then a boss with `boss_board`, under shared team turns at the party's main
/// phase; `extra` adds permanents to the heroes'.
fn party(
    heroes: usize,
    extra: &[(usize, std::sync::Arc<mtg_cards::CardDefinition>)],
    boss_board: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Scenario {
    let mut seats = vec![0; heroes];
    seats.push(1);
    let mut scenario = Scenario::new(heroes + 1)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats,
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1)
        .life(heroes, 100);
    for seat in 0..heroes {
        let mut board = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        board.extend(
            extra
                .iter()
                .filter(|(at, _)| *at == seat)
                .map(|(_, card)| ready(card.clone())),
        );
        scenario = scenario.battlefield(seat, board);
    }
    scenario.battlefield(
        heroes,
        boss_board.into_iter().map(ready).collect::<Vec<_>>(),
    )
}

/// The pending question, which must be a declaration of attackers for `seat`.
fn asked(driver: &Driver, seat: usize) -> AttackOptions {
    let decision = driver.decision().unwrap();
    assert_eq!(
        decision.player,
        pid(seat),
        "asked: {:?}",
        decision.kind.name()
    );
    match &decision.kind {
        DecisionKind::DeclareAttackers(options) => options.clone(),
        other => panic!("expected attackers, got {}", other.name()),
    }
}

/// `seat` attacks the boss with the named creatures.
fn attack(driver: &mut Driver, boss: usize, names: &[String]) -> Result<Vec<Event>, ScriptError> {
    let chosen: Vec<(ObjectId, AttackTarget)> = names
        .iter()
        .map(|name| (driver.id_of(name).unwrap(), AttackTarget::Player(pid(boss))))
        .collect();
    driver.answer(Answer::DeclareAttackers(chosen))
}

fn declared(events: &[Event]) -> Vec<Vec<(ObjectId, AttackTarget)>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::AttackersDeclared { attackers, .. } => Some(attackers.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::two_heroes(2)]
#[case::four_heroes(4)]
#[case::six_heroes(6)]
fn each_hero_chooses_their_own_attacks_and_the_party_declares_once(
    #[case] heroes: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(party(heroes, &[], Vec::new()))?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut all = Vec::new();
    for seat in 0..heroes {
        let options = asked(&driver, seat);
        let own = driver.id_of(&format!("Bear {seat}"))?;
        let offered: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
        assert_eq!(offered, [own], "hero {seat} is offered their own Bear only");
        let party = options.party.expect("a shared declaration");
        let before: Vec<usize> = party
            .declared
            .iter()
            .map(|(who, _, _)| who.index())
            .collect();
        assert_eq!(
            before,
            (0..seat).collect::<Vec<_>>(),
            "hero {seat} sees the choices before"
        );
        let events = attack(&mut driver, heroes, &[format!("Bear {seat}")])?;
        all.extend(events);
        if seat + 1 < heroes {
            assert!(
                declared(&all).is_empty(),
                "nothing is declared before the last hero chose"
            );
        }
    }
    let once = declared(&all);
    assert_eq!(once.len(), 1, "one declaration for the party");
    assert_eq!(once[0].len(), heroes);
    assert_eq!(driver.state().combat().attackers().len(), heroes);
    Ok(())
}

/// Hero 0 holds Akrasan Squire (exalted); `attacking` names the heroes whose Bear attacks.
#[rstest]
#[case::the_first_hero_alone(&[0], 1)]
#[case::both_heroes(&[0, 1], 0)]
#[case::the_second_hero_alone(&[1], 0)]
fn exalted_reads_the_partys_whole_declaration(
    #[case] attacking: &[usize],
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let squire = card("Akrasan Squire").unwrap();
    let mut driver = Driver::from_scenario(party(2, &[(0, squire)], Vec::new()))?;
    driver.pass_until(Step::DeclareAttackers)?;
    for seat in 0..2 {
        asked(&driver, seat);
        let names: Vec<String> = attacking
            .iter()
            .filter(|at| **at == seat)
            .map(|at| format!("Bear {at}"))
            .collect();
        attack(&mut driver, 2, &names)?;
    }
    // The Squire's exalted triggers only for a creature of hero 0 attacking alone in the whole declaration.
    assert_eq!(driver.state().stack().len(), triggers, "{attacking:?}");
    Ok(())
}

#[rstest]
#[case::two_heroes(2)]
#[case::four_heroes(4)]
fn a_limit_of_one_attacker_counts_every_heros_choice(
    #[case] heroes: usize,
) -> Result<(), ScriptError> {
    let arbiter = card("Silent Arbiter").unwrap();
    let mut driver = Driver::from_scenario(party(heroes, &[], vec![arbiter]))?;
    driver.pass_until(Step::DeclareAttackers)?;
    assert_eq!(asked(&driver, 0).most, Some(1));
    attack(&mut driver, heroes, &["Bear 0".to_owned()])?;
    let options = asked(&driver, 1);
    assert_eq!(options.most, Some(0), "the first hero took the one attack");
    assert!(
        attack(&mut driver, heroes, &["Bear 1".to_owned()]).is_err(),
        "a second attacker is refused"
    );
    let mut events = Vec::new();
    for seat in 1..heroes {
        asked(&driver, seat);
        events.extend(attack(&mut driver, heroes, &[])?);
    }
    assert_eq!(
        declared(&events),
        [vec![(
            driver.id_of("Bear 0")?,
            AttackTarget::Player(pid(heroes))
        )]]
    );
    Ok(())
}

#[test]
fn a_choice_illegal_only_together_is_asked_again_with_its_reason() -> Result<(), ScriptError> {
    let flunky = with_rules(
        &free_creature("Flunky", 3, 3),
        vec![serde_json::from_str(ALONE).unwrap()],
    );
    let mut driver = Driver::from_scenario(party(2, &[(0, flunky)], Vec::new()))?;
    driver.pass_until(Step::DeclareAttackers)?;
    // Twice the Flunky attacks alone and the party chooses again; the reason is shown to the first hero.
    for round in 0..2 {
        let options = asked(&driver, 0);
        assert_eq!(
            options.party.unwrap().refused.is_some(),
            round > 0,
            "round {round}"
        );
        attack(&mut driver, 2, &["Flunky".to_owned()])?;
        asked(&driver, 1);
        let events = attack(&mut driver, 2, &[])?;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AttackChoicesRefused { .. })),
            "round {round}: refused"
        );
        assert_eq!(
            declared(&events),
            Vec::<Vec<(ObjectId, AttackTarget)>>::new()
        );
    }
    // Together with the second hero's Bear it is legal.
    attack(&mut driver, 2, &["Flunky".to_owned()])?;
    let events = attack(&mut driver, 2, &["Bear 1".to_owned()])?;
    assert_eq!(declared(&events)[0].len(), 2);
    Ok(())
}

#[test]
fn after_two_refusals_the_engine_declares_the_plain_legal_attack() -> Result<(), ScriptError> {
    let flunky = with_rules(
        &free_creature("Flunky", 3, 3),
        vec![serde_json::from_str(ALONE).unwrap()],
    );
    let mut driver = Driver::from_scenario(party(2, &[(0, flunky)], Vec::new()))?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut last = Vec::new();
    for _ in 0..3 {
        attack(&mut driver, 2, &["Flunky".to_owned()])?;
        last = attack(&mut driver, 2, &[])?;
    }
    assert_eq!(
        declared(&last),
        [Vec::new()],
        "no attack: the legal choice the rules require"
    );
    Ok(())
}
