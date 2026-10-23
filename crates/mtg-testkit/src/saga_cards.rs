//! Sagas for the scenario and fuzz games: test Sagas whose chapters gain a different amount of life, the spells that put
//! lore counters on a Saga or return one with extra counters, the corpus Sagas, and the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, land};
use crate::corpus;

/// The corpus Sagas.
pub const SAGA_CARDS: &[&str] = &[
    "The Birth of Meletis",
    "Binding the Old Gods",
    "Book of Mazarbul",
    "Origin of the Hulk",
    "Summon: G.F. Ifrit",
    "History of Benalia",
    "Vault 75: Middle School",
    "Founding of Omashu",
];

fn rules(json: &str) -> Ability {
    // The abilities are written here, so a typo is a bug of the test cards.
    serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
}

/// A Saga costing `{W}` whose chapters gain life: each entry is the chapter numbers one line covers and the life it
/// gains, so the life a player gained says which chapters ran.
pub fn saga(name: &str, chapters: &[(&[u32], i32)]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_subtype("Saga")
        .with_mana_cost(cost(0, &[Color::White]));
    for (numbers, life) in chapters {
        for number in *numbers {
            card = card.with_rules(rules(&format!(
                r#"{{"Triggered":{{"when":{{"Chapter":{{"number":{number}}}}},"effects":[{{"GainLife":{{"amount":{life}}}}}]}}}}"#
            )));
        }
    }
    Arc::new(card)
}

/// An instant costing `{W}`: "Put `count` lore counters on target Saga."
pub fn lore_spell(name: &str, count: u32) -> Arc<CardDefinition> {
    let ability = rules(&format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Subtype":"Saga"}}]}}}}],"effects":[{{"PutCounters":{{"on":{{"Target":0}},"counter":{{"Other":"lore"}},"count":{count}}}}}]}}}}"#
    ));
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(ability),
    )
}

/// An instant costing `{W}`: "Return target Saga card from your graveyard to the battlefield with `extra` lore counters
/// on it."
pub fn recall_spell(name: &str, extra: u32) -> Arc<CardDefinition> {
    let ability = rules(&format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Subtype":"Saga"}},{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{"counters":[[{{"Other":"lore"}},{extra}]]}}}}}}}}]}}}}"#
    ));
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(ability),
    )
}

/// A legal Normal deck of lands, the corpus Sagas, the cards that put lore counters on them and bring them back, the
/// cards that double and add counters, and plain creatures, derived entirely from `rng`.
pub fn saga_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 5])).collect();
    for name in SAGA_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 2));
        }
    }
    for name in ["Doubling Season", "Hardened Scales"] {
        if let Ok(card) = corpus::card(name) {
            deck.push(card);
        }
    }
    deck.extend([
        lore_spell("Lore Surge", 2),
        lore_spell("Lore Trickle", 1),
        recall_spell("Retell", 1),
    ]);
    for slot in 0..14 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Helper {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(5)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`saga_deck`]s, fully determined by `seed`.
pub fn saga_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| saga_deck(&mut GameRng::substream(seed, 28_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
