//! Goal: whole bot games with token makers in the decks (Dockside Extortionist, Hero of Bladehold, Tireless Tracker, a Food maker and a watcher that counts tokens) finish and keep every invariant (zones with tokens that cease to exist, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use tokens: Treasures are made and sacrificed for mana, Clues and Food are paid for and sacrificed, Soldiers enter attacking and tokens die or leave and cease to exist.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::format::Normal;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::play_game;
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

/// Gains 1 life whenever a token its controller controls goes to a graveyard.
fn mourner() -> Arc<CardDefinition> {
    let watch = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"ControlledBy":"You"},"Token"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Mourner")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(watch).unwrap()),
    )
}

/// Makes a Food when it enters.
fn cook() -> Arc<CardDefinition> {
    let make = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"food"}}}]}}"#;
    Arc::new(
        CardDefinition::new("Cook")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(make).unwrap()),
    )
}

/// Lands for both colours, token makers, artifacts for Dockside to count and creatures that kill.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    deck.extend((0..15).map(|_| Arc::new(mountain().as_ref().clone())));
    deck.extend((0..15).map(|_| Arc::new(plains().as_ref().clone())));
    for (name, copies) in [
        ("Dockside Extortionist", 4),
        ("Hero of Bladehold", 4),
        ("Tireless Tracker", 4),
        ("Sol Ring", 3),
        ("Pacifism", 3),
        ("Grizzly Bears", 2),
        ("Lightning Bolt", 3),
        ("Doom Blade", 2),
    ] {
        deck.extend((0..copies).map(|_| card(name).unwrap()));
    }
    deck.extend((0..2).map(|_| mourner()));
    deck.extend((0..3).map(|_| cook()));
    deck
}

#[derive(Default)]
struct Seen {
    treasures: usize,
    soldiers_attacking: usize,
    treasures_spent: usize,
    tokens_ceasing: usize,
    clues_and_food: usize,
    clues_and_food_paid: usize,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        let mut treasures = BTreeSet::new();
        let mut payable = BTreeSet::new();
        for event in events {
            match event {
                Event::TokenCreated(token) if token.name == "Treasure" => {
                    self.treasures += 1;
                    treasures.insert(token.object);
                }
                Event::TokenCreated(token) if token.name == "Clue" || token.name == "Food" => {
                    self.clues_and_food += 1;
                    payable.insert(token.object);
                }
                Event::TokenCreated(token) if token.attacking.is_some() => {
                    self.soldiers_attacking += 1;
                }
                Event::ManaAdded {
                    source: Some(source),
                    ..
                } if treasures.contains(source) => self.treasures_spent += 1,
                Event::AbilityActivated { source, .. } if payable.contains(&source.id) => {
                    self.clues_and_food_paid += 1;
                }
                Event::ZoneChanged {
                    to,
                    from: mtg_engine::zones::Zone::Battlefield,
                    ..
                } if !matches!(to, mtg_engine::zones::Zone::Battlefield) => {
                    self.tokens_ceasing += 1;
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_token_makers_finish_and_keep_every_invariant(#[case] seats: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(); seats]);
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&outcome.events);
    }
    assert!(seen.treasures > 0, "Treasures were made");
    assert!(seen.soldiers_attacking > 0, "Soldiers entered attacking");
    assert!(
        seen.treasures_spent > 0,
        "a Treasure was sacrificed for mana"
    );
    assert!(seen.clues_and_food > 0, "Clues and Food were made");
    assert!(
        seen.clues_and_food_paid > 0,
        "a Clue or Food was paid for and sacrificed"
    );
    assert!(seen.tokens_ceasing > 0, "permanents left the battlefield");
}
