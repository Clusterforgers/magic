//! Goal: a bot always has a legal answer: the table's fallback a bot falls back on is accepted by the engine at every decision of seeded games (and is never the removal of a trigger, which only the clock may give), and in a 4-seat team game a bot whose brain runs past its deadline, or answers illegally, at the target of a stolen Flickerwisp's trigger (returned by Puppeteer Clique) answers legally and plays on without conceding.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use common::{Client, game_id, roomy};
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::TargetChoice;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Engine, Event, GameSetup};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::bots::{
    BotBrain, BotLevel, BotPace, BotSeat, Thinkers, fallback, safe_answer, spawn_bot_seat_with,
    tries,
};
use mtg_server::registry::Removal;
use mtg_server::{EventStore, MemoryStore, spawn_game_with};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::{bear, giant, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::decks::{
    fuzz_setup, pick_fuzz_setup, spell_fuzz_setup, symbol_fuzz_setup, trigger_fuzz_setup,
};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use tokio::sync::watch;

/// The most decisions one seeded game is played for.
const DECISIONS: usize = 1500;

/// A seeded table's setup.
type Setup = fn(usize, u64) -> GameSetup;

fn picked(seats: usize, seed: u64) -> GameSetup {
    pick_fuzz_setup(seats, seed).unwrap()
}

#[rstest]
fn the_fallback_is_accepted_at_every_decision(
    #[values(
        fuzz_setup,
        spell_fuzz_setup,
        trigger_fuzz_setup,
        symbol_fuzz_setup,
        picked
    )]
    setup: Setup,
    #[values(2, 4)] seats: usize,
    #[values(1, 2)] seed: u64,
) {
    let mut engine = Engine::new(setup(seats, seed)).unwrap();
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..seats)
        .map(|seat| brain(Level::Easy, seed, PlayerId::from_index(seat), false))
        .collect();
    let mut kinds = BTreeSet::new();
    for _ in 0..DECISIONS {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        if engine.result().is_some() {
            break;
        }
        kinds.insert(decision.kind.name());
        let answer = fallback(&decision);
        assert!(
            !matches!(answer, Some(Answer::ChooseTargets(TargetChoice::Remove))),
            "a seat may not remove a trigger: {decision:?}"
        );
        match answer {
            Some(answer) => {
                let mut tried = engine.clone();
                if let Err(refused) = tried.apply(answer.clone()) {
                    panic!("fallback {answer:?} refused ({refused}) for {decision:?}");
                }
            }
            None => assert!(
                tries(&decision)
                    .into_iter()
                    .any(|answer| engine.clone().apply(answer).is_ok()),
                "no answer is accepted for {decision:?}"
            ),
        }
        let chosen = brains[decision.player.index()].choose(&decision, Some(&engine));
        if engine.apply(chosen).is_err() {
            engine.apply(safe_answer(&decision)).unwrap();
        }
    }
    assert!(kinds.len() > 3, "too few kinds asked: {kinds:?}");
}

/// What seat 3's brain does at the stolen Flickerwisp's target.
#[derive(Clone, Copy, Debug)]
enum AtTarget {
    /// It thinks past the deadline.
    Hangs,
    /// It answers at once with no target, which the table refuses from a seat.
    Removes,
}

/// Seat 3: casts what it can, pays automatically, aims Puppeteer Clique at the graveyard and fails at the Flickerwisp
/// trigger's target as `at_target` says; the table's safe answer for everything else.
struct Thief {
    at_target: AtTarget,
    reached: Arc<AtomicBool>,
}

impl SeatBrain for Thief {
    fn choose(&mut self, decision: &Decision, _table: Option<&Engine>) -> Answer {
        match &decision.kind {
            DecisionKind::Priority(options) => match options.castable.first() {
                Some(id) => Answer::Priority(PriorityAction::CastSpell(*id)),
                None => Answer::Priority(PriorityAction::Pass),
            },
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) if !options.wording.contains("graveyard") => {
                self.reached.store(true, Ordering::SeqCst);
                if matches!(self.at_target, AtTarget::Hangs) {
                    std::thread::sleep(Duration::from_millis(800));
                }
                Answer::ChooseTargets(TargetChoice::Remove)
            }
            _ => safe_answer(decision),
        }
    }
}

/// Four seats in two teams (seat 3 with seat 1), each with lands and creatures; seat 3 holds Puppeteer Clique in its
/// main phase and seat 0's graveyard holds Flickerwisp.
fn stolen_flicker() -> GameSetup {
    let mut scenario = Scenario::new(4)
        .house_rules(HouseRules {
            teams: Some(Teams::even(TeamMode::Teams, 2, 2)),
            ..HouseRules::default()
        })
        .at(3, 3, Step::Main1);
    for seat in 0..4 {
        let mut permanents = vec![ready(island()), ready(island())];
        permanents.extend((0..3).map(|_| ready(swamp())));
        permanents.extend((0..6).flat_map(|_| [ready(bear()), ready(giant())]));
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
        .hand(3, [card("Puppeteer Clique").unwrap()])
        .graveyard(0, [card("Flickerwisp").unwrap()])
        .setup()
        .unwrap()
}

#[rstest]
#[case::past_its_deadline(AtTarget::Hangs)]
#[case::refused(AtTarget::Removes)]
#[tokio::test]
async fn a_bot_failing_at_a_stolen_flickerwisp_target_answers_legally_and_plays_on(
    #[case] at_target: AtTarget,
) {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), stolen_flicker(), store.clone(), roomy()).unwrap();
    let reached = Arc::new(AtomicBool::new(false));
    let thief = Thief {
        at_target,
        reached: reached.clone(),
    };
    let bot = BotSeat {
        seat: PlayerId(3),
        level: BotLevel::Easy,
    };
    let first = BotBrain::new(3, bot.seat, bot.level)
        .with_brain(Box::new(thief))
        .with_deadline(Duration::from_millis(200));
    let (_alive, removed) = watch::channel(Removal::Dropped);
    let pace = BotPace {
        delay: Duration::from_millis(5),
        thinkers: Thinkers::new(2),
    };
    let _bot = spawn_bot_seat_with(handle.clone(), bot, 3, first, pace, removed);
    let mut clients = Vec::new();
    for seat in 0..3 {
        let client = Client::join(&handle, seat).await;
        client.say(&handle, ClientMessage::Ready).await;
        clients.push(client);
    }
    let watcher = clients.remove(0);
    for client in clients {
        let handle = handle.clone();
        tokio::spawn(answer_passively(handle, client, None));
    }
    tokio::time::timeout(
        Duration::from_mins(1),
        answer_passively(handle.clone(), watcher, Some(4)),
    )
    .await
    .expect("the table did not reach seat 0's turn");
    assert!(
        reached.load(Ordering::SeqCst),
        "the Flickerwisp target was never asked"
    );
    let log = store.load(game_id(1)).await.unwrap();
    let lost: Vec<&Event> = log
        .iter()
        .filter(|event| matches!(event, Event::PlayerLost { .. }))
        .collect();
    assert!(lost.is_empty(), "a seat left the game: {lost:?}");
}

/// Answers `client`'s decisions passively until the game ends or, with `until`, the table reaches that turn.
async fn answer_passively(handle: mtg_server::GameHandle, mut client: Client, until: Option<u32>) {
    while let Some(message) = client.inbox.recv().await {
        match message {
            ServerMessage::Decision { id, decision, .. } => {
                let answer = common::passive(&decision);
                let message = ClientMessage::Answer {
                    decision: id,
                    answer,
                };
                if handle.send(client.seat, message).await.is_err() {
                    return;
                }
            }
            ServerMessage::Events(batch)
                if until.is_some_and(|turn| batch.table.turn.number >= turn) =>
            {
                return;
            }
            ServerMessage::GameOver { .. } => return,
            _ => {}
        }
    }
}

#[rstest]
#[case::hard(Level::Hard)]
#[case::very_hard(Level::VeryHard)]
fn a_searching_brain_asked_late_answers_the_stolen_flickerwisp_target_within_the_cap(
    #[case] level: Level,
) {
    let mut driver = mtg_testkit::Driver::new(Engine::new(stolen_flicker()).unwrap()).unwrap();
    driver.cast("Puppeteer Clique").unwrap();
    let decision = loop {
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::ChooseTargets(options) = &decision.kind
            && !options.wording.contains("graveyard")
        {
            break decision;
        }
        driver.answer(safe_answer(&decision)).unwrap();
    };
    let mut searching = brain(level, 7, decision.player, true);
    let waited = Duration::from_secs(4);
    let asked = std::time::Instant::now().checked_sub(waited).unwrap();
    searching.set_asked(asked);
    let answer = searching.choose(&decision, Some(driver.engine()));
    let took = asked.elapsed().saturating_sub(waited);
    assert!(
        took < Duration::from_millis(2500),
        "the brain took {took:?} past the wait"
    );
    driver.answer(answer).unwrap();
}
