//! Goal: whole bot games at two to four seats with the cards that give keywords to the cards of a hand (Rhet-Tomb Mystic, Tectonic Reformation, Jo Grant, Molecule Man) beside the cards they give them to finish with every invariant held, the bots paying in part and by hand, and the log of each game replays to its final state; across the games cards that print no cycling were cycled for a granted cycling, creature cards and land cards both.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, card_grants_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the games showed: cards cycled that print no cycling, split into creature cards and land cards, and the spells
/// cast in all.
#[derive(Default)]
struct Seen {
    granted_creatures: u64,
    granted_lands: u64,
    casts: u64,
}

#[test]
fn bot_games_with_cards_that_give_keywords_to_a_hand_keep_every_invariant_and_replay() {
    let seen = Mutex::new(Seen::default());
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        for (players, options) in [
            (2, PlayOptions::default()),
            (
                3,
                PlayOptions {
                    partial_payments: true,
                    ..PlayOptions::default()
                },
            ),
            (
                4,
                PlayOptions {
                    partial_payments: true,
                    symbol_choices: true,
                    ..PlayOptions::default()
                },
            ),
        ] {
            let setup = card_grants_fuzz_setup(players, seed);
            let outcome =
                play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
            let setup = card_grants_fuzz_setup(players, seed);
            let replayed = replay(&setup, &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::SpellCast { .. } => seen.casts += 1,
                    Event::Cycled { object, .. } => {
                        let Some(card) = replayed
                            .object(*object)
                            .map(mtg_engine::object::GameObject::card)
                        else {
                            continue;
                        };
                        let prints_cycling = card
                            .rules
                            .iter()
                            .any(|rules| matches!(&**rules, Ability::Keyword(Keyword::Cycling(_))));
                        if prints_cycling {
                            continue;
                        }
                        if card.types.contains(&CardType::Land) {
                            seen.granted_lands += 1;
                        } else if card.types.contains(&CardType::Creature) {
                            seen.granted_creatures += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(
        seen.granted_creatures > 0,
        "some creature card was cycled for a granted cycling"
    );
    assert!(
        seen.granted_lands > 0,
        "some land card was cycled for a granted cycling"
    );
}
