//! Goal: whole bot games with decks of Auras (Control Magic, Pacifism, Rancor, Animate Dead, Curse of Chaos) and Canny Thieves that exile the opponents' top cards face down for their controller to cast keep every invariant at two to four seats, no Aura spell among them is on the stack without the target it enchants, every Aura cast announces one, and the games really cast Auras from hand and from an opponent's library.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::{Event, replay};
use mtg_testkit::attach_cards::{HIDDEN_AURA_CARDS, hidden_aura_fuzz_setup};
use mtg_testkit::play_game;
use rstest::rstest;

use crate::common::seeds::fuzz_games;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(16)
}

#[rstest]
fn aura_spells_always_carry_their_target(#[values(2, 3, 4)] players: usize) {
    let (mut own, mut stolen) = (0u64, 0u64);
    for seed in 0..games() {
        let setup = hidden_aura_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        for event in &outcome.events {
            let Event::SpellCast {
                player,
                object,
                choices,
            } = event
            else {
                continue;
            };
            // The objects of a player who left the game are gone from the end state.
            let Some(spell) = end.object(*object) else {
                continue;
            };
            if !HIDDEN_AURA_CARDS.contains(&spell.card().name.as_str()) {
                continue;
            }
            assert!(
                choices.targets.first().is_some_and(|t| t.len() == 1),
                "seed {seed}: {} cast without its target",
                spell.card().name
            );
            if spell.owner() == *player {
                own += 1;
            } else {
                stolen += 1;
            }
        }
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(own > 0, "no Aura was cast from hand in {campaign}");
    assert!(
        stolen > 0,
        "no Aura was cast from an opponent's library in {campaign}"
    );
}
