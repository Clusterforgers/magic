//! Goal: players are eliminated cleanly in multiplayer games (objects removed, turns and priority skip them) and the winner or draw is decided correctly.

use crate::common;

use common::pid;
use mtg_engine::choose::Choices;
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::decision::Answer;
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameResult, apply_event};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn scenario_with_dead(players: usize, dead: &[usize]) -> Scenario {
    dead.iter()
        .fold(Scenario::new(players), |s, seat| s.life(*seat, 0))
}

#[rstest]
#[case::two_players_one_loses(2, &[0], Some(GameResult::Winner(pid(1))))]
#[case::two_players_other_loses(2, &[1], Some(GameResult::Winner(pid(0))))]
#[case::two_players_both_lose(2, &[0, 1], Some(GameResult::Draw))]
#[case::three_players_two_lose(3, &[1, 2], Some(GameResult::Winner(pid(0))))]
#[case::three_players_first_two_lose(3, &[0, 1], Some(GameResult::Winner(pid(2))))]
#[case::three_players_one_loses(3, &[1], None)]
#[case::four_players_three_lose(4, &[0, 1, 2], Some(GameResult::Winner(pid(3))))]
#[case::five_players_all_lose_together(5, &[0, 1, 2, 3, 4], Some(GameResult::Draw))]
#[case::eight_players_seven_lose(8, &[0, 1, 2, 4, 5, 6, 7], Some(GameResult::Winner(pid(3))))]
#[case::eight_players_nobody_loses(8, &[], None)]
fn winner_or_draw_is_decided_when_players_lose_together(
    #[case] players: usize,
    #[case] dead: &[usize],
    #[case] expected: Option<GameResult>,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(scenario_with_dead(players, dead))?;
    assert_eq!(driver.engine().result(), expected);
    assert_eq!(driver.engine().decision().is_none(), expected.is_some());
    let ended = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::GameEnded { .. }))
        .count();
    assert_eq!(ended, usize::from(expected.is_some()));
    if expected.is_some() {
        assert!(matches!(
            driver.engine().events().last(),
            Some(Event::GameEnded { .. })
        ));
    }
    Ok(())
}

#[test]
fn nothing_more_can_be_answered_after_the_game_ends() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario_with_dead(2, &[1]))?;
    let result = driver.answer(Answer::Priority(mtg_engine::decision::PriorityAction::Pass));
    assert_eq!(result, Err(ScriptError::Engine(EngineError::NoDecision)));
    Ok(())
}

#[rstest]
fn an_eliminated_players_objects_all_leave_the_game(
    #[values(3, 5)] players: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(players)
            .life(1, 0)
            .hand(1, [forest(), forest()])
            .battlefield(1, [ready(forest()), ready(free_creature("Doomed", 1, 1))])
            .graveyard(1, [forest()])
            .battlefield(0, [ready(free_creature("Survivor", 1, 1))]),
    )?;
    let state = driver.state();
    assert!(
        state.objects().all(|o| o.owner() != pid(1)),
        "no object owned by the loser remains"
    );
    let left = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::ObjectLeftGame { .. }))
        .count();
    assert_eq!(
        left,
        30 + 2 + 2 + 1,
        "library, hand, battlefield and graveyard all left"
    );
    assert_eq!(common::names(state, Zone::Battlefield), ["Survivor"]);
    assert!(matches!(
        driver
            .engine()
            .events()
            .iter()
            .find(|e| matches!(e, Event::PlayerLost { .. })),
        Some(Event::PlayerLost {
            reason: LossReason::ZeroLife,
            ..
        })
    ));
    assert_eq!(driver.engine().result(), None);
    Ok(())
}

#[rstest]
#[case::skip_middle(4, &[1])]
#[case::skip_two(5, &[1, 3])]
#[case::skip_first_and_last(4, &[0, 3])]
#[case::two_left(6, &[0, 2, 3, 4])]
fn turns_and_priority_skip_eliminated_players(
    #[case] players: usize,
    #[case] dead: &[usize],
) -> Result<(), ScriptError> {
    let alive: Vec<usize> = (0..players).filter(|s| !dead.contains(s)).collect();
    let first = alive[0];
    let mut driver =
        Driver::from_scenario(scenario_with_dead(players, dead).at(1, first, Step::Upkeep))?;
    // One priority round involves exactly the living players, in turn order.
    let mut holders = Vec::new();
    for _ in 0..alive.len() {
        holders.push(driver.decision()?.player.index());
        driver.pass()?;
    }
    assert_eq!(holders, alive);
    // Two full rounds of turns visit living seats only.
    let mut actives = vec![first];
    for _ in 0..(alive.len() * 40) {
        let decision = driver.decision()?;
        assert!(
            alive.contains(&decision.player.index()),
            "{:?} is eliminated",
            decision.player
        );
        driver.answer_passively()?;
        let active = driver.state().turn().active().index();
        if actives.last() != Some(&active) {
            actives.push(active);
        }
        if actives.len() > alive.len() * 2 {
            break;
        }
    }
    let expected: Vec<usize> = alive.iter().copied().cycle().take(actives.len()).collect();
    assert_eq!(actives, expected);
    Ok(())
}

#[test]
fn an_attacker_aimed_at_an_eliminated_player_leaves_combat() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("A1", 2, 2)),
                    ready(free_creature("A2", 2, 2)),
                ],
            )
            .battlefield(1, [ready(free_creature("D1", 1, 1))])
            .battlefield(2, [ready(free_creature("D2", 1, 1))]),
    )?;
    driver.attack(&[("A1", 1), ("A2", 2)])?;
    assert_eq!(driver.state().combat().attackers().len(), 2);
    let mut state = driver.state().clone();
    apply_event(
        &mut state,
        &Event::PlayerLost {
            player: pid(1),
            reason: LossReason::ZeroLife,
        },
    )
    .map_err(ScriptError::Engine)?;
    let remaining: Vec<AttackTarget> = state
        .combat()
        .attackers()
        .iter()
        .map(mtg_engine::combat::Attacker::target)
        .collect();
    assert_eq!(remaining, [AttackTarget::Player(pid(2))]);
    Ok(())
}

#[test]
fn priority_moves_on_when_its_holder_is_eliminated() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(3).at(2, 0, Step::Upkeep))?;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    let mut state = driver.state().clone();
    apply_event(
        &mut state,
        &Event::PlayerLost {
            player: pid(1),
            reason: LossReason::ZeroLife,
        },
    )
    .map_err(ScriptError::Engine)?;
    let priority = state
        .priority()
        .ok_or_else(|| ScriptError::Expectation("no priority round".into()))?;
    assert_eq!(
        priority.holder(),
        pid(2),
        "priority passes to the next living player"
    );
    assert!(!priority.passed().contains(&pid(1)));
    Ok(())
}

#[test]
fn a_losing_players_spells_leave_the_stack_with_them() -> Result<(), ScriptError> {
    let setup = Scenario::new(3)
        .hand(1, [instant("Doomed Spell", 0, &[])])
        .battlefield(0, [ready(free_creature("Striker", 3, 3))])
        .life(1, 3)
        .setup()?;
    let probe = Engine::new(setup.clone())?;
    let spell = probe.state().zone(Zone::Hand(pid(1)))[0];
    let striker = probe.state().zone(Zone::Battlefield)[0];
    // The spell is on the stack when lethal damage arrives; resuming from the crafted log runs the SBAs.
    let crafted = vec![
        Event::PriorityGranted { player: pid(1) },
        Event::SpellCast {
            player: pid(1),
            object: spell,
            choices: Choices::default(),
        },
        Event::DamageDealt {
            source: striker,
            target: DamageTarget::Player(pid(1)),
            amount: 3,
            deathtouch: false,
            form: Default::default(),
            toxic: 0,
        },
    ];
    let mut log: Vec<Event> = probe.events().to_vec();
    log.extend(crafted);
    let resumed = Engine::resume(setup, log)?;
    assert_eq!(resumed.state().stack().len(), 0);
    assert!(resumed.state().objects().all(|o| o.owner() != pid(1)));
    assert!(!resumed.state().is_alive(pid(1)));
    Ok(())
}
