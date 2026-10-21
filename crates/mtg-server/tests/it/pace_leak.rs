//! Goal: the table cannot tell a seat with a play from one without by what it is sent or when, at the windows the dwell hides. In two games with the same seed that differ by one card in a hidden hand, an instant the seat can cast against one it cannot afford, an attack that ends the game and a creature spell with the ability that triggers as it enters, the other seat (able to act or idle) is sent the same kinds of message, batches and views, each batch arriving 250 to 700 ms after the one before it on a paused clock, and nothing that depends on the hand (a yield, a stop, an error); with the dwell switched off the same comparison finds the difference, and so it does in a seat's own main phase with an empty stack, which the dwell leaves alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use common::{Tape, Taped, game_id, kind, ms, passive, roomy, seat};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, Targetable};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, DecisionKind, Event, GameSetup};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{damage, free_creature, giant, instant};
use mtg_testkit::trigger_cards::{aimed_at, creature_with, trigger as ability, when};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use tokio::time::Instant;

/// How long each seat's bot takes to answer, decision after decision: a person's pause, inside the dwell range.
const THINKING_MS: [[u64; 8]; 2] = [
    [450, 620, 530, 680, 360, 640, 400, 560],
    [610, 380, 660, 500, 560, 340, 690, 460],
];

/// The same for the games over the stack, where seat 0 casts: its two answers to a cast (the spell, the payment) come in
/// one batch, so together they fit the dwell range as well.
const STACK_THINKING_MS: [[u64; 8]; 2] = [[300; 8], THINKING_MS[1]];

/// Batches the first of the other seats is sent before a game that has not ended is cut.
const BATCHES: usize = 12;

/// The card in the holder's hand that the two games do not share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    /// An instant that costs nothing: the holder can cast it in any priority window.
    Castable,
    /// An instant that costs {3}{G}{G} when the holder has no lands: it can never be cast.
    Unaffordable,
}

/// The instant the holder holds.
fn trick(card: Card) -> std::sync::Arc<mtg_cards::CardDefinition> {
    match card {
        Card::Castable => instant("Trick", 0, &[]),
        Card::Unaffordable => instant("Trick", 5, &[]),
    }
}

/// Libraries that hold nothing anybody can use, and the same seed.
fn quiet() -> Scenario {
    Scenario::new(2)
        .seed(11)
        .library(0, vec![giant(); 30])
        .library(1, vec![giant(); 30])
}

/// Seat 0 attacks seat 1, who has two life and nothing to block with, in the declare attackers step of turn 2: the
/// declare attackers and declare blockers steps are where the dwell hides what the holder holds, and the damage ends
/// the game. The other seat holds a free instant, so it can act in each of its windows, or (`other_can_act` false) a
/// giant, so it cannot and is passed for after a dwell of its own like the holder without a play.
fn combat(holder: usize, card: Card, other_can_act: bool) -> GameSetup {
    let hand = |seat| {
        if seat == holder {
            trick(card)
        } else if other_can_act {
            instant("Free Trick", 0, &[])
        } else {
            giant()
        }
    };
    quiet()
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
        .life(1, 2)
        .hand(0, [hand(0)])
        .hand(1, [hand(1)])
        .setup()
        .unwrap()
}

/// Seat 0 holds the card in its own first main phase, with an empty stack, and nothing happens there but passing.
fn own_main(card: Card) -> GameSetup {
    quiet().hand(0, [trick(card)]).setup().unwrap()
}

/// A bot that attacks seat 1 with everything it has and passes whenever it is asked anything else.
fn attacking(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(
            options
                .attackers
                .iter()
                .map(|attacker| (attacker.object, AttackTarget::Player(seat(1))))
                .collect(),
        ),
        _ => passive(decision),
    }
}

/// What one game showed.
struct Run {
    /// Per seat that does not hold the card: everything it was sent, with arrival times, up to the end of the game or
    /// the last batch counted.
    others: Vec<Vec<Taped>>,
    /// Everything the holder was sent.
    holder: Vec<Taped>,
    /// The event log at the end.
    log: Vec<Event>,
    /// The state hash at the end.
    hash: Option<u64>,
}

/// Whether `tape` has heard the end of the game or [`BATCHES`] batches.
fn finished(tape: &Tape) -> bool {
    let heard = tape.heard();
    heard
        .iter()
        .any(|(_, message)| matches!(message, ServerMessage::GameOver { .. }))
        || heard
            .iter()
            .filter(|(_, message)| matches!(message, ServerMessage::Events(_)))
            .count()
            >= BATCHES
}

/// Plays a game with the server's default pace but `dwell`: both seats are bots that answer after a pause (seat 0
/// attacks), and `holder` is the one the other seat is watching. It ends with the game, after [`BATCHES`] batches or
/// after a minute.
async fn play(setup: GameSetup, holder: usize, dwell: Dwell) -> Run {
    play_with(setup, holder, dwell, [attacking, passive], THINKING_MS).await
}

/// [`play`] with the answers of the two seats given (what they do with each decision they are sent) and how long each
/// takes to give them.
async fn play_with(
    setup: GameSetup,
    holder: usize,
    dwell: Dwell,
    brains: [fn(&Decision) -> Answer; 2],
    thinking: [[u64; 8]; 2],
) -> Run {
    let since = Instant::now();
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: Pace {
            dwell,
            ..Pace::default()
        },
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    let mut tapes = Vec::new();
    for (n, think) in thinking.iter().enumerate() {
        let inbox = handle.join(seat(n)).await.unwrap();
        let think = think
            .iter()
            .map(|pause| Duration::from_millis(*pause))
            .collect();
        tapes.push(Tape::play(
            inbox,
            since,
            handle.clone(),
            seat(n),
            think,
            brains[n],
        ));
    }
    for n in 0..2 {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    let watcher = 1 - holder;
    while since.elapsed() < Duration::from_secs(60) && !finished(&tapes[watcher]) {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let status = handle.status().await.unwrap();
    let mut others = Vec::new();
    let mut held = Vec::new();
    for (n, tape) in tapes.into_iter().enumerate() {
        let heard = tape.heard();
        tape.stop();
        if n == holder {
            held = heard;
        } else {
            others.push(cut(heard));
        }
    }
    Run {
        others,
        holder: held,
        log: store.load(game_id(1)).await.unwrap(),
        hash: status.state_hash,
    }
}

/// `heard` up to the end of the game, or up to and including its [`BATCHES`]th batch.
fn cut(heard: Vec<Taped>) -> Vec<Taped> {
    let mut batches = 0;
    let mut kept = Vec::new();
    for (at, message) in heard {
        let over = matches!(message, ServerMessage::GameOver { .. });
        batches += usize::from(matches!(message, ServerMessage::Events(_)));
        kept.push((at, message));
        if over || batches == BATCHES {
            break;
        }
    }
    kept
}

/// The messages of `heard` as they are on the wire, without when they arrived.
fn told(heard: &[Taped]) -> Vec<ServerMessage> {
    heard.iter().map(|(_, message)| message.clone()).collect()
}

/// The kinds of message in `heard`, in order.
fn kinds(heard: &[Taped]) -> Vec<String> {
    heard.iter().map(|(_, message)| kind(message)).collect()
}

/// Where each batch in `heard` starts in the log, how many events it holds and where the game stood after it.
fn boundaries(heard: &[Taped]) -> Vec<(u64, usize, (u32, Step))> {
    heard
        .iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Events(batch) => Some((
                batch.from_seq,
                batch.events.len(),
                (batch.table.turn.number, batch.table.turn.step),
            )),
            _ => None,
        })
        .collect()
}

/// Milliseconds between each batch in `heard` and the one before it.
fn gaps(heard: &[Taped]) -> Vec<u64> {
    let arrivals: Vec<u64> = heard
        .iter()
        .filter(|(_, message)| matches!(message, ServerMessage::Events(_)))
        .map(|(at, _)| ms(*at))
        .collect();
    arrivals.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// The wait the server's default dwell draws from, in milliseconds: the range every batch has to arrive in.
fn dwell_range() -> std::ops::RangeInclusive<u64> {
    let (shortest, longest) = Dwell::DEFAULT.range();
    ms(shortest)..=ms(longest)
}

/// What an observer may be sent while it waits: a `Welcome`, the lobby before the game, the game's events, its end and
/// the decisions of its own (which depend on its own hand only: what it is told must be the same in both games).
/// Anything else (`Yielding`, `Stops`, `FullControl`, a snapshot, an error) can depend on somebody else's hand.
const MAY_BE_SENT: [&str; 5] = ["Welcome", "Lobby", "Events", "GameOver", "Decision"];

/// Everything that differs between what the seats that do not hold the card were sent in the two games, and everything
/// they were sent that could depend on the hand; empty when the table cannot tell the games apart.
fn differences(with_a_play: &Run, without: &Run) -> Vec<String> {
    let mut found = Vec::new();
    for (n, (a, b)) in with_a_play.others.iter().zip(&without.others).enumerate() {
        if kinds(a) != kinds(b) {
            found.push(format!(
                "seat {n}: the messages it is sent differ: {:?} against {:?}",
                kinds(a),
                kinds(b)
            ));
        }
        if boundaries(a) != boundaries(b) {
            found.push(format!(
                "seat {n}: the batches differ: {:?} against {:?}",
                boundaries(a),
                boundaries(b)
            ));
        }
        if told(a) != told(b) {
            found.push(format!("seat {n}: it is told different things"));
        }
        for (label, heard) in [("with a play", a), ("without one", b)] {
            for gap in gaps(heard) {
                if !dwell_range().contains(&gap) {
                    found.push(format!(
                        "seat {n}, {label}: a batch came {gap} ms after the one before"
                    ));
                }
            }
            for sent in kinds(heard) {
                if !MAY_BE_SENT.contains(&sent.as_str()) {
                    found.push(format!("seat {n}, {label}: it was sent a {sent}"));
                }
            }
        }
    }
    found
}

/// The windows the holder was asked about, as whether it could act at each.
fn asked(run: &Run) -> Vec<bool> {
    run.holder
        .iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Decision { decision, .. } => match &decision.kind {
                DecisionKind::Priority(options) => Some(options.can_act),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Whether the game ended in what `run`'s other seats were sent.
fn ended(run: &Run) -> bool {
    run.others.iter().all(|heard| {
        heard
            .iter()
            .any(|(_, message)| matches!(message, ServerMessage::GameOver { .. }))
    })
}

#[rstest]
#[case::the_attacker_holds_the_card_and_the_other_seat_can_act(0, true)]
#[case::the_attacker_holds_the_card_and_the_other_seat_is_idle(0, false)]
#[case::the_defender_holds_the_card_and_the_other_seat_can_act(1, true)]
#[case::the_defender_holds_the_card_and_the_other_seat_is_idle(1, false)]
#[tokio::test(start_paused = true)]
async fn the_table_cannot_tell_a_seat_with_a_play_from_one_without_in_combat(
    #[case] holder: usize,
    #[case] other_can_act: bool,
) {
    assert!(
        THINKING_MS
            .iter()
            .flatten()
            .all(|pause| dwell_range().contains(pause)),
        "the bots think as long as a dwell lasts, or the pause alone would give them away"
    );
    let hands = |card| combat(holder, card, other_can_act);
    let with_a_play = play(hands(Card::Castable), holder, Dwell::DEFAULT).await;
    let without = play(hands(Card::Unaffordable), holder, Dwell::DEFAULT).await;

    // What the other seat can see of the two games is nothing that tells them apart...
    assert_eq!(differences(&with_a_play, &without), Vec::<String>::new());

    // ...though the games differ in one hidden card and in nothing the log says...
    assert_eq!(with_a_play.log, without.log);
    assert_ne!(with_a_play.hash, without.hash);
    // ...so that the holder is asked in both of its windows (declare attackers and declare blockers) in one game and in
    // none in the other, and both games ran to the end.
    assert_eq!(asked(&with_a_play), [true, true]);
    assert_eq!(asked(&without), Vec::<bool>::new());
    assert!(
        ended(&with_a_play) && ended(&without),
        "the attack is lethal"
    );
    assert!(
        with_a_play.others[0]
            .iter()
            .filter(|(_, message)| matches!(message, ServerMessage::Events(_)))
            .count()
            >= 5,
        "the other seat was sent a batch for the attack and for each of the four windows"
    );
}

#[rstest]
#[case::the_attacker_holds_the_card(0)]
#[case::the_defender_holds_the_card(1)]
#[tokio::test(start_paused = true)]
async fn with_the_dwell_switched_off_the_same_comparison_finds_the_tell(#[case] holder: usize) {
    // MTG_DWELL_MS=0: an idle window is passed for at once, in the batch of the step before it.
    let with_a_play = play(combat(holder, Card::Castable, false), holder, Dwell::OFF).await;
    let without = play(
        combat(holder, Card::Unaffordable, false),
        holder,
        Dwell::OFF,
    )
    .await;

    let found = differences(&with_a_play, &without);
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("batches differ")),
        "the pace tells the games apart without a dwell: {found:?}"
    );
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("messages it is sent differ")),
        "and so does the stream of messages: {found:?}"
    );
}

/// A bot that casts whatever it can (at seat 1 when it can aim there, else at the first target, paying automatically)
/// and answers passively otherwise.
fn casting(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(spell) => PriorityAction::CastSpell(*spell),
            None => PriorityAction::Pass,
        }),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseTargets(options) => {
            let foe = options
                .legal
                .iter()
                .copied()
                .find(|target| matches!(target, Target::Player(p) if p.index() == 1));
            let first = options.legal.iter().copied().take(options.min as usize);
            Answer::ChooseTargets(TargetChoice::Targets(
                foe.map_or_else(|| first.collect(), |foe| vec![foe]),
            ))
        }
        _ => passive(decision),
    }
}

/// Seat 0 casts, in its first main phase of turn 2, a spell that makes its opponent lose the three life it has (with
/// `trigger`, a creature whose ability deals three damage to an opponent as it enters). The spell and, with `trigger`,
/// then the ability it sets off wait on the stack in the windows of seat 1, and the last of them ends the game. Seat 1
/// holds the card the two games differ by.
fn stack_game(card: Card, trigger: bool) -> GameSetup {
    let spell = if trigger {
        let enters = aimed_at(
            ability(when::this_enters(), vec![damage(Ref::Target(0), 3)]),
            Targetable::Opponent,
        );
        creature_with("Drifter", vec![enters])
    } else {
        Arc::new(
            CardDefinition::new("Doom")
                .with_type(CardType::Instant)
                .with_mana_cost(ManaCost::default())
                .with_rules(Ability::Spell {
                    targets: Vec::new(),
                    effects: vec![Effect::LoseLife {
                        who: Ref::Opponents,
                        amount: Amount::Fixed(3),
                    }],
                }),
        )
    };
    quiet()
        .at(2, 0, Step::Main1)
        .hand(0, [spell])
        .hand(1, [trick(card)])
        .life(1, 3)
        .setup()
        .unwrap()
}

#[rstest]
#[case::a_spell_waits_on_the_stack(false, 1)]
#[case::a_spell_and_then_its_trigger_wait_on_the_stack(true, 2)]
#[tokio::test(start_paused = true)]
async fn the_table_cannot_tell_a_seat_with_a_play_from_one_without_while_a_spell_or_a_trigger_waits_on_the_stack(
    #[case] trigger: bool,
    #[case] windows: usize,
) {
    let brains: [fn(&Decision) -> Answer; 2] = [casting, passive];
    let with_a_play = play_with(
        stack_game(Card::Castable, trigger),
        1,
        Dwell::DEFAULT,
        brains,
        STACK_THINKING_MS,
    )
    .await;
    let without = play_with(
        stack_game(Card::Unaffordable, trigger),
        1,
        Dwell::DEFAULT,
        brains,
        STACK_THINKING_MS,
    )
    .await;

    // Seat 0, which cast the spell, cannot tell what seat 1 held in the windows over the stack...
    assert_eq!(differences(&with_a_play, &without), Vec::<String>::new());
    // ...though the games differ in one hidden card and in nothing the log says...
    assert_eq!(with_a_play.log, without.log);
    assert_ne!(with_a_play.hash, without.hash);
    // ...so that seat 1 is asked in each of its windows over the spell or the trigger in one game and in none in the
    // other.
    assert_eq!(asked(&with_a_play), vec![true; windows]);
    assert_eq!(asked(&without), Vec::<bool>::new());
    let cast = with_a_play
        .log
        .iter()
        .any(|e| matches!(e, Event::SpellCast { .. }));
    let triggered = with_a_play
        .log
        .iter()
        .any(|e| matches!(e, Event::AbilityPutOnStack(put) if put.cause.is_some()));
    assert!(cast && triggered == trigger, "the games used the stack");
}

#[rstest]
#[case::a_spell_waits_on_the_stack(false)]
#[case::a_spell_and_then_its_trigger_wait_on_the_stack(true)]
#[tokio::test(start_paused = true)]
async fn with_the_dwell_switched_off_the_same_comparison_finds_the_tell_over_the_stack(
    #[case] trigger: bool,
) {
    let brains: [fn(&Decision) -> Answer; 2] = [casting, passive];
    let with_a_play = play_with(
        stack_game(Card::Castable, trigger),
        1,
        Dwell::OFF,
        brains,
        STACK_THINKING_MS,
    )
    .await;
    let without = play_with(
        stack_game(Card::Unaffordable, trigger),
        1,
        Dwell::OFF,
        brains,
        STACK_THINKING_MS,
    )
    .await;
    let found = differences(&with_a_play, &without);
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("batches differ")),
        "the pace tells the games apart without a dwell: {found:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_seats_own_main_phase_with_an_empty_stack_is_not_hidden() {
    // A seat that can act is asked there, so the other seat waits for it; one that cannot is passed for at once. The
    // dwell leaves that window alone: a land or a spell played there is in plain sight anyway.
    let with_a_play = play(own_main(Card::Castable), 0, Dwell::DEFAULT).await;
    let without = play(own_main(Card::Unaffordable), 0, Dwell::DEFAULT).await;

    let found = differences(&with_a_play, &without);
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("batches differ")),
        "the hand shows in the pace of the own main phase: {found:?}"
    );
}
