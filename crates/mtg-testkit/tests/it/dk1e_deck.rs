//! Goal: the owner's Commander deck (Y'shtola, Night's Blessed; an Aura, Equipment and Umbra deck) is read from its saved list the way the server reads a deck, its cards are listed as Verified, Partial or Manual with what is missing, and whole bot games of it at two, three and four seats against precon decks and against itself finish under every invariant, the replay of the log included, with the commander cast from the command zone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_cards::support::Support;
use mtg_engine::events::Event;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, GameState, apply_event};
use mtg_testkit::fixture_decks::{FixtureDeck, commander_game, fixture_deck};
use mtg_testkit::{PlayOptions, play_game_with};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size and opponent kind; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(25)
}

/// The precon decks that stand in for the other seats.
const PRECONS: [&str; 3] = [
    "precon_calling_all_angels",
    "precon_wretched_ranks",
    "precon_tramplesaurus_rex",
];

/// The table text of a deck's statuses: one line per card that is not Verified, with what is missing.
fn status_table(deck: &FixtureDeck) -> String {
    let mut lines = vec![format!(
        "{}: {} Verified, {} Partial, {} Manual, {} other of {} distinct cards",
        deck.name,
        count(deck, Support::Verified),
        count(deck, Support::Partial),
        count(deck, Support::Manual),
        count(deck, Support::Stale) + count(deck, Support::Unplayable),
        deck.statuses.len(),
    )];
    for status in deck.not_verified() {
        lines.push(format!(
            "{:?} | {:?} | {:?} | {}",
            status.name,
            status.support,
            status.source,
            status.reasons.join("; ")
        ));
    }
    lines.join("\n")
}

fn count(deck: &FixtureDeck, support: Support) -> usize {
    deck.statuses
        .iter()
        .filter(|status| status.support == support)
        .count()
}

#[test]
fn the_owners_deck_lists_every_card_that_is_not_verified_with_what_is_missing() {
    let deck = fixture_deck("owner").unwrap();
    println!("{}", status_table(&deck));
    let copies: u32 = deck.statuses.iter().map(|status| status.copies).sum();
    assert_eq!(copies, 100, "the commander and 99 other cards");
    assert_eq!(deck.statuses[0].name, "Y'shtola, Night's Blessed");
    for status in deck.not_verified() {
        assert!(
            !status.reasons.is_empty(),
            "{} is {:?} and says nothing of why",
            status.name,
            status.support
        );
    }
}

#[rstest]
#[case::owner("owner")]
#[case::calling_all_angels(PRECONS[0])]
#[case::wretched_ranks(PRECONS[1])]
#[case::tramplesaurus_rex(PRECONS[2])]
fn every_saved_list_is_a_commander_and_ninety_nine_cards_of_known_cards(#[case] name: &str) {
    let deck = fixture_deck(name).unwrap();
    assert_eq!(deck.commanders.len(), 1, "{name}");
    assert_eq!(deck.cards.len(), 99, "{name}");
}

/// What a campaign of the owner's deck did.
#[derive(Default, Debug)]
struct Seen {
    games: u64,
    /// Casts of each card of the owner's deck by name, over all the games.
    casts: BTreeMap<String, u64>,
    /// Games in which Y'shtola was cast from the command zone.
    commander_cast: u64,
}

impl Seen {
    /// Counts the casts of one game by folding its log over its setup.
    fn add(&mut self, setup: &GameSetup, events: &[Event], owner: &FixtureDeck) {
        let mut state = GameState::from_setup(setup).unwrap();
        let mut from_command = false;
        for event in events {
            if let Event::SpellCast { object, .. } = event
                && let Some(card) = state.object(*object)
            {
                let name = card.card().name.clone();
                if owner.statuses.iter().any(|status| status.name == name) {
                    from_command |= matches!(card.zone(), Zone::Command(_));
                    *self.casts.entry(name).or_default() += 1;
                }
            }
            apply_event(&mut state, event).unwrap();
        }
        self.games += 1;
        self.commander_cast += u64::from(from_command);
    }
}

/// Plays the owner's deck in the seat `seed % seats` against `others(seed, seat)` for every other seat.
fn play_campaign(
    seats: usize,
    options: PlayOptions,
    others: impl Fn(u64, usize) -> &'static FixtureDeck + Sync,
) {
    let owner = decks().0;
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let at = usize::try_from(seed).unwrap() % seats;
        let table: Vec<&FixtureDeck> = (0..seats)
            .map(|seat| {
                if seat == at {
                    owner
                } else {
                    others(seed, seat)
                }
            })
            .collect();
        let setup = commander_game(&table, seed);
        let outcome = play_game_with(setup.clone(), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.lock().unwrap().add(&setup, &outcome.events, owner);
    });
    let seen = seen.into_inner().unwrap();
    eprintln!("{seats} seats: {seen:?}");
    assert!(
        seen.casts.values().sum::<u64>() > seen.games,
        "the games cast the deck's cards: {seen:?}"
    );
    if games() >= 25 {
        assert!(seen.commander_cast > 0, "Y'shtola was never cast: {seen:?}");
    }
}

/// The owner's deck and the precon decks, read once.
fn decks() -> (&'static FixtureDeck, &'static [FixtureDeck]) {
    static DECKS: std::sync::OnceLock<(FixtureDeck, Vec<FixtureDeck>)> = std::sync::OnceLock::new();
    let (owner, precons) = DECKS.get_or_init(|| {
        (
            fixture_deck("owner").unwrap(),
            PRECONS
                .iter()
                .map(|name| fixture_deck(name).unwrap())
                .collect(),
        )
    });
    (owner, precons)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_owners_deck_plays_against_precon_decks_under_every_invariant(#[case] seats: usize) {
    play_campaign(seats, PlayOptions::default(), |seed, seat| {
        let precons = decks().1;
        &precons[(usize::try_from(seed).unwrap() + seat) % precons.len()]
    });
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_owners_deck_plays_against_itself_under_every_invariant(#[case] seats: usize) {
    play_campaign(seats, PlayOptions::default(), |_, _| decks().0);
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_owners_deck_plays_under_the_servers_windows_with_payments_and_manual_actions(
    #[case] seats: usize,
) {
    // The cards the engine plays by hand are played with manual actions, by the seats and out of turn.
    let options = PlayOptions {
        auto_pass: true,
        partial_payments: true,
        symbol_choices: true,
        manual_every: 10,
        manual_anytime_every: 25,
        ..PlayOptions::default()
    };
    play_campaign(seats, options, |seed, seat| {
        let precons = decks().1;
        &precons[(usize::try_from(seed).unwrap() + seat) % precons.len()]
    });
}
