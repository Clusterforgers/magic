//! Goal: whole bot games at 2 to 4 seats with the cards cast from a graveyard for a discard (jump-start, retrace) and the creatures embalmed and eternalized from it keep every invariant, and across the seeds each way is used; every token made that way is a Zombie copy with no mana cost, white (embalm) or a black 4/4 (eternalize), every jump-started spell leaves the stack for exile.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_core::card::Subtype;
use mtg_core::color::{Color, ColorSet};
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, discard_cast_fuzz_setup, play_game_with};

use crate::common::seeds::for_each_seed;

#[test]
fn bot_games_with_discard_casts_and_zombie_copies_keep_every_invariant_and_use_each_way() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                discard_cast_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut jump_started = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::JumpStart) => {
                        *seen.entry("jump-start").or_default() += 1;
                        jump_started.push(*object);
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Stack,
                        to,
                        ..
                    } if jump_started.contains(object) => {
                        assert_eq!(
                            *to,
                            Zone::Exile,
                            "a jump-started spell leaves the stack for exile"
                        );
                        *seen.entry("jump-start exiled").or_default() += 1;
                    }
                    Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Retrace) => {
                        *seen.entry("retrace").or_default() += 1;
                    }
                    Event::TokenCreated(token) if token.copy_of.is_some() => {
                        let definition = &token.definition;
                        assert_eq!(definition.mana_cost, None, "{} has a mana cost", token.name);
                        assert!(definition.subtypes.contains(&Subtype("Zombie".into())));
                        let eternalized = definition.power == Some(4);
                        let color = if eternalized {
                            Color::Black
                        } else {
                            Color::White
                        };
                        assert_eq!(definition.colors(), ColorSet::EMPTY.with(color));
                        *seen
                            .entry(if eternalized { "eternalize" } else { "embalm" })
                            .or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for way in [
        "jump-start",
        "jump-start exiled",
        "retrace",
        "embalm",
        "eternalize",
    ] {
        assert!(
            seen.get(way).copied().unwrap_or(0) > 0,
            "no card was played the way {way}: {seen:?}"
        );
    }
}
