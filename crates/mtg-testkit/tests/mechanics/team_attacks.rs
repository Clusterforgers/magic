//! Goal: where teams share their turns (Two-Headed Giant, shared team turns) every player of the active team is asked in turn order for the attacks of their own creatures, so one player declining never keeps a teammate's creatures at home (CR 805.10), and the team makes one declaration that is checked as a whole (CR 805.10b): limits, requirements, "attacks alone" and the refusal path read every player's choice; bots answer for themselves, a human is never decided for, and a table where each player takes their own turn is unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, AttackOptions, DecisionKind};
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// "This creature can't attack alone."
const ALONE: &str = r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"CantAttackAlone"}}}}"#;

/// Two-Headed Giant: two teams of two side by side.
const GIANT: &[u32] = &[0, 0, 1, 1];
/// Shared team turns: three teams of two.
const THREE_PAIRS: &[u32] = &[0, 0, 1, 1, 2, 2];
/// The party and its boss.
const PARTY: &[u32] = &[0, 0, 0, 1];

/// A table of `mode` with the team of each seat, at the main phase of the first team's turn: every seat holds a ready
/// Bear named `Bear <seat>` (a summoning sick one for the seats in `sick_seats`), and `extra` adds ready permanents.
fn table(
    mode: TeamMode,
    seats: &[u32],
    sick_seats: &[usize],
    extra: &[(usize, Arc<CardDefinition>)],
) -> Scenario {
    let mut scenario = Scenario::new(seats.len())
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode,
                seats: seats.to_vec(),
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1);
    for seat in 0..seats.len() {
        let bear = free_creature(&format!("Bear {seat}"), 2, 2);
        let mut board = vec![if sick_seats.contains(&seat) {
            sick(bear)
        } else {
            ready(bear)
        }];
        board.extend(
            extra
                .iter()
                .filter(|(at, _)| *at == seat)
                .map(|(_, card)| ready(card.clone())),
        );
        scenario = scenario.battlefield(seat, board);
    }
    scenario
}

/// The seats of the first team, which takes the first turn, in turn order.
fn members(seats: &[u32]) -> Vec<usize> {
    (0..seats.len()).filter(|seat| seats[*seat] == 0).collect()
}

/// The seat every attack goes to: the first of another team.
fn foe(seats: &[u32]) -> usize {
    seats.iter().position(|team| *team != 0).unwrap()
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

/// Answers with the named creatures, each attacking `foe`.
fn attack(driver: &mut Driver, foe: usize, names: &[String]) -> Result<Vec<Event>, ScriptError> {
    let chosen: Vec<(ObjectId, AttackTarget)> = names
        .iter()
        .map(|name| (driver.id_of(name).unwrap(), AttackTarget::Player(pid(foe))))
        .collect();
    driver.answer(Answer::DeclareAttackers(chosen))
}

fn bear(seat: usize) -> Vec<String> {
    vec![format!("Bear {seat}")]
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

/// A game at the team's declare-attackers step.
fn at_declaration(
    mode: TeamMode,
    seats: &[u32],
    sick_seats: &[usize],
    extra: &[(usize, Arc<CardDefinition>)],
) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(mode, seats, sick_seats, extra))?;
    driver.pass_until(Step::DeclareAttackers)?;
    Ok(driver)
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
#[case::party(TeamMode::SharedTurns, PARTY)]
fn every_player_of_the_team_is_asked_for_their_own_creatures_in_turn_order(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let mut driver = at_declaration(mode, seats, &[], &[])?;
    let team = members(seats);
    let mut all = Vec::new();
    for (at, seat) in team.iter().enumerate() {
        let options = asked(&driver, *seat);
        let own = driver.id_of(&format!("Bear {seat}"))?;
        let offered: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
        assert_eq!(offered, [own], "seat {seat} is offered their own Bear only");
        let party = options.party.expect("a shared declaration");
        let before: Vec<usize> = party.declared.iter().map(|(who, ..)| who.index()).collect();
        assert_eq!(before, team[..at], "seat {seat} sees the choices before");
        all.extend(attack(&mut driver, foe(seats), &bear(*seat))?);
        if at + 1 < team.len() {
            assert!(
                declared(&all).is_empty(),
                "nothing is declared before the last chose"
            );
        }
    }
    let once = declared(&all);
    assert_eq!(once.len(), 1, "one declaration for the team");
    assert_eq!(once[0].len(), team.len());
    assert_eq!(driver.state().combat().attackers().len(), team.len());
    Ok(())
}

#[rstest]
#[case::giant_the_first(TeamMode::TwoHeadedGiant, GIANT, 0)]
#[case::giant_the_second(TeamMode::TwoHeadedGiant, GIANT, 1)]
#[case::pairs_the_first(TeamMode::SharedTurns, THREE_PAIRS, 0)]
#[case::pairs_the_second(TeamMode::SharedTurns, THREE_PAIRS, 1)]
#[case::party_the_first(TeamMode::SharedTurns, PARTY, 0)]
#[case::party_the_second(TeamMode::SharedTurns, PARTY, 1)]
#[case::party_the_third(TeamMode::SharedTurns, PARTY, 2)]
fn a_player_who_declines_does_not_keep_a_teammate_home(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
    #[case] declines: usize,
) -> Result<(), ScriptError> {
    let team = members(seats);
    let mut driver = at_declaration(mode, seats, &[], &[])?;
    let mut all = Vec::new();
    for (at, seat) in team.iter().enumerate() {
        asked(&driver, *seat);
        let names = if at == declines {
            Vec::new()
        } else {
            bear(*seat)
        };
        all.extend(attack(&mut driver, foe(seats), &names)?);
    }
    let kept: Vec<ObjectId> = team
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != declines)
        .map(|(_, seat)| driver.id_of(&format!("Bear {seat}")).unwrap())
        .collect();
    let once = declared(&all);
    assert_eq!(once.len(), 1);
    let attackers: Vec<ObjectId> = once[0].iter().map(|(object, _)| *object).collect();
    assert_eq!(attackers, kept, "every other player's Bear attacks");
    Ok(())
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
fn a_player_with_no_creature_that_can_attack_is_not_asked(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let mut driver = at_declaration(mode, seats, &[0], &[])?;
    let first = members(seats)[1];
    let options = asked(&driver, first);
    assert_eq!(options.party.unwrap().declared, Vec::new());
    let events = attack(&mut driver, foe(seats), &bear(first))?;
    assert_eq!(declared(&events)[0].len(), 1);
    Ok(())
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
fn a_limit_of_one_attacker_counts_every_players_choice(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let arbiter = card("Silent Arbiter").unwrap();
    let mut driver = at_declaration(mode, seats, &[], &[(foe(seats), arbiter)])?;
    let team = members(seats);
    assert_eq!(asked(&driver, team[0]).most, Some(1));
    attack(&mut driver, foe(seats), &bear(team[0]))?;
    assert_eq!(
        asked(&driver, team[1]).most,
        Some(0),
        "the first player took the one attack"
    );
    assert!(
        attack(&mut driver, foe(seats), &bear(team[1])).is_err(),
        "a second attacker is refused"
    );
    let events = attack(&mut driver, foe(seats), &[])?;
    assert_eq!(
        declared(&events),
        [vec![(
            driver.id_of(&format!("Bear {}", team[0]))?,
            AttackTarget::Player(pid(foe(seats)))
        )]]
    );
    Ok(())
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
fn a_creature_that_must_attack_is_required_of_its_controller_alone(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let mut driver = at_declaration(mode, seats, &[], &[])?;
    let team = members(seats);
    let second = driver.id_of(&format!("Bear {}", team[1]))?;
    let named = driver.state().object(second).unwrap().reference();
    driver.add_rule(Rule::MustAttack(vec![named]), Duration::UntilEndOfTurn)?;
    // The teammate who does not control it is free to stay home.
    assert_eq!(asked(&driver, team[0]).required, Vec::new());
    attack(&mut driver, foe(seats), &[])?;
    assert_eq!(asked(&driver, team[1]).required, [second]);
    assert!(
        attack(&mut driver, foe(seats), &[]).is_err(),
        "the creature must attack if able"
    );
    let events = attack(&mut driver, foe(seats), &bear(team[1]))?;
    assert_eq!(declared(&events)[0].len(), 1);
    Ok(())
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
fn a_choice_illegal_only_together_is_asked_again_with_its_reason(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let flunky = with_rules(
        &free_creature("Flunky", 3, 3),
        vec![serde_json::from_str(ALONE).unwrap()],
    );
    let team = members(seats);
    let mut driver = at_declaration(mode, seats, &[], &[(team[0], flunky)])?;
    // Twice the Flunky attacks alone and the team chooses again; the reason is shown to the first player.
    for round in 0..2 {
        let options = asked(&driver, team[0]);
        assert_eq!(
            options.party.unwrap().refused.is_some(),
            round > 0,
            "round {round}"
        );
        attack(&mut driver, foe(seats), &["Flunky".to_owned()])?;
        asked(&driver, team[1]);
        let events = attack(&mut driver, foe(seats), &[])?;
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
    // Beside the second player's Bear it is legal.
    attack(&mut driver, foe(seats), &["Flunky".to_owned()])?;
    let events = attack(&mut driver, foe(seats), &bear(team[1]))?;
    assert_eq!(declared(&events)[0].len(), 2);
    Ok(())
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::three_pairs(TeamMode::SharedTurns, THREE_PAIRS)]
fn after_two_refusals_the_engine_declares_the_plain_legal_attack(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
) -> Result<(), ScriptError> {
    let flunky = with_rules(
        &free_creature("Flunky", 3, 3),
        vec![serde_json::from_str(ALONE).unwrap()],
    );
    let team = members(seats);
    let mut driver = at_declaration(mode, seats, &[], &[(team[0], flunky)])?;
    let mut last = Vec::new();
    for _ in 0..3 {
        attack(&mut driver, foe(seats), &["Flunky".to_owned()])?;
        last = attack(&mut driver, foe(seats), &[])?;
    }
    assert_eq!(
        declared(&last),
        [Vec::new()],
        "no attack: what the rules require"
    );
    Ok(())
}

/// Hero 0 holds Akrasan Squire (exalted); `attacking` names the seats whose Bear attacks.
#[rstest]
#[case::giant_the_first_alone(TeamMode::TwoHeadedGiant, GIANT, &[0], 1)]
#[case::giant_both(TeamMode::TwoHeadedGiant, GIANT, &[0, 1], 0)]
#[case::giant_the_second_alone(TeamMode::TwoHeadedGiant, GIANT, &[1], 0)]
#[case::pairs_the_first_alone(TeamMode::SharedTurns, THREE_PAIRS, &[0], 1)]
#[case::pairs_both(TeamMode::SharedTurns, THREE_PAIRS, &[0, 1], 0)]
fn exalted_reads_the_teams_whole_declaration(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
    #[case] attacking: &[usize],
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let squire = card("Akrasan Squire").unwrap();
    let mut driver = at_declaration(mode, seats, &[], &[(0, squire)])?;
    for seat in members(seats) {
        asked(&driver, seat);
        let names = if attacking.contains(&seat) {
            bear(seat)
        } else {
            Vec::new()
        };
        attack(&mut driver, foe(seats), &names)?;
    }
    // The Squire's exalted triggers only for a creature of its controller attacking alone in the whole declaration.
    assert_eq!(driver.state().stack().len(), triggers, "{attacking:?}");
    Ok(())
}

/// Bots answer for themselves and a human is never decided for: `bot` is the position in the team of the seat a bot
/// plays; the human's Bear attacks, and the bot's choice stands as it made it.
#[rstest]
#[case::giant_the_first_is_a_bot(TeamMode::TwoHeadedGiant, GIANT, 0)]
#[case::giant_the_second_is_a_bot(TeamMode::TwoHeadedGiant, GIANT, 1)]
#[case::pairs_the_second_is_a_bot(TeamMode::SharedTurns, THREE_PAIRS, 1)]
fn a_bot_answers_for_itself_and_waits_for_the_human(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
    #[case] bot: usize,
) -> Result<(), ScriptError> {
    let mut driver = at_declaration(mode, seats, &[], &[])?;
    let team = members(seats);
    let mut brains: Vec<Box<dyn SeatBrain>> = team
        .iter()
        .map(|seat| brain(Level::Normal, 7, pid(*seat), false))
        .collect();
    let mut all = Vec::new();
    let mut bot_attacked = false;
    for (at, seat) in team.iter().enumerate() {
        let options = asked(&driver, *seat);
        let offered: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
        assert_eq!(offered, [driver.id_of(&format!("Bear {seat}"))?]);
        let events = if at == bot {
            let decision = driver.decision()?.clone();
            let answer = brains[at].choose(&decision, Some(driver.engine()));
            if let Answer::DeclareAttackers(chosen)
            | Answer::DeclareExerting {
                attackers: chosen, ..
            } = &answer
            {
                assert!(chosen.iter().all(|(object, _)| offered.contains(object)));
                bot_attacked = !chosen.is_empty();
            }
            driver.answer(answer)?
        } else {
            attack(&mut driver, foe(seats), &bear(*seat))?
        };
        all.extend(events);
        if at + 1 < team.len() {
            assert!(declared(&all).is_empty(), "the human still has to choose");
        }
    }
    let once = declared(&all);
    assert_eq!(once.len(), 1);
    let humans: Vec<ObjectId> = team
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != bot)
        .map(|(_, seat)| driver.id_of(&format!("Bear {seat}")).unwrap())
        .collect();
    for id in &humans {
        assert!(
            once[0].iter().any(|(object, _)| object == id),
            "the human's Bear attacks"
        );
    }
    assert_eq!(once[0].len(), humans.len() + usize::from(bot_attacked));
    Ok(())
}

#[test]
fn where_every_player_takes_their_own_turn_the_active_player_declares_alone()
-> Result<(), ScriptError> {
    let mut driver = at_declaration(TeamMode::Teams, &[0, 1, 0, 1], &[], &[])?;
    let options = asked(&driver, 0);
    assert!(options.party.is_none(), "no shared declaration");
    let offered: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
    assert_eq!(
        offered,
        [driver.id_of("Bear 0")?],
        "the teammate's creatures are not theirs to send"
    );
    let events = attack(&mut driver, 1, &bear(0))?;
    assert_eq!(declared(&events)[0].len(), 1);
    Ok(())
}
