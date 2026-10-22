//! Goal: whole bot games at two, three and four seats with foretell cards (Behold the Multiverse, Saw It Coming, Demon Bolt, Augury Raven) keep every invariant, and across the seeds cards are foretold, stay in exile face down, and are cast for their foretell cost on a later turn than the one they were foretold in.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, foretell_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_foretell_cards_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(20);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                foretell_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The turn each card was foretold in, by object: a cast for the foretell cost is later.
            let mut foretold_in: BTreeMap<_, u32> = BTreeMap::new();
            let mut turn = 0;
            for event in &outcome.events {
                match event {
                    Event::TurnBegan { number, .. } => turn = *number,
                    Event::ZoneChanged {
                        object,
                        from: Zone::Hand(_),
                        to: Zone::Exile,
                        cause: ZoneCause::Foretold,
                    } => {
                        foretold_in.insert(*object, turn);
                        *seen.entry("foretold").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Exile,
                        ..
                    } => {
                        if foretold_in.remove(object).is_some() {
                            *seen.entry("left exile").or_default() += 1;
                        }
                    }
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::Foretold) => {
                        let foretold = foretold_in
                            .get(object)
                            .copied()
                            .expect("only a foretold card is cast for the foretell cost");
                        assert!(foretold < turn, "cast in the turn it was foretold in");
                        *seen.entry("cast for the foretell cost").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["foretold", "cast for the foretell cost"] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
}
