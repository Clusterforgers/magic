//! Goal: whole bot games at 2 to 4 seats with scavenge, transmute, awaken, gravestorm and sunburst cards keep every invariant, and across the seeds each mechanic occurs: a card is scavenged, a card is transmuted, a spell is cast for its awaken cost and puts counters on a land, a spell with gravestorm is copied, and a sunburst permanent enters with its counters.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::{CastWay, CounterKind};
use mtg_engine::Event;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, play_game_with, recursion_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the events just before an activation paid: the exile of a card from a graveyard or the discard of a card.
fn paid_by(events: &[Event], at: usize) -> &'static str {
    for event in events[..at].iter().rev().take(6) {
        match event {
            Event::ZoneChanged {
                from: Zone::Graveyard(_),
                to: Zone::Exile,
                ..
            } => return "scavenge",
            Event::CardDiscarded { .. } => return "transmute",
            _ => {}
        }
    }
    "other"
}

#[test]
fn bot_games_with_the_recursion_keywords_keep_every_invariant_and_use_each_of_them() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                recursion_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut awakened = None;
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::AbilityActivated {
                        origin: AbilitySource::PrintedKeyword(_),
                        ..
                    } => match paid_by(&outcome.events, at) {
                        "scavenge" => *seen.entry("scavenge").or_default() += 1,
                        "transmute" => *seen.entry("transmute").or_default() += 1,
                        _ => {}
                    },
                    Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Awaken) => {
                        *seen.entry("awaken").or_default() += 1;
                        awakened = Some(at);
                    }
                    Event::SpellCopied(_) => *seen.entry("gravestorm copy").or_default() += 1,
                    Event::CountersChanged(changed) if changed.delta > 0 => {
                        if changed.counter == CounterKind::Charge {
                            *seen.entry("sunburst charge").or_default() += 1;
                        }
                        if awakened.is_some_and(|cast| at < cast + 60) {
                            *seen.entry("awaken counters").or_default() += 1;
                            awakened = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in [
        "scavenge",
        "transmute",
        "awaken",
        "awaken counters",
        "gravestorm copy",
        "sunburst charge",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no game went through {part}: {seen:?}"
        );
    }
}
