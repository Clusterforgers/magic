//! Goal: a seat that is not the active player has the response clock on the decisions the effects of spells and abilities ask it (the targets of a trigger or a spell, the order of its triggers) and the active player has none; at the deadline the server answers through the normal path with the safe answer (no target, which removes a trigger and takes a spell back; the triggers in the order they triggered), a late answer is stale, and an answer in time stops the clock.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Client, game_id, roomy, seat};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, TargetChoice, TriggersChoice};
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Decision, DecisionKind, Event, GameSetup, GameState};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, GameStatus, Pace, spawn_game_with};
use mtg_testkit::cards::{burn, damage, free_creature, gain_life, mountain};
use mtg_testkit::trigger_cards::{aimed_at, creature_with, trigger, watcher, when};
use mtg_testkit::{Scenario, ready};

const RESPONSE: Duration = Duration::from_secs(10);

/// The server defaults: auto-pass on, 10 s to respond, 45 s to block, no dwell.
const DEFAULT: Pace = Pace {
    auto_pass: true,
    response: Some(RESPONSE),
    blocks: Some(Duration::from_secs(45)),
    dwell: Dwell::OFF,
};

fn spawn(setup: GameSetup) -> (GameHandle, common::FlakyStore) {
    let store = common::FlakyStore::new();
    let config = GameConfig {
        pace: DEFAULT,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    (handle, store)
}

fn creatures() -> Targetable {
    Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)]))
}

/// Seat 0 holds a Bolt and a Mountain and burns the Victim of seat 1, which has these `watchers` as well.
fn burn_table(watchers: Vec<std::sync::Arc<mtg_cards::CardDefinition>>) -> GameSetup {
    let mut permanents = vec![ready(free_creature("Victim", 1, 1))];
    permanents.extend(watchers.into_iter().map(ready));
    Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, permanents)
        .setup()
        .unwrap()
}

/// A Seer whose trigger is aimed at a creature: when any creature dies it deals 1 damage to a target creature.
fn seer(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    creature_with(
        name,
        vec![aimed_at(
            trigger(when::any_creature_dies(), vec![damage(Ref::Target(0), 1)]),
            creatures(),
        )],
    )
}

fn object_named(setup: &GameSetup, name: &str) -> ObjectId {
    let state = GameState::from_setup(setup).unwrap();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

async fn decision_of(client: &mut Client) -> (DecisionId, Decision, Option<u64>) {
    client
        .until(|m| match m {
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
            } => Some((*id, decision.clone(), *deadline_ms)),
            _ => None,
        })
        .await
}

async fn reply(handle: &GameHandle, from: &Client, id: DecisionId, answer: Answer) {
    from.say(
        handle,
        ClientMessage::Answer {
            decision: id,
            answer,
        },
    )
    .await;
}

async fn moved_past(handle: &GameHandle, events: u64) -> GameStatus {
    for _ in 0..100 {
        let status = handle.status().await.unwrap();
        if status.events > events {
            return status;
        }
        tokio::task::yield_now().await;
    }
    panic!("the game never got past {events} events");
}

async fn quiet(handle: &GameHandle) -> GameStatus {
    for _ in 0..20 {
        tokio::task::yield_now().await;
        handle.status().await.unwrap();
    }
    handle.status().await.unwrap()
}

/// Both seats join and say they are ready.
async fn join_and_ready(handle: &GameHandle) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(handle, n).await);
    }
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

/// Seat 0 casts its Bolt at the Victim; the Victim dies and whatever it set off is put on the stack.
async fn burn_the_victim(handle: &GameHandle, clients: &mut [Client], victim: ObjectId) {
    let (id, decision, _) = decision_of(&mut clients[0]).await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 0 holds priority");
    };
    let bolt = options.castable[0];
    reply(
        handle,
        &clients[0],
        id,
        Answer::Priority(PriorityAction::CastSpell(bolt)),
    )
    .await;
    let (id, decision, left) = decision_of(&mut clients[0]).await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("the spell asks for its target, got {decision:?}");
    };
    assert_eq!(left, None, "the active player is not hurried");
    let victim = options
        .legal
        .iter()
        .copied()
        .find(|t| matches!(t, Target::Object(r) if r.id == victim))
        .unwrap();
    reply(
        handle,
        &clients[0],
        id,
        Answer::ChooseTargets(TargetChoice::Targets(vec![victim])),
    )
    .await;
    let (id, _, _) = decision_of(&mut clients[0]).await;
    reply(handle, &clients[0], id, Answer::PayMana(PayChoice::Auto)).await;
    // Nobody has anything else to do, so the table passes for both seats and the Bolt resolves.
}

fn is_stale(messages: &[ServerMessage]) -> bool {
    matches!(
        messages,
        [ServerMessage::Error {
            code: ErrorCode::StaleDecision,
            ..
        }]
    )
}

#[tokio::test(start_paused = true)]
async fn the_targets_of_a_trigger_of_a_seat_that_is_not_active_run_on_the_response_clock_and_the_expiry_removes_it()
 {
    let setup = burn_table(vec![seer("Seer")]);
    let victim = object_named(&setup, "Victim");
    let (handle, store) = spawn(setup);
    let mut clients = join_and_ready(&handle).await;
    burn_the_victim(&handle, &mut clients, victim).await;

    let (id, decision, left) = decision_of(&mut clients[1]).await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("seat 1 chooses the target of its trigger, got {decision:?}");
    };
    assert!(options.triggered && !options.can_cancel);
    assert_eq!(left, Some(10_000), "a clock like a priority window's");
    let events = handle.status().await.unwrap().events;
    clients[1].drain();

    tokio::time::advance(Duration::from_secs(9)).await;
    assert_eq!(quiet(&handle).await.events, events, "a second early");
    tokio::time::advance(Duration::from_secs(1)).await;
    moved_past(&handle, events).await;
    let logged = store.inner().load(game_id(1)).await.unwrap();
    let removed = &logged[usize::try_from(events).unwrap()..];
    assert!(
        matches!(removed.first(), Some(Event::TriggerRemoved { .. })),
        "no target chosen, so the trigger is removed: {removed:?}"
    );
    assert!(
        !removed
            .iter()
            .any(|e| matches!(e, Event::AbilityPutOnStack(put) if put.cause.is_some())),
        "it never went on the stack"
    );

    // The seat's late answer is the ordinary stale one.
    clients[1].drain();
    reply(
        &handle,
        &clients[1],
        id,
        Answer::ChooseTargets(TargetChoice::Remove),
    )
    .await;
    handle.status().await.unwrap();
    assert!(is_stale(&clients[1].drain()));
}

#[tokio::test(start_paused = true)]
async fn choosing_the_target_in_time_stops_the_clock() {
    let setup = burn_table(vec![seer("Seer")]);
    let victim = object_named(&setup, "Victim");
    let (handle, store) = spawn(setup);
    let mut clients = join_and_ready(&handle).await;
    burn_the_victim(&handle, &mut clients, victim).await;
    let (id, decision, _) = decision_of(&mut clients[1]).await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("seat 1 chooses the target of its trigger");
    };
    tokio::time::advance(Duration::from_secs(9)).await;
    reply(
        &handle,
        &clients[1],
        id,
        Answer::ChooseTargets(TargetChoice::Targets(vec![options.legal[0]])),
    )
    .await;
    let status = quiet(&handle).await;
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert!(
        logged
            .iter()
            .any(|e| matches!(e, Event::AbilityPutOnStack(put) if put.cause.is_some() && put.controller == seat(1))),
        "the trigger went on the stack"
    );
    // Whatever the game waits for now has a clock of its own, started by the answer: where the old one would have run out
    // nothing happens.
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(quiet(&handle).await, status);
}

#[tokio::test(start_paused = true)]
async fn a_seat_cannot_remove_its_own_trigger_while_a_target_is_legal() {
    let setup = burn_table(vec![seer("Seer")]);
    let victim = object_named(&setup, "Victim");
    let (handle, _store) = spawn(setup);
    let mut clients = join_and_ready(&handle).await;
    burn_the_victim(&handle, &mut clients, victim).await;
    let (id, _, _) = decision_of(&mut clients[1]).await;
    let events = handle.status().await.unwrap().events;
    clients[1].drain();

    reply(
        &handle,
        &clients[1],
        id,
        Answer::ChooseTargets(TargetChoice::Remove),
    )
    .await;
    assert_eq!(quiet(&handle).await.events, events, "nothing happened");
    assert!(
        matches!(
            clients[1].drain().as_slice(),
            [ServerMessage::Error {
                code: ErrorCode::IllegalAnswer,
                ..
            }]
        ),
        "the seat is told the answer is illegal"
    );

    // The decision stays pending: the clock still removes the trigger when it runs out.
    tokio::time::advance(Duration::from_secs(10)).await;
    moved_past(&handle, events).await;
}

#[tokio::test(start_paused = true)]
async fn the_order_of_triggers_runs_on_the_clock_and_the_expiry_keeps_the_order_they_triggered() {
    let first = watcher("First", when::any_creature_dies(), vec![gain_life(1)]);
    let second = watcher("Second", when::any_creature_dies(), vec![gain_life(10)]);
    let setup = burn_table(vec![first, second]);
    let victim = object_named(&setup, "Victim");
    let (handle, store) = spawn(setup);
    let mut clients = join_and_ready(&handle).await;
    burn_the_victim(&handle, &mut clients, victim).await;

    let (id, decision, left) = decision_of(&mut clients[1]).await;
    let DecisionKind::OrderTriggers(options) = &decision.kind else {
        panic!("seat 1 orders its two triggers, got {decision:?}");
    };
    assert_eq!(options.triggers.len(), 2);
    assert_eq!(left, Some(10_000));
    let events = handle.status().await.unwrap().events;

    tokio::time::advance(RESPONSE).await;
    moved_past(&handle, events).await;
    let logged = store.inner().load(game_id(1)).await.unwrap();
    let after = &logged[usize::try_from(events).unwrap()..];
    assert!(
        matches!(
            after.first(),
            Some(Event::TriggersOrdered { player, order }) if *player == seat(1) && order == &[0, 1]
        ),
        "{after:?}"
    );
    clients[1].drain();
    reply(
        &handle,
        &clients[1],
        id,
        Answer::OrderTriggers(TriggersChoice(vec![1, 0])),
    )
    .await;
    handle.status().await.unwrap();
    assert!(is_stale(&clients[1].drain()));
}

#[tokio::test(start_paused = true)]
async fn the_active_player_is_not_hurried_to_choose_the_targets_of_its_trigger() {
    let setup = Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain()), ready(seer("Seer"))])
        .battlefield(1, [ready(free_creature("Victim", 1, 1))])
        .setup()
        .unwrap();
    let victim = object_named(&setup, "Victim");
    let (handle, _) = spawn(setup);
    let mut clients = join_and_ready(&handle).await;
    burn_the_victim(&handle, &mut clients, victim).await;
    let (_, decision, left) = decision_of(&mut clients[0]).await;
    assert!(matches!(decision.kind, DecisionKind::ChooseTargets(_)));
    assert_eq!(left, None);
    let status = handle.status().await.unwrap();
    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(quiet(&handle).await, status, "an hour later it still waits");
}

#[tokio::test(start_paused = true)]
async fn the_targets_of_a_spell_cast_by_a_seat_that_is_not_active_run_on_the_clock_and_the_expiry_takes_the_cast_back()
 {
    // Seat 1 is in Full Control, so it is asked at seat 0's main phase, where it begins to cast a Bolt and sits on it.
    let setup = Scenario::new(2)
        .hand(1, [burn("Bolt", 3)])
        .battlefield(0, [ready(free_creature("Target", 1, 1))])
        .battlefield(1, [ready(mountain())])
        .setup()
        .unwrap();
    let (handle, store) = spawn(setup);
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(&handle, n).await);
    }
    clients[1]
        .say(
            &handle,
            ClientMessage::FullControl {
                mode: FullControlMode::Held,
            },
        )
        .await;
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    let (window, decision, left) = decision_of(&mut clients[1]).await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 1 is asked about priority in seat 0's turn");
    };
    assert_eq!(left, Some(10_000));
    let bolt = options.castable[0];
    reply(
        &handle,
        &clients[1],
        window,
        Answer::Priority(PriorityAction::CastSpell(bolt)),
    )
    .await;
    let (id, decision, left) = decision_of(&mut clients[1]).await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("the Bolt asks for its target, got {decision:?}");
    };
    assert!(options.can_cancel && !options.triggered);
    assert_eq!(left, Some(10_000), "the clock of a priority window");
    let events = handle.status().await.unwrap().events;

    tokio::time::advance(RESPONSE).await;
    let status = quiet(&handle).await;
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert!(
        !logged.iter().any(|e| matches!(e, Event::SpellCast { .. })),
        "the cast was taken back and left nothing in the log"
    );
    assert_eq!(status.events, events, "no event: a cancel is free");
    // Seat 1 is asked about priority again, and a late answer to the old question is stale.
    let (again, decision, _) = decision_of(&mut clients[1]).await;
    assert!(again > id);
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    clients[1].drain();
    reply(
        &handle,
        &clients[1],
        id,
        Answer::ChooseTargets(TargetChoice::Cancel),
    )
    .await;
    handle.status().await.unwrap();
    assert!(is_stale(&clients[1].drain()));
}
