//! Goal: the transcript fixture the web tests replay is exactly what seat 0 of seeded 2-player bot
//! games is sent, and folding its messages with the client rule reproduces the expected view.
//!
//! Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test transcript`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Step, run_game};
use mtg_core::ids::PlayerId;
use mtg_engine::Decision;
use mtg_protocol::{DecisionId, PROTOCOL_VERSION, ServerMessage, StateView, project_events};
use mtg_testkit::fuzz_setup;
use serde::Deserialize;

const FIXTURE: &str = "web/src/protocol/fixtures/two-player-seat0.json";
const SEAT: PlayerId = PlayerId(0);
/// Event batches in a transcript.
const BATCHES: usize = 40;
/// A game that ends within the late-join window, so the transcript carries the end of the game.
const LATE_JOIN_SEED: u64 = 20;
/// A game in which seat 0 mulligans, so the transcript carries cards leaving and entering its view.
const LOBBY_SEED: u64 = 58;
const GAME: &str = "00000000-0000-4000-8000-000000000001";
const DESCRIPTION: &str = "What seat 0 of a seeded two-player bot game is sent by mtg-server, in order. \
    `messages` is a seat that joins a running game: Welcome, Snapshot, its pending Decision, then the last \
    batches of the game with their Decisions and GameOver. `lobby` is a seat that joined before the start: \
    Welcome, then the first batches from seq 0 with no Snapshot. `expected` is the seat's true StateView \
    after the last message. Regenerate: MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test transcript";

/// One engine step as the seat lives it.
struct Beat {
    /// What the server queues for the seat after the step: batch, decision, game over, in that order.
    sent: Vec<ServerMessage>,
    /// Log length after the step.
    seq: u64,
    /// The seat's true view after the step.
    view: StateView,
    /// The decision waiting after the step and its id.
    pending: Option<(DecisionId, Decision)>,
}

#[derive(Debug, Deserialize)]
struct Transcript {
    seed: u64,
    messages: Vec<ServerMessage>,
    expected: StateView,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    description: String,
    seat: PlayerId,
    seed: u64,
    messages: Vec<ServerMessage>,
    expected: StateView,
    lobby: Transcript,
}

/// Plays a seeded game and records, step by step, what the server would send seat 0.
///
/// Mirrors `GameActor`: an empty step sends no batch, every pending decision gets the next id,
/// and only the seat it names is sent it.
fn play(seed: u64) -> Vec<Beat> {
    let mut beats = Vec::new();
    let mut last_id = 0;
    run_game(fuzz_setup(2, seed), seed, |step: &Step| {
        let mut sent = Vec::new();
        if !step.events.is_empty() {
            let batch = project_events(step.before, step.events, &[SEAT], step.from_seq)
                .unwrap()
                .remove(0);
            sent.push(ServerMessage::Events(batch));
        }
        let pending = step.decision.map(|decision| {
            last_id += 1;
            (DecisionId(last_id), decision.clone())
        });
        if let Some((id, decision)) = pending.as_ref().filter(|(_, d)| d.player == SEAT) {
            sent.push(ServerMessage::Decision {
                id: *id,
                decision: decision.clone(),
            });
        }
        if let Some(result) = step.after.result() {
            sent.push(ServerMessage::GameOver { result });
        }
        beats.push(Beat {
            sent,
            seq: step.from_seq + u64::try_from(step.events.len()).unwrap(),
            view: StateView::for_player(step.after, SEAT),
            pending,
        });
    });
    beats
}

fn welcome() -> ServerMessage {
    ServerMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        game: GAME.to_owned(),
        player: SEAT,
        seats: 2,
    }
}

fn has_batch(beat: &Beat) -> bool {
    beat.sent
        .iter()
        .any(|message| matches!(message, ServerMessage::Events(_)))
}

/// A seat that joins a running game just before its last `BATCHES` batches.
fn late_join(seed: u64) -> (Vec<ServerMessage>, StateView) {
    let beats = play(seed);
    let mut found = 0;
    let start = (0..beats.len())
        .rev()
        .find(|at| {
            found += usize::from(has_batch(&beats[*at]));
            found == BATCHES
        })
        .expect("the game is longer than the transcript");
    let before = &beats[start - 1];
    let mut messages = vec![
        welcome(),
        ServerMessage::Snapshot {
            seq: before.seq,
            state: Box::new(before.view.clone()),
        },
    ];
    if let Some((id, decision)) = before.pending.as_ref().filter(|(_, d)| d.player == SEAT) {
        messages.push(ServerMessage::Decision {
            id: *id,
            decision: decision.clone(),
        });
    }
    messages.extend(beats[start..].iter().flat_map(|beat| beat.sent.clone()));
    (messages, beats.last().unwrap().view.clone())
}

/// A seat that was connected before the game started: no snapshot, the first batch is from seq 0.
fn lobby(seed: u64) -> (Vec<ServerMessage>, StateView) {
    let beats = play(seed);
    let mut messages = vec![welcome()];
    let mut found = 0;
    for beat in &beats {
        messages.extend(beat.sent.clone());
        found += usize::from(has_batch(beat));
        if found == BATCHES {
            return (messages, beat.view.clone());
        }
    }
    panic!("the game is shorter than the transcript");
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap()
}

/// One message per line keeps the committed file diffable.
fn lines(messages: &[ServerMessage], indent: &str) -> String {
    let body: Vec<String> = messages
        .iter()
        .map(|message| format!("{indent}  {}", json(message)))
        .collect();
    format!("[\n{}\n{indent}]", body.join(",\n"))
}

fn render() -> String {
    let (messages, expected) = late_join(LATE_JOIN_SEED);
    let (lobby_messages, lobby_expected) = lobby(LOBBY_SEED);
    format!(
        "{{\n  \"description\": {},\n  \"seat\": {},\n  \"seed\": {LATE_JOIN_SEED},\n  \"messages\": {},\n  \"expected\": {},\n  \
         \"lobby\": {{\n    \"seed\": {LOBBY_SEED},\n    \"messages\": {},\n    \"expected\": {}\n  }}\n}}\n",
        json(&DESCRIPTION),
        json(&SEAT),
        lines(&messages, "  "),
        json(&expected),
        lines(&lobby_messages, "    "),
        json(&lobby_expected),
    )
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FIXTURE)
}

/// The client rule from `docs/protocol.md`: start from a snapshot, or from the batch at seq 0, and
/// fold each batch that starts where the last one ended.
fn replay(messages: &[ServerMessage]) -> StateView {
    let mut view: Option<StateView> = None;
    let mut next_seq = 0;
    for message in messages {
        match message {
            ServerMessage::Snapshot { seq, state } => {
                view = Some((**state).clone());
                next_seq = *seq;
            }
            ServerMessage::Events(batch) => {
                assert_eq!(
                    batch.from_seq, next_seq,
                    "a gap means the client must resync"
                );
                match view.as_mut() {
                    Some(view) => view.apply(batch),
                    None => view = Some(StateView::from_first_batch(batch).unwrap()),
                }
                next_seq += u64::try_from(batch.events.len()).unwrap();
            }
            _ => {}
        }
    }
    view.expect("the transcript has a state")
}

fn decision_ids(messages: &[ServerMessage]) -> Vec<u64> {
    messages
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Decision { id, decision } => {
                assert_eq!(
                    decision.player, SEAT,
                    "decisions go only to the seat they name"
                );
                Some(id.0)
            }
            _ => None,
        })
        .collect()
}

fn batches(messages: &[ServerMessage]) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, ServerMessage::Events(_)))
        .count()
}

fn regenerating() -> bool {
    std::env::var_os("MTG_UPDATE_BINDINGS").is_some()
}

#[test]
fn the_committed_fixture_is_what_the_server_would_send() {
    let fresh = render();
    let path = fixture_path();
    if regenerating() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &fresh).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == fresh,
        "the transcript fixture drifted; run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test transcript"
    );
    assert!(
        fresh.len() < 300 * 1024,
        "fixture is {} bytes, over the 300 KB budget",
        fresh.len()
    );
}

#[test]
fn folding_the_committed_fixture_gives_the_expected_view() {
    // Regenerating writes the file in the other test, so judge the fresh text rather than race it.
    let text = if regenerating() {
        render()
    } else {
        fs::read_to_string(fixture_path()).unwrap()
    };
    let fixture: Fixture = serde_json::from_str(&text).unwrap();
    assert!(fixture.description.contains("Regenerate"));
    assert_eq!(fixture.seat, SEAT);

    for (name, seed, messages, expected) in [
        (
            "late join",
            fixture.seed,
            &fixture.messages,
            &fixture.expected,
        ),
        (
            "lobby",
            fixture.lobby.seed,
            &fixture.lobby.messages,
            &fixture.lobby.expected,
        ),
    ] {
        assert!(
            matches!(messages[0], ServerMessage::Welcome { player, seats: 2, .. } if player == SEAT)
        );
        assert_eq!(batches(messages), BATCHES, "{name} seed {seed}");
        let ids = decision_ids(messages);
        assert!(
            ids.windows(2).all(|pair| pair[0] < pair[1]) && ids.len() > 5,
            "{name}: decision ids rise"
        );
        assert!(
            replay(messages) == *expected,
            "{name}: folding the messages does not give the expected view"
        );
    }

    assert!(matches!(
        fixture.messages[1],
        ServerMessage::Snapshot { .. }
    ));
    assert!(matches!(
        fixture.messages.last(),
        Some(ServerMessage::GameOver { .. })
    ));
    assert!(fixture.expected.table.result.is_some());
    assert!(
        matches!(&fixture.lobby.messages[1], ServerMessage::Events(batch) if batch.from_seq == 0),
        "a lobby seat starts from the batch at seq 0"
    );
    assert!(
        fixture
            .lobby
            .messages
            .iter()
            .any(|m| matches!(m, ServerMessage::Events(batch) if !batch.gone.is_empty())),
        "the lobby transcript shows cards leaving the seat's view"
    );
}
