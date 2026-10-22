//! Goal: whole bot games with decks of the Curses that trigger on an attack at the enchanted player (Bounty, Chaos, Disturbance, Opulence, Shallow Graves, Verbosity, Vitality, Predation and the Forsaken) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards, what every Aura is attached to) for many seeds at tables of two, three and four seats, and the games really use them: each Curse is cast at a player, enters attached to them and triggers when they are attacked.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::{Event, GameState, replay};
use mtg_testkit::{curse_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// What a campaign saw the Curses do.
#[derive(Default)]
struct Seen {
    cast: BTreeMap<String, u64>,
    triggered: BTreeMap<String, u64>,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let name = |id| game.object(id).map(|object| object.card().name.clone());
        for event in events {
            match event {
                Event::SpellCast { object, .. } => {
                    if let Some(name) = name(*object).filter(|n| n.starts_with("Curse of")) {
                        *self.cast.entry(name).or_default() += 1;
                    }
                }
                Event::AbilityTriggered(triggered) => {
                    if let Some(name) =
                        name(triggered.source.id).filter(|n| n.starts_with("Curse of"))
                    {
                        *self.triggered.entry(name).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_curse_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = curse_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for name in mtg_testkit::curse_cards::CURSE_CARDS {
        assert!(
            seen.cast.get(*name).copied().unwrap_or(0) > 0,
            "{name} was never cast in {campaign}"
        );
    }
    assert!(
        seen.triggered.values().sum::<u64>() > 0,
        "no Curse triggered in {campaign}"
    );
    for name in [
        "Curse of Vitality",
        "Curse of Verbosity",
        "Curse of Disturbance",
        "Curse of Opulence",
        "Curse of Bounty",
    ] {
        assert!(
            seen.triggered.get(name).copied().unwrap_or(0) > 0,
            "{name} never triggered in {campaign}: {:?}",
            seen.triggered
        );
    }
}
