//! Goal: when the active player leaves the game (a concession or a loss) the turn goes on without an active player (CR 800.4): their objects leave and the effects that gave them control end, nobody is asked what only the active player decides, priority starts with the next player in turn order in every step, the turn runs to its cleanup and the next turn belongs to the next player in turn order; a two-player game ends at once; at every step the active player can be in, with a spell, an ability or attackers of theirs in play, and at tables of three to five seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::player::{LossReason, Player};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameResult, characteristics};
use mtg_testkit::cards::{burn, forest, free_creature, gain_life, mountain, pinger};
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The steps of a turn in which nothing happens.
const QUIET_TURN: [Step; 10] = [
    Step::Untap,
    Step::Upkeep,
    Step::Draw,
    Step::Main1,
    Step::BeginCombat,
    Step::DeclareAttackers,
    Step::EndCombat,
    Step::Main2,
    Step::End,
    Step::Cleanup,
];

/// What happened from a concession to the end of the turn.
#[derive(Default)]
struct Remainder {
    /// Every event, from the concession on.
    events: Vec<Event>,
    /// The seat asked first in each step that asked anybody.
    asked: Vec<(Step, PlayerId)>,
    /// The names of the kinds of decision asked.
    kinds: Vec<&'static str>,
}

impl Remainder {
    /// The steps that began in the turn, in order.
    fn steps(&self) -> Vec<Step> {
        self.events
            .iter()
            .take_while(|e| !matches!(e, Event::TurnBegan { .. }))
            .filter_map(|e| match e {
                Event::StepBegan { step } => Some(*step),
                _ => None,
            })
            .collect()
    }

    /// The turn that began after it: its number and active player.
    fn next_turn(&self) -> Option<(u32, PlayerId)> {
        self.events.iter().find_map(|e| match e {
            Event::TurnBegan { number, active, .. } => Some((*number, *active)),
            _ => None,
        })
    }

    fn asked(&self, kind: &str) -> bool {
        self.kinds.contains(&kind)
    }
}

/// Answers passively, each decision by the seat it asks, until the turn is over; `events` are what came before.
fn play_out_turn(driver: &mut Driver, events: Vec<Event>) -> Result<Remainder, ScriptError> {
    let turn = driver.state().turn().number();
    let mut rest = Remainder {
        events,
        ..Remainder::default()
    };
    while driver.state().turn().number() == turn {
        let decision = driver.decision()?.clone();
        assert!(
            driver.state().is_alive(decision.player),
            "{:?} has left the game and is asked for {}",
            decision.player,
            decision.kind.name()
        );
        rest.kinds.push(decision.kind.name());
        let step = driver.state().turn().step();
        if rest.asked.last().map(|(s, _)| *s) != Some(step) {
            rest.asked.push((step, decision.player));
        }
        rest.events.extend(driver.answer_passively()?);
    }
    Ok(rest)
}

fn conceded(seat: usize) -> Event {
    Event::PlayerLost {
        player: pid(seat),
        reason: LossReason::Conceded,
    }
}

fn life(driver: &Driver, seat: usize) -> Option<i32> {
    driver.state().player(pid(seat)).map(Player::life)
}

/// Seat `seat` holds a Bear, a land and a spell, and moves in `step` of turn 2.
fn table(players: usize, seat: usize, step: Step) -> Scenario {
    Scenario::new(players)
        .at(2, seat, step)
        .battlefield(seat, [ready(free_creature("Bear", 2, 2)), ready(forest())])
        .hand(seat, [forest()])
}

/// The state right after the active player has left: the turn is the same and has no active player.
fn assert_turn_without_active_player(driver: &Driver, seat: usize, turn: u32) {
    let state = driver.state();
    assert_eq!(state.turn().number(), turn, "the turn goes on");
    assert_eq!(state.turn().active(), pid(seat));
    assert_eq!(state.active_player(), None, "nobody is the active player");
    assert!(state.objects().all(|o| o.owner() != pid(seat)));
}

#[rstest]
fn the_turn_runs_out_without_an_active_player(
    #[values(3, 4)] players: usize,
    #[values(0, 1, 2, 3)] seat: usize,
    #[values(
        Step::Upkeep,
        Step::Draw,
        Step::Main1,
        Step::BeginCombat,
        Step::EndCombat,
        Step::Main2,
        Step::End
    )]
    step: Step,
) -> Result<(), ScriptError> {
    if seat >= players {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(players, seat, step))?;
    assert_eq!(
        driver.decision()?.player,
        pid(seat),
        "the active player moves first"
    );
    let events = driver.concede(pid(seat))?;
    assert_eq!(events.first(), Some(&conceded(seat)));
    assert_turn_without_active_player(&driver, seat, 2);
    assert_eq!(driver.state().turn().step(), step, "the step goes on");
    let next = (seat + 1) % players;
    assert_eq!(
        driver.decision()?.player,
        pid(next),
        "priority passes to the next seat"
    );
    let hands: Vec<usize> = (0..players)
        .map(|p| driver.state().zone(Zone::Hand(pid(p))).len())
        .collect();

    let rest = play_out_turn(&mut driver, events)?;

    let position = QUIET_TURN.iter().position(|s| *s == step).unwrap();
    assert_eq!(
        rest.steps(),
        QUIET_TURN[position + 1..],
        "every later step still begins"
    );
    assert!(
        rest.kinds.iter().all(|kind| *kind == "priority"),
        "{:?}",
        rest.kinds
    );
    assert!(
        rest.asked.iter().all(|(_, seat)| *seat == pid(next)),
        "every step starts with the next seat: {:?}",
        rest.asked
    );
    assert_eq!(
        rest.next_turn(),
        Some((3, pid(next))),
        "the next turn is the next seat's"
    );
    let state = driver.state();
    assert_eq!(state.turn().active(), pid(next));
    assert_eq!(state.active_player(), Some(pid(next)));
    assert_eq!(state.turn().step(), Step::Upkeep);
    let still: Vec<usize> = (0..players)
        .map(|p| state.zone(Zone::Hand(pid(p))).len())
        .collect();
    assert_eq!(
        still, hands,
        "nobody drew in the turn that had no active player"
    );
    // The next turn is a turn like any other: its active player draws.
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(next))).len(),
        hands[next] + 1
    );
    Ok(())
}

#[rstest]
fn a_two_player_game_ends_at_once(
    #[values(0, 1)] seat: usize,
    #[values(
        Step::Upkeep,
        Step::Draw,
        Step::Main1,
        Step::BeginCombat,
        Step::EndCombat,
        Step::Main2,
        Step::End
    )]
    step: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, seat, step))?;
    let events = driver.concede(pid(seat))?;
    assert_eq!(events.first(), Some(&conceded(seat)));
    assert!(matches!(events.last(), Some(Event::GameEnded { .. })));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::StepBegan { .. } | Event::TurnBegan { .. })),
        "no step or turn follows: {events:?}"
    );
    assert_eq!(
        driver.engine().result(),
        Some(GameResult::Winner(pid(1 - seat)))
    );
    assert!(driver.decision().is_err());
    Ok(())
}

/// The turn after a leaving is the next living seat's, whichever seats left before.
#[rstest]
#[case::three_seats_first(3, &[], 0, 1)]
#[case::three_seats_middle(3, &[], 1, 2)]
#[case::three_seats_last_wraps(3, &[], 2, 0)]
#[case::four_seats_last_wraps(4, &[], 3, 0)]
#[case::skips_a_seat_that_left_before(4, &[1], 0, 2)]
#[case::wraps_over_a_seat_that_left_before(4, &[0], 3, 1)]
#[case::skips_two_seats(5, &[1, 2], 0, 3)]
#[case::wraps_over_two_seats(5, &[4, 0], 3, 1)]
#[case::five_seats_last(5, &[], 4, 0)]
fn the_next_turn_goes_to_the_next_player_in_turn_order(
    #[case] players: usize,
    #[case] gone: &[usize],
    #[case] seat: usize,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let scenario = gone
        .iter()
        .fold(table(players, seat, Step::Main1), |s, out| s.life(*out, 0));
    let mut driver = Driver::from_scenario(scenario)?;
    let events = driver.concede(pid(seat))?;
    assert_turn_without_active_player(&driver, seat, 2);
    let rest = play_out_turn(&mut driver, events)?;
    assert_eq!(rest.next_turn(), Some((3, pid(expected))));
    assert!(
        rest.asked.iter().all(|(_, asked)| *asked == pid(expected)
            || !gone.contains(&asked.index()) && *asked != pid(seat)),
        "{:?}",
        rest.asked
    );
    // The seats that left before never move again; a second turn goes on to the seat after the next one.
    let again = driver.state().turn().active();
    assert_eq!(again, pid(expected));
    Ok(())
}

#[test]
fn three_active_players_leave_in_turn_and_the_last_seat_wins() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(4, 0, Step::Main1))?;
    for seat in 0..3 {
        assert_eq!(driver.state().turn().active(), pid(seat));
        assert_eq!(driver.state().active_player(), Some(pid(seat)));
        let events = driver.concede(pid(seat))?;
        if seat == 2 {
            assert!(matches!(events.last(), Some(Event::GameEnded { .. })));
            break;
        }
        play_out_turn(&mut driver, events)?;
        assert_eq!(
            driver.state().turn().number(),
            3 + u32::try_from(seat).unwrap()
        );
    }
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(3))));
    Ok(())
}

// Objects of the active player on the stack

#[test]
fn a_spell_of_the_active_player_leaves_the_stack_with_them() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())]),
    )?;
    let at = driver.target_player(1);
    driver.cast_at("Bolt", &[at])?;
    assert_eq!(driver.state().stack().len(), 1);
    let events = driver.concede(pid(0))?;
    assert_eq!(driver.state().stack(), []);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::SpellResolved { .. }))
    );
    assert_eq!(life(&driver, 1), Some(20), "the spell never resolves");
    assert_turn_without_active_player(&driver, 0, 2);
    assert_eq!(driver.decision()?.player, pid(1));
    play_out_turn(&mut driver, events)?;
    assert_eq!(life(&driver, 1), Some(20));
    Ok(())
}

#[test]
fn the_spell_of_another_seat_on_top_resolves_when_the_active_player_leaves()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [burn("Reply", 1)])
            .battlefield(1, [ready(mountain())]),
    )?;
    let at = driver.target_player(2);
    driver.cast_at("Bolt", &[at])?;
    let bolt = driver.state().stack()[0];
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    driver.cast_at("Reply", &[at])?;
    driver.pass()?;
    driver.pass()?;
    // Everybody else has passed with Reply on top, and the active player leaves instead of passing.
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(driver.state().stack().len(), 2);
    let events = driver.concede(pid(0))?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ObjectLeftGame { object } if *object == bolt)),
        "the spell of the player who left is gone"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::SpellResolved { .. }))
            .count(),
        1,
        "the spell on top resolves"
    );
    assert_eq!(life(&driver, 2), Some(19), "only Reply hurt");
    assert_eq!(driver.state().stack(), []);
    assert_turn_without_active_player(&driver, 0, 2);
    assert_eq!(driver.decision()?.player, pid(1));
    Ok(())
}

#[test]
fn an_ability_of_the_active_player_ceases_to_exist_when_they_leave() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(pinger("Pinger", 0))]),
    )?;
    driver.begin_activation("Pinger")?;
    let at = driver.target_player(1);
    driver.choose_targets(&[at])?;
    assert_eq!(driver.state().stack().len(), 1);
    let events = driver.concede(pid(0))?;
    assert_eq!(driver.state().stack(), []);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::ResolutionBegan { .. }))
    );
    let rest = play_out_turn(&mut driver, events)?;
    assert_eq!(life(&driver, 1), Some(20));
    assert_eq!(rest.next_turn(), Some((3, pid(1))));
    Ok(())
}

#[test]
fn a_trigger_of_the_active_player_on_the_stack_ceases_to_exist_when_they_leave()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(3).at(2, 0, Step::Untap).battlefield(
        0,
        [ready(watcher(
            "Spring",
            when::your_upkeep(),
            vec![gain_life(5)],
        ))],
    ))?;
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    assert_eq!(driver.state().stack().len(), 1, "the upkeep trigger waits");
    let events = driver.concede(pid(0))?;
    assert_eq!(driver.state().stack(), []);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::ResolutionBegan { .. }))
    );
    assert_turn_without_active_player(&driver, 0, 2);
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    Ok(())
}

#[test]
fn the_end_step_still_triggers_what_names_no_player_in_a_turn_without_an_active_player()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(3).at(2, 0, Step::Main1).battlefield(
        1,
        [ready(watcher(
            "Clock",
            when::an_end_step(),
            vec![gain_life(1)],
        ))],
    ))?;
    let events = driver.concede(pid(0))?;
    play_out_turn(&mut driver, events)?;
    assert_eq!(
        life(&driver, 1),
        Some(21),
        "the end step began and the trigger resolved"
    );
    Ok(())
}

// Combat

#[test]
fn attackers_of_the_active_player_leave_combat_when_they_leave() -> Result<(), ScriptError> {
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
    let events = driver.concede(pid(0))?;
    assert_eq!(driver.state().combat().attackers(), []);
    assert_turn_without_active_player(&driver, 0, 2);
    let rest = play_out_turn(&mut driver, events)?;
    assert!(
        !rest.asked("declare-blockers"),
        "nobody blocks what is gone"
    );
    assert_eq!(
        rest.steps(),
        [Step::EndCombat, Step::Main2, Step::End, Step::Cleanup]
    );
    assert_eq!((life(&driver, 1), life(&driver, 2)), (Some(20), Some(20)));
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield),
        ["D1", "D2"]
    );
    Ok(())
}

#[test]
fn leaving_while_attackers_are_asked_declares_none() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A1", 2, 2))])
            .battlefield(1, [ready(free_creature("D1", 1, 1))]),
    )?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::DeclareAttackers(_)
    ));
    let events = driver.concede(pid(0))?;
    assert!(
        events.iter().any(
            |e| matches!(e, Event::AttackersDeclared { attackers, .. } if attackers.is_empty())
        )
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::NothingToDo { .. }))
    );
    let rest = play_out_turn(&mut driver, events)?;
    assert!(!rest.asked("declare-attackers"));
    assert_eq!(
        rest.steps(),
        [Step::EndCombat, Step::Main2, Step::End, Step::Cleanup]
    );
    Ok(())
}

#[test]
fn leaving_while_damage_is_to_be_assigned_deals_none() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Big", 3, 3))])
            .battlefield(
                1,
                [
                    ready(free_creature("B1", 1, 1)),
                    ready(free_creature("B2", 1, 1)),
                ],
            ),
    )?;
    driver.attack(&[("Big", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.block(&[("B1", "Big"), ("B2", "Big")])?;
    driver.pass_until(Step::CombatDamage)?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::AssignDamage(_)
    ));
    let events = driver.concede(pid(0))?;
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::DamageDealt { .. }))
    );
    assert_eq!(driver.state().combat().attackers(), []);
    assert_eq!(driver.state().turn().step(), Step::CombatDamage);
    assert_eq!(driver.decision()?.player, pid(1));
    let rest = play_out_turn(&mut driver, events)?;
    assert!(!rest.asked("assign-damage"));
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield),
        ["B1", "B2"]
    );
    Ok(())
}

#[test]
fn damage_already_dealt_stays_when_the_active_player_leaves_in_the_damage_step()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Big", 3, 3))]),
    )?;
    driver.attack(&[("Big", 1)])?;
    driver.pass_until(Step::CombatDamage)?;
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(life(&driver, 1), Some(17), "the damage was dealt");
    let events = driver.concede(pid(0))?;
    assert_eq!(driver.state().combat().attackers(), []);
    assert_eq!(life(&driver, 1), Some(17));
    let rest = play_out_turn(&mut driver, events)?;
    assert_eq!(
        rest.steps(),
        [Step::EndCombat, Step::Main2, Step::End, Step::Cleanup]
    );
    assert_eq!(life(&driver, 1), Some(17));
    Ok(())
}

// Cleanup and losses by the rules of the game

#[test]
fn nobody_discards_for_the_active_player_who_left_before_cleanup() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::End)
            .hand(0, std::iter::repeat_with(forest).take(9)),
    )?;
    driver.pass_until(Step::Cleanup)?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Discard(_)));
    let events = driver.concede(pid(0))?;
    assert!(
        events.contains(&Event::TurnBegan {
            number: 3,
            active: pid(1),
            extra: false
        }),
        "the turn ends with the cleanup, which has nobody to ask"
    );
    assert_eq!(driver.state().turn().active(), pid(1));
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    Ok(())
}

#[test]
fn losing_on_an_empty_library_in_the_draw_step_lets_the_turn_go_on() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(4, 0, Step::Untap)
            .library(0, std::iter::empty()),
    )?;
    driver.pass_until(Step::Draw)?;
    assert!(!driver.state().is_alive(pid(0)));
    assert_eq!(driver.state().turn().number(), 4);
    assert_eq!(driver.state().active_player(), None);
    assert_eq!(
        driver.decision()?.player,
        pid(1),
        "the draw step's priority passes on"
    );
    let lost = driver.engine().events().iter().any(|e| {
        matches!(e, Event::PlayerLost { player, reason: LossReason::EmptyLibrary } if *player == pid(0))
    });
    assert!(lost);
    let rest = play_out_turn(&mut driver, Vec::new())?;
    assert_eq!(
        rest.steps(),
        [
            Step::Main1,
            Step::BeginCombat,
            Step::DeclareAttackers,
            Step::EndCombat,
            Step::Main2,
            Step::End,
            Step::Cleanup
        ]
    );
    assert_eq!(rest.next_turn(), Some((5, pid(1))));
    Ok(())
}

#[test]
fn losing_to_your_own_spell_in_the_main_phase_lets_the_turn_go_on() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .life(0, 3)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())]),
    )?;
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me])?;
    for _ in 0..3 {
        driver.pass()?;
    }
    assert!(!driver.state().is_alive(pid(0)));
    assert_turn_without_active_player(&driver, 0, 2);
    assert_eq!(driver.state().turn().step(), Step::Main1);
    assert_eq!(driver.decision()?.player, pid(1));
    let rest = play_out_turn(&mut driver, Vec::new())?;
    assert_eq!(rest.next_turn(), Some((3, pid(1))));
    assert!(rest.asked.iter().all(|(_, seat)| *seat == pid(1)));
    Ok(())
}

// Effects and control

#[test]
fn effects_of_the_player_who_left_end_with_their_control_of_objects() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .battlefield(1, [ready(free_creature("Ogre", 3, 3))]),
    )?;
    let ogre = driver.id_of("Ogre")?;
    driver.add_effect(
        driver.on(ogre),
        Modification::SetController(pid(0)),
        Duration::Forever,
    )?;
    driver.add_effect(
        driver.on(ogre),
        Modification::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
        Duration::UntilEndOfTurn,
    )?;
    driver.add_effect(
        driver.on(ogre),
        Modification::ModifyPowerToughness {
            power: 2,
            toughness: 2,
        },
        Duration::UntilYourNextTurn,
    )?;
    assert_eq!(
        characteristics::controller_of(driver.state(), ogre),
        Some(pid(0))
    );
    let events = driver.concede(pid(0))?;
    assert_eq!(
        characteristics::controller_of(driver.state(), ogre),
        Some(pid(1)),
        "the Ogre goes back"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::EffectExpired { .. }))
            .count(),
        2,
        "the effect that gave them control and the one that lasts until their next turn end"
    );
    assert_eq!(
        driver.state().effects().len(),
        1,
        "the one until end of turn goes on"
    );
    play_out_turn(&mut driver, events)?;
    assert!(driver.state().effects().is_empty(), "it ends with the turn");
    assert_eq!(common::names(driver.state(), Zone::Battlefield), ["Ogre"]);
    Ok(())
}

#[test]
fn an_object_still_controlled_by_the_player_who_left_is_exiled() -> Result<(), ScriptError> {
    let setup = Scenario::new(3)
        .at(2, 1, Step::Main1)
        .hand(0, [free_creature("Borrowed", 2, 2)])
        .setup()?;
    let probe = Engine::new(setup.clone())?;
    let borrowed = probe.state().zone(Zone::Hand(pid(0)))[0];
    // Seat 0's creature came into play under the control of seat 1, who then leaves.
    let mut log = probe.events().to_vec();
    log.extend([
        Event::ZoneChanged {
            object: borrowed,
            from: Zone::Hand(pid(0)),
            to: Zone::Battlefield,
            cause: mtg_engine::events::ZoneCause::Moved,
        },
        Event::Entered {
            object: borrowed,
            tapped: false,
            controller: pid(1),
            chosen: None,
        },
        conceded(1),
    ]);
    let engine = Engine::resume(setup, log)?;
    let state = engine.state();
    let object = state
        .object(borrowed)
        .expect("the card is exiled, not gone");
    assert_eq!((object.zone(), object.owner()), (Zone::Exile, pid(0)));
    assert_eq!(state.zone(Zone::Battlefield), []);
    assert_eq!(state.active_player(), None);
    Ok(())
}
