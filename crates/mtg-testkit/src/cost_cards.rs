//! Costs of activated abilities beyond `{T}`, mana and the sacrifice of the source (CR 118.3, 602.2): life paid, counters
//! removed, a permanent sacrificed, tapped or returned, a card discarded or exiled from a graveyard, and cycling, and
//! the decks and games built from them, which hold the corpus cards that pay these costs. Each ability draws or gains
//! life when it resolves, so a test sees what was paid.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a cost deck: three colours in turn.
const LANDS: usize = 28;

fn rules(json: &str) -> Ability {
    // The abilities are written here, so a typo is a bug of the test cards.
    serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
}

/// A creature costing `generic` and `colored`, with these abilities written as data.
fn creature_with(
    name: &str,
    (generic, colored): (u32, &[Color]),
    abilities: &[&str],
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost(generic, colored))
        .with_power_toughness(1, 1);
    for ability in abilities {
        card = card.with_rules(rules(ability));
    }
    Arc::new(card)
}

/// An artifact costing `generic`, with these abilities written as data.
fn artifact_with(name: &str, generic: u32, abilities: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Artifact)
        .with_mana_cost(cost(generic, &[]));
    for ability in abilities {
        card = card.with_rules(rules(ability));
    }
    Arc::new(card)
}

const DRAW: &str = r#"[{"Draw":{}}]"#;

/// `{1}{B}` creature: "Sacrifice a creature: Draw a card."
pub fn sacrificer(name: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[{{"Sacrifice":{{"filter":[{{"Type":"Creature"}}]}}}}],"effects":{DRAW}}}}}"#
    );
    creature_with(name, (1, &[Color::Black]), &[&ability])
}

/// `{R}` creature: "{T}, Discard a card: Draw a card."
pub fn rummager(name: &str) -> Arc<CardDefinition> {
    let ability =
        format!(r#"{{"Activated":{{"cost":["Tap",{{"Discard":{{}}}}],"effects":{DRAW}}}}}"#);
    creature_with(name, (0, &[Color::Red]), &[&ability])
}

/// `{B}` creature: "Exile a creature card from your graveyard: You gain 2 life."
pub fn gravedigger(name: &str) -> Arc<CardDefinition> {
    let ability = r#"{"Activated":{"cost":[{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}],"effects":[{"GainLife":{"amount":2}}]}}"#;
    creature_with(name, (0, &[Color::Black]), &[ability])
}

/// `{2}` artifact: "Tap two untapped creatures you control: Draw a card."
pub fn drummer(name: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[{{"TapPermanents":{{"filter":[{{"Type":"Creature"}}],"count":2}}}}],"effects":{DRAW}}}}}"#
    );
    artifact_with(name, 2, &[&ability])
}

/// `{1}` artifact: "{1}, Tap an untapped creature you control: Draw a card."
pub fn levy(name: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}"}},{{"TapPermanents":{{"filter":[{{"Type":"Creature"}}]}}}}],"effects":{DRAW}}}}}"#
    );
    artifact_with(name, 1, &[&ability])
}

/// `{B}` 1/1 creature: "Pay 3 life: Draw a card."
pub fn bleeder(name: &str) -> Arc<CardDefinition> {
    let ability = format!(r#"{{"Activated":{{"cost":[{{"PayLife":3}}],"effects":{DRAW}}}}}"#);
    creature_with(name, (0, &[Color::Black]), &[&ability])
}

/// `{2}` artifact that enters with three charge counters: "Remove a charge counter from this artifact: You gain 1 life."
pub fn charger(name: &str) -> Arc<CardDefinition> {
    let enters = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"charge","count":3}}}}"#;
    let ability = r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"charge"}}],"effects":[{"GainLife":{"amount":1}}]}}"#;
    artifact_with(name, 2, &[enters, ability])
}

/// `{1}{U}` creature: "{1}, Return a land you control to its owner's hand: Draw a card."
pub fn returner(name: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}"}},{{"ReturnToHand":{{"filter":[{{"Type":"Land"}}]}}}}],"effects":{DRAW}}}}}"#
    );
    creature_with(name, (1, &[Color::Blue]), &[&ability])
}

/// `{2}{R}` 1/1 creature: "{1}, {T}, Pay 1 life, Sacrifice a creature, Discard a card: You gain 3 life." Every kind of
/// part at once, to see them announced in the order the cost prints them.
pub fn gauntlet(name: &str) -> Arc<CardDefinition> {
    let ability = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap",{"PayLife":1},{"Sacrifice":{"filter":[{"Type":"Creature"}]}},{"Discard":{}}],"effects":[{"GainLife":{"amount":3}}]}}"#;
    creature_with(name, (2, &[Color::Red]), &[ability])
}

/// `{G}` creature: "{T}: Add {G}."
pub fn mana_elf(name: &str) -> Arc<CardDefinition> {
    let ability =
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;
    creature_with(name, (0, &[Color::Green]), &[ability])
}

/// `{3}{R}` 3/3 creature with `Cycling {1}`.
pub fn cycler(name: &str) -> Arc<CardDefinition> {
    let cycling = Keyword::Cycling(mtg_cards::cost::Cost::mana(
        "{1}".parse().unwrap_or_else(|_| unreachable!()),
    ));
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(3, &[Color::Red]))
            .with_power_toughness(3, 3)
            .with_rules(Ability::Keyword(cycling)),
    )
}

/// `{2}{U}` 2/2 creature with "Cycling—Pay 2 life."
pub fn life_cycler(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(2, &[Color::Blue]))
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Cycling(mtg_cards::cost::Cost(
                vec![mtg_cards::cost::CostItem::PayLife(
                    mtg_cards::value::Amount::Fixed(2),
                )],
            )))),
    )
}

/// The deck of a seat: lands in three colours and every card with a cost of this kind, with creatures and burn for the
/// costs to bear on.
pub fn cost_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Black, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (card, copies) in [
        (sacrificer("Sacrificer"), 3),
        (rummager("Rummager"), 2),
        (gravedigger("Gravedigger"), 2),
        (drummer("Drummer"), 2),
        (levy("Levy"), 2),
        (bleeder("Bleeder"), 2),
        (charger("Charger"), 2),
        (returner("Returner"), 2),
        (gauntlet("Gauntlet"), 1),
        (cycler("Cycler"), 3),
        (life_cycler("Life Cycler"), 2),
        (bear(), 4),
        (creature("Ogre", 2, &[Color::Red], 3, 3), 2),
        (burn("Burn", 3), 3),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    for (name, copies) in [
        ("Flooded Strand", 2),
        ("Nantuko Husk", 2),
        ("Rummaging Goblin", 2),
        ("Soul Shepherd", 2),
        ("Llanowar Behemoth", 2),
        ("Tumble Magnet", 2),
        ("Lurching Rotbeast", 2),
        ("Souldrinker", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A game of `players` seats with cost decks.
pub fn costs_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(cost_deck(&mut GameRng::substream(seed, 17_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
