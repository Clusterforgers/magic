//! Goal: "Pass until my turn" (`YieldUntil::MyTurnThrough`) passes a seat through every priority window of the other seats' turns, over opposing spells and its own stops, asks it every decision with a real choice (targets, blockers, discard) without ending, ends as its own turn begins (before any decision there, at 2 to 4 seats), ends when the seat acts by itself, and leaves a log that replays to the same game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Bot, Client, FlakyStore, game_id, passive, roomy, seat, table_setup};
use mtg_cards::refs::{Ref, Targetable};
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Event, GameSetup, replay};
use mtg_protocol::{
    ClientMessage, DecisionId, FullControlMode, ServerMessage, StopSide, YieldUntil,
};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{damage, free_creature, instant};
use mtg_testkit::trigger_cards::{aimed_at, creature_with, trigger, when};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

const THROUGH: YieldUntil = YieldUntil::MyTurnThrough;

/// The default windows decide when a seat is asked, and no clock or dwell runs.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    think: None,
    active: None,
    extension: None,
    relaxed: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

fn spawn(setup: GameSetup) -> (GameHandle, FlakyStore) {
    let store = FlakyStore::new();
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    (handle, store)
}

/// Seats and readies every client; seat 0 plays in Full Control, so it is asked at every window.
async fn join_and_ready(handle: &GameHandle, seats: usize) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(handle, n).await);
    }
    let held = ClientMessage::FullControl {
        mode: FullControlMode::Held,
    };
    clients[0].say(handle, held).await;
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

async fn answer(handle: &GameHandle, from: &Client, id: DecisionId, answer: Answer) {
    let message = ClientMessage::Answer {
        decision: id,
        answer,
    };
    send(handle, from, message).await;
}

/// Answers passively for every seat but `me` while the game waits on one; stops when it waits on `me` or on nobody.
async fn drive_others(handle: &GameHandle, clients: &mut [Client], me: usize) {
    for _ in 0..1000 {
        match handle.status().await.unwrap().waiting_on {
            Some(at) if at != seat(me) => {
                let (id, decision) = clients[at.index()].decision().await;
                answer(handle, &clients[at.index()], id, passive(&decision)).await;
            }
            _ => return,
        }
    }
    panic!("the other seats never stopped");
}

/// Seat 0 casts the first spell it can at the window it is looking at and pays for it.
async fn cast(handle: &GameHandle, foe: &mut Client) {
    let (id, decision) = foe.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let spell = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    answer(handle, foe, id, spell).await;
    let (id, _) = foe.decision().await;
    answer(handle, foe, id, Answer::PayMana(PayChoice::Auto)).await;
}

/// What the messages say, in order: a decision by its kind, a yield notice, and a run of event batches once.
fn order(messages: &[ServerMessage]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for message in messages {
        let now = match message {
            ServerMessage::Decision { decision, .. } => {
                format!("decision {}", decision.kind.name())
            }
            ServerMessage::Yielding { until } => format!("yield {until:?}"),
            ServerMessage::Events(_) => "events".into(),
            _ => continue,
        };
        if now != "events" || out.last().is_none_or(|last| last != "events") {
            out.push(now);
        }
    }
    out
}

/// The decisions sent after the notice that the yield ended, which must be there, and none before it.
fn decided_after_the_end(messages: &[ServerMessage]) -> Vec<String> {
    let said = order(messages);
    let end = said
        .iter()
        .position(|m| m == "yield None")
        .expect("the yield ended");
    assert!(
        said[..end].iter().all(|m| !m.starts_with("decision")),
        "{said:?}"
    );
    said[end..]
        .iter()
        .filter(|m| m.starts_with("decision"))
        .cloned()
        .collect()
}

/// The seat's answer to `decision`: the first opponent as a target, else the passive answer.
fn answer_to(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::ChooseTargets(_) => {
            Answer::ChooseTargets(TargetChoice::Targets(vec![Target::Player(seat(0))]))
        }
        _ => passive(decision),
    }
}

fn decisions(messages: &[ServerMessage]) -> Vec<(DecisionId, Decision)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Decision { id, decision, .. } => Some((*id, decision.clone())),
            _ => None,
        })
        .collect()
}

fn priority_decisions(messages: &[ServerMessage]) -> usize {
    decisions(messages)
        .iter()
        .filter(|(_, d)| matches!(d.kind, DecisionKind::Priority(_)))
        .count()
}

/// The turn number of the last event batch among `messages`.
fn last_turn(messages: &[ServerMessage]) -> u32 {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Events(batch) => Some(batch.table.turn.number),
            _ => None,
        })
        .expect("an event batch")
}

fn passes_by(log: &[Event], player: usize) -> usize {
    log.iter()
        .filter(|e| matches!(e, Event::PriorityPassed { player: p } if p.index() == player))
        .count()
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
#[tokio::test(start_paused = true)]
async fn it_passes_every_window_of_the_other_turns_over_spells_and_stops_and_ends_as_the_own_turn_begins(
    #[case] seats: usize,
) {
    // The last seat holds an instant, so it could act in every window, and stops in the opponents' main phases.
    let me = seats - 1;
    let setup = Scenario::new(seats)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(me, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap();
    let (handle, store) = spawn(setup);
    let mut clients = join_and_ready(&handle, seats).await;
    for step in [Step::Main1, Step::Main2, Step::End] {
        let stop = ClientMessage::SetStop {
            side: StopSide::Opponent,
            step,
            on: true,
            keep: true,
        };
        send(&handle, &clients[me], stop).await;
    }
    send(
        &handle,
        &clients[me],
        ClientMessage::Yield { until: THROUGH },
    )
    .await;
    clients[me].drain();

    cast(&handle, &mut clients[0]).await;
    drive_others(&handle, &mut clients, me).await;

    let told = clients[me].drain();
    assert_eq!(priority_decisions(&told), 1, "only the one in its own turn");
    assert_eq!(decided_after_the_end(&told), ["decision priority"]);
    assert_eq!(
        last_turn(&told),
        u32::try_from(seats).unwrap(),
        "its own first turn"
    );
    let log = store.inner().load(game_id(1)).await.unwrap();
    assert!(
        passes_by(&log, me) >= 4 * (seats - 1),
        "passed for over every turn"
    );
}

#[rstest]
#[case::its_upkeep_trigger_asks_for_a_target(true)]
#[case::its_first_window(false)]
#[tokio::test(start_paused = true)]
async fn it_is_off_from_the_untap_step_before_any_decision_of_the_own_turn(#[case] upkeep: bool) {
    let shock = aimed_at(
        trigger(when::your_upkeep(), vec![damage(Ref::Target(0), 1)]),
        Targetable::AnyTarget,
    );
    let mut scenario = Scenario::new(2).hand(1, [instant("My Zap", 0, &[])]);
    if upkeep {
        scenario = scenario.battlefield(1, [ready(creature_with("Pinger", vec![shock]))]);
    }
    let (handle, _) = spawn(scenario.setup().unwrap());
    let mut clients = join_and_ready(&handle, 2).await;
    send(
        &handle,
        &clients[1],
        ClientMessage::Yield { until: THROUGH },
    )
    .await;
    clients[1].drain();
    drive_others(&handle, &mut clients, 1).await;

    let told = clients[1].drain();
    let first = if upkeep {
        "decision choose-targets"
    } else {
        "decision priority"
    };
    assert_eq!(decided_after_the_end(&told), [first]);
    assert_eq!(last_turn(&told), 2);
}

/// What the seat that passes is asked in the middle of the opponent's turn.
#[derive(Debug, Clone, Copy)]
enum Ask {
    Blockers,
    Targets,
}

#[rstest]
#[case::blockers(Ask::Blockers)]
#[case::targets_of_its_own_trigger(Ask::Targets)]
#[tokio::test(start_paused = true)]
async fn a_real_choice_is_asked_and_the_pass_goes_on_after_it(#[case] ask: Ask) {
    let watcher = aimed_at(
        trigger(when::any_creature_enters(), vec![damage(Ref::Target(0), 1)]),
        Targetable::AnyTarget,
    );
    let setup = match ask {
        Ask::Blockers => Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Raider", 2, 2))])
            .battlefield(1, [ready(free_creature("Guard", 1, 3))]),
        Ask::Targets => Scenario::new(2)
            .hand(0, [free_creature("Raider", 2, 2)])
            .battlefield(1, [ready(creature_with("Watcher", vec![watcher]))]),
    };
    let (handle, _) = spawn(setup.setup().unwrap());
    let mut clients = join_and_ready(&handle, 2).await;
    send(
        &handle,
        &clients[1],
        ClientMessage::Yield { until: THROUGH },
    )
    .await;
    clients[1].drain();

    let (id, decision) = clients[0].decision().await;
    match ask {
        Ask::Blockers => {
            let DecisionKind::DeclareAttackers(options) = &decision.kind else {
                panic!("expected the attack, got {decision:?}");
            };
            let attack = vec![(options.attackers[0].object, AttackTarget::Player(seat(1)))];
            answer(&handle, &clients[0], id, Answer::DeclareAttackers(attack)).await;
        }
        Ask::Targets => {
            let DecisionKind::Priority(options) = &decision.kind else {
                panic!("expected priority, got {decision:?}");
            };
            let spell = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
            answer(&handle, &clients[0], id, spell).await;
            let (id, _) = clients[0].decision().await;
            answer(&handle, &clients[0], id, Answer::PayMana(PayChoice::Auto)).await;
        }
    }
    drive_others(&handle, &mut clients, 1).await;

    let told = clients[1].drain();
    let (id, decision) = decisions(&told).pop().expect("a decision");
    let kind = match ask {
        Ask::Blockers => "declare-blockers",
        Ask::Targets => "choose-targets",
    };
    assert_eq!(decision.kind.name(), kind);
    assert!(
        order(&told).iter().all(|m| !m.starts_with("yield")),
        "still on"
    );
    assert_eq!(priority_decisions(&told), 0);
    answer(&handle, &clients[1], id, answer_to(&decision)).await;

    drive_others(&handle, &mut clients, 1).await;
    let told = clients[1].drain();
    assert_eq!(priority_decisions(&told), 1, "its own turn");
    assert!(order(&told).contains(&"yield None".to_string()));
}

#[tokio::test(start_paused = true)]
async fn the_discard_of_its_own_turn_is_asked_and_the_pass_lasts_through_the_next_turn() {
    let setup = Scenario::new(2)
        .at(1, 1, Step::Main1)
        .hand(1, vec![instant("Spare", 0, &[]); 9])
        .setup()
        .unwrap();
    let (handle, _) = spawn(setup);
    let mut clients = join_and_ready(&handle, 2).await;
    let (_, first) = clients[1].decision().await;
    assert_eq!(first.kind.name(), "priority");
    send(
        &handle,
        &clients[1],
        ClientMessage::Yield { until: THROUGH },
    )
    .await;
    drive_others(&handle, &mut clients, 1).await;

    let told = clients[1].drain();
    let (id, discard) = decisions(&told).pop().expect("a decision");
    assert_eq!(discard.kind.name(), "discard");
    assert_eq!(
        order(&told).iter().filter(|m| *m == "yield None").count(),
        0
    );
    answer(&handle, &clients[1], id, passive(&discard)).await;

    drive_others(&handle, &mut clients, 1).await;
    let told = clients[1].drain();
    assert_eq!(last_turn(&told), 3, "through seat 0's turn to its next own");
    assert!(order(&told).contains(&"yield None".to_string()));
}

#[rstest]
#[case::through(THROUGH, true)]
#[case::my_turn(YieldUntil::MyTurn, false)]
#[tokio::test(start_paused = true)]
async fn an_action_of_the_seat_ends_only_the_pass_until_its_turn(
    #[case] until: YieldUntil,
    #[case] ends: bool,
) {
    let (handle, _) = spawn(Scenario::new(2).setup().unwrap());
    let mut clients = join_and_ready(&handle, 2).await;
    send(&handle, &clients[1], ClientMessage::Yield { until }).await;
    clients[1].drain();
    send(&handle, &clients[1], ClientMessage::TakeBack).await;
    let told = clients[1].drain();
    assert_eq!(order(&told).contains(&"yield None".to_string()), ends);
}

#[rstest]
#[tokio::test]
async fn a_game_where_a_seat_passes_until_its_turn_every_turn_replays_the_same(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), config).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        let mut bot = Bot::join(&handle, n, seed).await;
        if n == 0 {
            bot.yield_on_pass = Some(THROUGH);
        }
        bots.push(bot);
    }
    let reports = common::play_bots(&handle, bots).await;
    let status = handle.status().await.unwrap();
    let result = status.result.expect("the game ended");
    assert!(reports.iter().all(|r| r.result == result));
    assert!(reports.iter().all(|r| r.errors.is_empty()), "{reports:?}");
    assert!(reports[0].yields > 0);

    let log = store.load(game_id(1)).await.unwrap();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        status.state_hash.unwrap()
    );
    assert!(passes_by(&log, 0) > reports[0].yields);
}
