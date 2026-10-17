//! Goal: shared helpers that play seeded bot games where every card has a distinct name.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod table;

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, Event, GameSetup, GameState};
use mtg_testkit::{RandomBot, fuzz_setup};

/// Whether `viewer` may not know which card is in `zone`: any library, or another seat's hand.
///
/// Stated here on its own so tests judge the crate's visibility rule against a second opinion.
pub fn is_hidden(zone: Zone, viewer: PlayerId) -> bool {
    match zone {
        Zone::Library(_) => true,
        Zone::Hand(owner) => owner != viewer,
        _ => false,
    }
}

/// Ids of the objects `viewer` may identify in `state`.
pub fn visible_ids(state: &GameState, viewer: PlayerId) -> BTreeSet<ObjectId> {
    state
        .objects()
        .filter(|o| !is_hidden(o.zone(), viewer))
        .map(mtg_engine::object::GameObject::id)
        .collect()
}

/// One answer's worth of progress: the state around it and the events it produced.
pub struct Step<'a> {
    /// State before the events.
    pub before: &'a GameState,
    /// The events, in log order.
    pub events: &'a [Event],
    /// State after the events.
    pub after: &'a GameState,
    /// Log position of the first event.
    pub from_seq: u64,
    /// How many answers have been applied (0 for the setup events).
    pub answers: usize,
    /// The decision waiting after the events, or `None` once the game is over.
    pub decision: Option<&'a Decision>,
}

/// A fuzz setup whose every card has a distinct name, so a leak of any card can be searched for.
pub fn unique_setup(players: usize, seed: u64) -> GameSetup {
    let mut setup = fuzz_setup(players, seed);
    for (seat, player) in setup.players.iter_mut().enumerate() {
        player.library = player
            .library
            .iter()
            .enumerate()
            .map(|(i, card)| {
                let mut def = (**card).clone();
                def.name = format!("Secret<p{seat}:c{i}>");
                std::sync::Arc::new(def)
            })
            .collect();
    }
    setup.validate_decks = false;
    setup
}

/// Plays `setup` to the end with seeded random bots, calling `observe` for the setup events and then every answer.
pub fn run_game(setup: GameSetup, seed: u64, mut observe: impl FnMut(&Step)) -> Engine {
    let initial = GameState::from_setup(&setup).expect("valid setup");
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(setup.players.len())
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut engine = Engine::new(setup).expect("engine starts");
    let setup_events = engine.events().to_vec();
    observe(&Step {
        before: &initial,
        events: &setup_events,
        after: engine.state(),
        from_seq: 0,
        answers: 0,
        decision: engine.decision(),
    });
    let mut answers = 0;
    while let Some(decision) = engine.decision().cloned() {
        let answer = bots[decision.player.index()].choose(&decision);
        let before = engine.state().clone();
        let from_seq = engine.events().len() as u64;
        let events = engine.apply(answer).expect("bot answers are legal");
        answers += 1;
        observe(&Step {
            before: &before,
            events: &events,
            after: engine.state(),
            from_seq,
            answers,
            decision: engine.decision(),
        });
    }
    engine
}
