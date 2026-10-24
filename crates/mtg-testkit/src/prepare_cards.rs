//! Prepare cards built in code, and a seeded deck of them for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::definition::Layout;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{free_creature, land};

/// The ability `json` writes.
fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
}

/// A prepare card: a creature face with `front` abilities and a spell face of `spell_type` with the `spell` ability.
fn prepare_card(
    (name, cost, power, toughness): (&str, &str, i32, i32),
    front: &[&str],
    (spell_name, spell_cost, spell_type): (&str, &str, CardType),
    spell: &str,
) -> Arc<CardDefinition> {
    let mut creature = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(ManaCost::parse(cost).unwrap_or_default())
        .with_power_toughness(power, toughness);
    for ability in front {
        creature = creature.with_rules(rules(ability));
    }
    let spell = CardDefinition::new(spell_name)
        .with_type(spell_type)
        .with_mana_cost(ManaCost::parse(spell_cost).unwrap_or_default())
        .with_rules(rules(spell));
    Arc::new(
        creature
            .clone()
            .with_faces(Layout::Prepare, vec![creature, spell]),
    )
}

/// "This creature enters prepared."
const ENTERS_PREPARED: &str =
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"EntersPrepared"}}"#;

/// "At the beginning of your upkeep, this creature becomes prepared."
const UPKEEP_PREPARES: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Prepare":{"what":"This"}}]}}"#;

/// "Whenever you attack, this creature becomes prepared." (the cards that attack in a fuzz game)
const ATTACK_PREPARES: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"ControlledBy":"You"}]}},"effects":[{"Prepare":{"what":"This"}}]}}"#;

/// Soothing Owl // Soothing Song: a 1/1 that enters prepared; the Sorcery gains 2 life.
pub fn owl() -> Arc<CardDefinition> {
    prepare_card(
        ("Soothing Owl", "{W}", 1, 1),
        &[ENTERS_PREPARED],
        ("Soothing Song", "{W}", CardType::Sorcery),
        r#"{"Spell":{"effects":[{"GainLife":{"amount":2}}]}}"#,
    )
}

/// Watchful Fox // Fire Bolt: a 2/1 that becomes prepared in its controller's upkeep; the Instant deals 3 damage to any target.
pub fn fox() -> Arc<CardDefinition> {
    prepare_card(
        ("Watchful Fox", "{R}", 2, 1),
        &[UPKEEP_PREPARES],
        ("Fire Bolt", "{R}", CardType::Instant),
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    )
}

/// Keen Hawk // Hawk's Gift: a 2/2 that enters prepared and becomes prepared when its controller attacks; the Sorcery
/// puts a +1/+1 counter on target creature.
pub fn hawk() -> Arc<CardDefinition> {
    prepare_card(
        ("Keen Hawk", "{G}", 2, 2),
        &[ENTERS_PREPARED, ATTACK_PREPARES],
        ("Hawk's Gift", "{G}", CardType::Sorcery),
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#,
    )
}

/// The prepare cards of the deck.
pub fn prepare_cards() -> [Arc<CardDefinition>; 3] {
    [owl(), fox(), hawk()]
}

/// A legal Normal deck of lands and four of each prepare card, derived entirely from `rng`.
pub fn prepare_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for card in prepare_cards() {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    // Creatures to attack with and to put the counters on.
    let fodder = [
        "Mite", "Gnat", "Brute", "Ogre", "Imp", "Wisp", "Troll", "Drake",
    ];
    for (index, name) in fodder.iter().cycle().take(24).enumerate() {
        deck.push(free_creature(
            name,
            1 + i32::try_from(index % 4).unwrap_or(0),
            2,
        ));
    }
    rng.shuffle(&mut deck);
    deck
}

/// A normal game of `players` seats whose decks are [`prepare_deck`]s, fully determined by `seed`.
pub fn prepare_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| prepare_deck(&mut GameRng::substream(seed, 90_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
