//! Goal: through the actor, on tokio's paused time, every question a seat is asked that the clocks cover (yes or no, an option, modes, a number, a division, a payment, the targets of a spell, keeping a hand and bottoming) behaves the same: the clock starts when the question is sent and carries its length, an answer in time wins and no clock answers again, at the deadline the table plays the one documented fallback exactly once through the normal path (logged, saved like the seat's own answer, a late answer stale), a reconnect is told the time that is left and the deadline does not move, and every other seat is sent the messages it would have been sent had the player answered with the fallback itself.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use common::{Client, FlakyStore, capture_logs, game_id, roomy, table_setup};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{
    DivideChoice, ModeChoice, NumberChoice, OptionChoice, Target, TargetChoice, YesNoChoice,
};
use mtg_engine::decision::{MulliganChoice, PayChoice, PriorityAction};
use mtg_engine::{Answer, Decision, DecisionKind, Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, GameId, Pace, spawn_game_with};
use mtg_testkit::announce_cards::split_bolt;
use mtg_testkit::cards::{burn, free_creature, giant, instant, island, mountain};
use mtg_testkit::choice_cards::{charm, gift, scorn};
use mtg_testkit::x_cards::stroke;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// The server defaults: auto-pass on, 10 s to respond, 45 s to block, 60 s to think, no dwell.
const DEFAULT: Pace = Pace {
    auto_pass: true,
    response: Some(Duration::from_secs(10)),
    blocks: Some(Duration::from_secs(45)),
    think: Some(Duration::from_secs(60)),
    active: None,
    extension: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

/// The response clock and the think clock in milliseconds.
const RESPONSE: u64 = 10_000;
const THINK: u64 = 60_000;

/// One question: how to reach it, which clock it runs on and what the table answers when the clock runs out.
#[derive(Clone, Copy, Debug)]
struct Case {
    /// The name of the decision kind ([`DecisionKind::name`]).
    name: &'static str,
    seats: usize,
    /// The seat that casts the spell whose question this is, or any seat for a question of the opening.
    caster: usize,
    /// The seat the question is put to; the first seat asked for a question of the opening.
    asked: Option<usize>,
    /// The clock the question is sent with, in milliseconds.
    clock: u64,
    setup: fn() -> GameSetup,
    /// What the table answers at the deadline, written out by hand (the expected answer, not the code under test).
    fallback: fn(&Decision) -> Answer,
    /// An answer in time, a different one where the question has a different legal answer.
    on_time: fn(&Decision) -> Answer,
}

/// A table of `seats` whose libraries have cards to draw.
fn stocked(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats).seed(7), |scenario, seat| {
        scenario.library(seat, vec![giant(); 30])
    })
}

fn yes_no() -> Case {
    Case {
        name: "yes-no",
        seats: 2,
        caster: 0,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(2)
                .hand(0, [gift("Gift")])
                .battlefield(0, [ready(island())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::YesNo(YesNoChoice(false)),
        on_time: |_| Answer::YesNo(YesNoChoice(true)),
    }
}

fn choose_option() -> Case {
    Case {
        name: "choose-option",
        seats: 3,
        caster: 1,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(3)
                .hand(1, [scorn("Scorn")])
                .battlefield(1, [ready(mountain())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::ChooseOption(OptionChoice(0)),
        on_time: |_| Answer::ChooseOption(OptionChoice(1)),
    }
}

fn choose_mode() -> Case {
    Case {
        name: "choose-mode",
        seats: 2,
        caster: 1,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(2)
                .hand(1, [charm("Charm", 1, 1, false)])
                .battlefield(1, [ready(mountain()), ready(mountain())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::ChooseMode(ModeChoice::Cancel),
        on_time: |decision| {
            let DecisionKind::ChooseMode(options) = &decision.kind else {
                panic!("a mode question, got {decision:?}");
            };
            Answer::ChooseMode(ModeChoice::Modes(vec![options.modes[1].index]))
        },
    }
}

fn choose_number() -> Case {
    Case {
        name: "choose-number",
        seats: 2,
        caster: 1,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(2)
                .hand(1, [stroke().unwrap()])
                .battlefield(1, [ready(island()), ready(island()), ready(island())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::ChooseNumber(NumberChoice::Cancel),
        on_time: |_| Answer::ChooseNumber(NumberChoice::Number(0)),
    }
}

fn divide_amount() -> Case {
    Case {
        name: "divide-amount",
        seats: 2,
        caster: 1,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(2)
                .hand(1, [split_bolt("Split", 2)])
                .battlefield(0, [ready(free_creature("Target", 3, 3))])
                .battlefield(1, [ready(mountain()), ready(mountain())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::DivideAmount(DivideChoice::Cancel),
        on_time: |decision| {
            let DecisionKind::DivideAmount(options) = &decision.kind else {
                panic!("a division, got {decision:?}");
            };
            let mut shares: Vec<(Target, u32)> = options.among.iter().map(|t| (*t, 1)).collect();
            let others = u32::try_from(shares.len() - 1).unwrap();
            shares[0].1 = options.total - others;
            Answer::DivideAmount(DivideChoice::Division(shares))
        },
    }
}

fn pay_mana() -> Case {
    Case {
        name: "pay-mana",
        seats: 2,
        caster: 1,
        asked: Some(1),
        clock: THINK,
        setup: || {
            stocked(2)
                .hand(1, [instant("Zap", 0, &[Color::Red])])
                .battlefield(1, [ready(mountain())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::PayMana(PayChoice::Cancel),
        on_time: |_| Answer::PayMana(PayChoice::Auto),
    }
}

fn spell_targets() -> Case {
    Case {
        name: "choose-targets",
        seats: 2,
        caster: 1,
        asked: Some(1),
        clock: RESPONSE,
        setup: || {
            stocked(2)
                .hand(1, [burn("Bolt", 3)])
                .battlefield(0, [ready(free_creature("Target", 1, 1))])
                .battlefield(1, [ready(mountain())])
                .setup()
                .unwrap()
        },
        fallback: |_| Answer::ChooseTargets(TargetChoice::Cancel),
        on_time: |decision| {
            let DecisionKind::ChooseTargets(options) = &decision.kind else {
                panic!("a choice of targets, got {decision:?}");
            };
            Answer::ChooseTargets(TargetChoice::Targets(vec![options.legal[0]]))
        },
    }
}

fn keep_or_mulligan() -> Case {
    Case {
        name: "mulligan",
        seats: 2,
        caster: 0,
        asked: None,
        clock: THINK,
        setup: || table_setup(2, 5),
        fallback: |_| Answer::Mulligan(MulliganChoice::Keep),
        on_time: |_| Answer::Mulligan(MulliganChoice::Keep),
    }
}

fn bottoming() -> Case {
    Case {
        name: "bottom-cards",
        seats: 2,
        caster: 0,
        asked: None,
        clock: THINK,
        setup: || table_setup(2, 5),
        fallback: |decision| {
            let DecisionKind::BottomCards(options) = &decision.kind else {
                panic!("a bottoming, got {decision:?}");
            };
            Answer::BottomCards(options.hand.iter().copied().take(options.count).collect())
        },
        on_time: |decision| {
            let DecisionKind::BottomCards(options) = &decision.kind else {
                panic!("a bottoming, got {decision:?}");
            };
            let skip = options.hand.len() - options.count;
            Answer::BottomCards(options.hand.iter().copied().skip(skip).collect())
        },
    }
}

/// What the other seats and the log held after the question was dealt with.
struct Played {
    log: Vec<Event>,
    /// Everything each seat was sent, in order, whatever it read.
    seen: Vec<Vec<ServerMessage>>,
    /// Times the table logged that it answered for the asked seat.
    answered_for: usize,
    /// The seat the question was put to.
    asked: usize,
}

struct Asked {
    id: DecisionId,
    seat: usize,
    decision: Decision,
    left: Option<u64>,
}

struct Table {
    id: GameId,
    handle: GameHandle,
    store: FlakyStore,
    clients: Vec<Client>,
    /// Everything each seat was sent so far.
    seen: Vec<Vec<ServerMessage>>,
}

fn next_id() -> GameId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    game_id(u128::from(NEXT.fetch_add(1, Ordering::SeqCst)) + 0x5100_0000)
}

impl Table {
    /// Starts the game: every seat but the first (the active player) asks for Full Control, so it is asked at every
    /// window of the first seat's turn and can cast an instant there.
    async fn start(case: &Case) -> Table {
        capture_logs();
        let id = next_id();
        let store = FlakyStore::new();
        let config = GameConfig {
            pace: DEFAULT,
            ..roomy()
        };
        let handle = spawn_game_with(id, (case.setup)(), store.clone(), config).unwrap();
        let mut clients = Vec::new();
        for seat in 0..case.seats {
            clients.push(Client::join(&handle, seat).await);
        }
        for client in clients.iter().skip(1) {
            let mode = FullControlMode::Held;
            client
                .say(&handle, ClientMessage::FullControl { mode })
                .await;
        }
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
        handle.status().await.unwrap();
        Table {
            id,
            handle,
            store,
            seen: vec![Vec::new(); case.seats],
            clients,
        }
    }

    /// The next decision `seat` is sent; the decision is already queued when this is called.
    async fn decision(&mut self, seat: usize) -> (DecisionId, Decision, Option<u64>) {
        loop {
            let message = self.clients[seat].next().await;
            self.seen[seat].push(message.clone());
            if let ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
                ..
            } = message
            {
                return (id, decision, deadline_ms);
            }
        }
    }

    async fn reply(&self, seat: usize, id: DecisionId, answer: Answer) {
        self.clients[seat]
            .say(
                &self.handle,
                ClientMessage::Answer {
                    decision: id,
                    answer,
                },
            )
            .await;
    }

    /// The opening asks every seat at once: the other seats give their routine answers until only `seat` is asked.
    async fn settle_others(&mut self, case: &Case, seat: usize) {
        loop {
            self.handle.status().await.unwrap();
            let mut answered = false;
            for other in (0..case.seats).filter(|other| *other != seat) {
                let messages = self.clients[other].drain();
                self.seen[other].extend(messages.iter().cloned());
                let last = messages.into_iter().rev().find_map(|m| match m {
                    ServerMessage::Decision { id, decision, .. } => Some((id, decision)),
                    _ => None,
                });
                if let Some((id, decision)) = last {
                    let answer = routine(case, other, &decision);
                    self.reply(other, id, answer).await;
                    answered = true;
                }
            }
            if !answered {
                return;
            }
        }
    }

    /// Plays the routine answers until `case`'s question is put to its seat.
    async fn reach(&mut self, case: &Case) -> Asked {
        for _ in 0..60 {
            let seat = self
                .handle
                .status()
                .await
                .unwrap()
                .waiting_on
                .expect("the game waits for someone")
                .index();
            let (id, decision, left) = self.decision(seat).await;
            if decision.kind.name() == case.name && case.asked.is_none_or(|asked| asked == seat) {
                if matches!(
                    decision.kind,
                    DecisionKind::Mulligan(_) | DecisionKind::BottomCards(_)
                ) {
                    self.settle_others(case, seat).await;
                }
                return Asked {
                    id,
                    seat,
                    decision,
                    left,
                };
            }
            let answer = routine(case, seat, &decision);
            self.reply(seat, id, answer).await;
            self.handle.status().await.unwrap();
        }
        panic!("the question {} was never asked", case.name);
    }

    /// Lets the actor and its timers run until nothing is left to do at this instant.
    async fn quiet(&self) {
        for _ in 0..20 {
            tokio::task::yield_now().await;
            self.handle.status().await.unwrap();
        }
    }

    /// Moves the paused clock by `millis`.
    async fn wait(&self, millis: u64) {
        tokio::time::advance(Duration::from_millis(millis)).await;
        self.quiet().await;
    }

    /// Reads what every seat has been sent and is still queued.
    fn collect(&mut self) {
        for (seat, client) in self.clients.iter_mut().enumerate() {
            self.seen[seat].extend(client.drain());
        }
    }

    /// How many times the table logged that its clock answered for `seat`'s `kind`.
    fn answered_for(&self, seat: usize, kind: &str) -> usize {
        let logs = capture_logs();
        let text = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
        let (game, seat) = (format!("game={}", self.id), format!("seat={seat}"));
        let kind = format!("kind=\"{kind}\"");
        text.lines()
            .filter(|line| line.contains("clock ran out"))
            .filter(|line| line.contains(&game) && line.contains(&seat) && line.contains(&kind))
            .count()
    }

    async fn finish(mut self, asked: &Asked, name: &str) -> Played {
        self.quiet().await;
        self.collect();
        Played {
            log: self.store.inner().load(self.id).await.unwrap(),
            answered_for: self.answered_for(asked.seat, name),
            asked: asked.seat,
            seen: self.seen.into_iter().map(without_game).collect(),
        }
    }
}

/// `messages` with the id of the game blanked, which is all that tells two runs of one game apart.
fn without_game(mut messages: Vec<ServerMessage>) -> Vec<ServerMessage> {
    for message in &mut messages {
        if let ServerMessage::Welcome { game, .. } = message {
            game.clear();
        }
    }
    messages
}

/// The routine answers on the way to a question: the caster casts what it holds and pays automatically, the targets are
/// the asked seat first, a hand is kept (a first hand of a seat that bottoms is sent back once) and nothing else is done.
fn routine(case: &Case, seat: usize, decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(spell) if seat == case.caster => PriorityAction::CastSpell(*spell),
            _ => PriorityAction::Pass,
        }),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseTargets(options) => {
            let first = Target::Player(PlayerId::from_index(case.asked.unwrap_or(0)));
            let mut legal = options.legal.clone();
            legal.sort_by_key(|target| *target != first);
            let most = usize::try_from(options.max).unwrap_or(1);
            Answer::ChooseTargets(TargetChoice::Targets(
                legal.into_iter().take(most).collect(),
            ))
        }
        DecisionKind::Mulligan(options) => Answer::Mulligan(
            if case.name == "bottom-cards" && options.mulligans_taken == 0 {
                MulliganChoice::Mulligan
            } else {
                MulliganChoice::Keep
            },
        ),
        DecisionKind::BottomCards(options) => {
            Answer::BottomCards(options.hand.iter().copied().take(options.count).collect())
        }
        other => panic!("no routine answer to a {} decision", other.name()),
    }
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

/// Plays to the question, then either lets the clock run out (`None`) or answers at once with `prompt`.
async fn play(case: &Case, prompt: Option<fn(&Decision) -> Answer>) -> Played {
    let mut table = Table::start(case).await;
    let asked = table.reach(case).await;
    match prompt {
        None => table.wait(case.clock).await,
        Some(answer) => {
            let answer = answer(&asked.decision);
            table.reply(asked.seat, asked.id, answer).await;
        }
    }
    table.finish(&asked, case.name).await
}

#[rstest]
#[case::yes_no(yes_no())]
#[case::choose_option(choose_option())]
#[case::choose_mode(choose_mode())]
#[case::choose_number(choose_number())]
#[case::divide_amount(divide_amount())]
#[case::pay_mana(pay_mana())]
#[case::spell_targets(spell_targets())]
#[case::keep_or_mulligan(keep_or_mulligan())]
#[case::bottoming(bottoming())]
#[tokio::test(start_paused = true)]
async fn the_clock_starts_when_the_question_is_sent_and_runs_the_length_of_its_kind(
    #[case] case: Case,
) {
    let mut table = Table::start(&case).await;
    let asked = table.reach(&case).await;
    assert_eq!(asked.left, Some(case.clock), "{}", case.name);
    let events = table.handle.status().await.unwrap().events;

    table.wait(case.clock - 1).await;
    assert_eq!(table.answered_for(asked.seat, case.name), 0, "not yet");
    assert_eq!(table.handle.status().await.unwrap().events, events);

    table.wait(1).await;
    assert_eq!(table.answered_for(asked.seat, case.name), 1, "now");
}

#[rstest]
#[case::yes_no(yes_no())]
#[case::choose_option(choose_option())]
#[case::choose_mode(choose_mode())]
#[case::choose_number(choose_number())]
#[case::divide_amount(divide_amount())]
#[case::pay_mana(pay_mana())]
#[case::spell_targets(spell_targets())]
#[case::keep_or_mulligan(keep_or_mulligan())]
#[case::bottoming(bottoming())]
#[tokio::test(start_paused = true)]
async fn an_answer_in_time_wins_and_the_clock_never_answers_again(#[case] case: Case) {
    let mut table = Table::start(&case).await;
    let asked = table.reach(&case).await;
    table.wait(case.clock - 1_000).await;
    let answer = (case.on_time)(&asked.decision);
    table.reply(asked.seat, asked.id, answer).await;
    table.quiet().await;
    let sent = table.clients[asked.seat].drain();
    assert!(
        !sent
            .iter()
            .any(|message| matches!(message, ServerMessage::Error { .. })),
        "the answer in time is accepted: {sent:?}"
    );

    table.wait(2 * case.clock).await;
    assert_eq!(
        table.answered_for(asked.seat, case.name),
        0,
        "the question was answered, so its clock has nothing left to answer"
    );
}

#[rstest]
#[case::yes_no(yes_no())]
#[case::choose_option(choose_option())]
#[case::choose_mode(choose_mode())]
#[case::choose_number(choose_number())]
#[case::divide_amount(divide_amount())]
#[case::pay_mana(pay_mana())]
#[case::spell_targets(spell_targets())]
#[case::keep_or_mulligan(keep_or_mulligan())]
#[case::bottoming(bottoming())]
#[tokio::test(start_paused = true)]
async fn at_the_deadline_the_table_plays_the_fallback_once_and_a_late_answer_is_stale(
    #[case] case: Case,
) {
    let timed_out = play(&case, None).await;
    let promptly = play(&case, Some(case.fallback)).await;
    assert_eq!(timed_out.answered_for, 1, "logged once");
    assert_eq!(
        promptly.answered_for, 0,
        "an answer of the seat is not logged as the table's"
    );
    assert_eq!(
        timed_out.log, promptly.log,
        "the table's answer is the documented fallback, saved like the seat's own"
    );

    let mut table = Table::start(&case).await;
    let asked = table.reach(&case).await;
    table.wait(case.clock).await;
    table.clients[asked.seat].drain();
    let late = (case.on_time)(&asked.decision);
    table.reply(asked.seat, asked.id, late).await;
    table.handle.status().await.unwrap();
    assert!(is_stale(&table.clients[asked.seat].drain()));
    assert_eq!(table.answered_for(asked.seat, case.name), 1, "still once");
}

#[rstest]
#[case::yes_no(yes_no())]
#[case::choose_option(choose_option())]
#[case::choose_mode(choose_mode())]
#[case::choose_number(choose_number())]
#[case::divide_amount(divide_amount())]
#[case::pay_mana(pay_mana())]
#[case::spell_targets(spell_targets())]
#[case::keep_or_mulligan(keep_or_mulligan())]
#[case::bottoming(bottoming())]
#[tokio::test(start_paused = true)]
async fn a_reconnect_is_told_the_time_that_is_left_and_the_deadline_stays(#[case] case: Case) {
    let mut table = Table::start(&case).await;
    let asked = table.reach(&case).await;
    table.wait(4_000).await;

    let mut back = Client::join(&table.handle, asked.seat).await;
    let (id, decision, left) = back
        .until(|message| match message {
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
                ..
            } => Some((*id, decision.clone(), *deadline_ms)),
            _ => None,
        })
        .await;
    assert_eq!(
        (id, &decision),
        (asked.id, &asked.decision),
        "the same question"
    );
    assert_eq!(left, Some(case.clock - 4_000), "the time that is left");
    table.clients[asked.seat] = back;

    table.wait(case.clock - 4_000 - 1).await;
    assert_eq!(
        table.answered_for(asked.seat, case.name),
        0,
        "the reconnect did not restart it"
    );
    table.wait(1).await;
    assert_eq!(
        table.answered_for(asked.seat, case.name),
        1,
        "the original deadline"
    );
}

#[rstest]
#[case::yes_no(yes_no())]
#[case::choose_option(choose_option())]
#[case::choose_mode(choose_mode())]
#[case::choose_number(choose_number())]
#[case::divide_amount(divide_amount())]
#[case::pay_mana(pay_mana())]
#[case::spell_targets(spell_targets())]
#[case::keep_or_mulligan(keep_or_mulligan())]
#[case::bottoming(bottoming())]
#[tokio::test(start_paused = true)]
async fn the_other_seats_are_sent_the_same_messages_whether_the_player_answered_or_timed_out(
    #[case] case: Case,
) {
    let timed_out = play(&case, None).await;
    let promptly = play(&case, Some(case.fallback)).await;
    assert_eq!(timed_out.asked, promptly.asked, "the same seat is asked");
    for seat in (0..case.seats).filter(|seat| *seat != timed_out.asked) {
        assert!(
            !timed_out.seen[seat].is_empty(),
            "seat {seat} was sent something"
        );
        assert_eq!(
            timed_out.seen[seat], promptly.seen[seat],
            "seat {seat} cannot tell a timeout from an answer"
        );
    }
}
