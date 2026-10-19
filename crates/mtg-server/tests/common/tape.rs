//! Goal: write down what a client is sent together with the moment it arrives on tokio's clock, which the tests that read it pause.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::Decision;
use mtg_protocol::{ClientMessage, DecisionId, EventBatch, ServerMessage};
use mtg_server::GameHandle;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// A message and how long after the tape's start it arrived.
pub type Taped = (Duration, ServerMessage);

/// A client that only listens: every message it is sent, with its arrival time.
pub struct Tape {
    log: Arc<Mutex<Vec<Taped>>>,
    task: JoinHandle<()>,
}

impl Tape {
    /// Listens to `inbox`; arrival times count from `since`.
    pub fn listen(mut inbox: Receiver<ServerMessage>, since: Instant) -> Tape {
        let log = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&log);
        let task = tokio::spawn(async move {
            while let Some(message) = inbox.recv().await {
                writer.lock().unwrap().push((since.elapsed(), message));
            }
        });
        Tape { log, task }
    }

    /// Like [`Tape::listen`] for a seat played by a bot: it answers each decision it is sent with the harmless answer
    /// (a pass for priority) after a pause, the n-th decision after `think[n]` (the list repeats).
    pub fn play(
        mut inbox: Receiver<ServerMessage>,
        since: Instant,
        handle: GameHandle,
        seat: PlayerId,
        think: Vec<Duration>,
    ) -> Tape {
        let log = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&log);
        let task = tokio::spawn(async move {
            let mut asked = 0;
            while let Some(message) = inbox.recv().await {
                if let ServerMessage::Decision { id, decision, .. } = &message {
                    let (id, answer) = (*id, super::passive(decision));
                    let pause = think[asked % think.len()];
                    asked += 1;
                    let handle = handle.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(pause).await;
                        let reply = ClientMessage::Answer {
                            decision: id,
                            answer,
                        };
                        // A game that has ended no longer listens.
                        let _ = handle.send(seat, reply).await;
                    });
                }
                writer.lock().unwrap().push((since.elapsed(), message));
            }
        });
        Tape { log, task }
    }

    /// Everything heard so far.
    pub fn heard(&self) -> Vec<Taped> {
        self.log.lock().unwrap().clone()
    }

    /// Forgets everything heard so far and returns it.
    pub fn take(&self) -> Vec<Taped> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }

    /// The event batches heard so far with their arrival times.
    pub fn batches(&self) -> Vec<(Duration, EventBatch)> {
        self.heard()
            .into_iter()
            .filter_map(|(at, message)| match message {
                ServerMessage::Events(batch) => Some((at, batch)),
                _ => None,
            })
            .collect()
    }

    /// The decisions heard so far with their arrival times.
    pub fn decisions(&self) -> Vec<(Duration, DecisionId, Decision)> {
        self.heard()
            .into_iter()
            .filter_map(|(at, message)| match message {
                ServerMessage::Decision { id, decision, .. } => Some((at, id, decision)),
                _ => None,
            })
            .collect()
    }

    /// The newest decision heard, if any.
    pub fn last_decision(&self) -> Option<(DecisionId, Decision)> {
        self.decisions()
            .pop()
            .map(|(_, id, decision)| (id, decision))
    }

    /// The names of the messages heard so far, in order.
    pub fn kinds(&self) -> Vec<String> {
        self.heard().iter().map(|(_, m)| kind(m)).collect()
    }

    /// Stops listening.
    pub fn stop(self) {
        self.task.abort();
    }
}

/// The name of a message as it is on the wire (`Events`, `Decision`, `Yielding`...).
pub fn kind(message: &ServerMessage) -> String {
    serde_json::to_value(message)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(|t| t.as_str().map(str::to_owned))
        })
        .expect("every server message is tagged with its type")
}

/// Whole milliseconds of `span`.
pub fn ms(span: Duration) -> u64 {
    u64::try_from(span.as_millis()).unwrap()
}
