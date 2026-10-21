//! Planeswalkers (CR 306, 606), and the decks and games built from them: the corpus planeswalkers, two more that burn,
//! destroy, make tokens and gain life, and the creatures, burn and pumps that attack and kill them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, keyworded_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a planeswalker deck: five colours in turn.
const LANDS: usize = 22;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A legendary planeswalker of this cost and starting loyalty with these abilities, written as data.
fn walker(
    name: &str,
    (generic, colored): (u32, &[Color]),
    loyalty: u32,
    abilities: &[&str],
) -> Result<Arc<CardDefinition>, CorpusError> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Planeswalker)
        .with_mana_cost(cost(generic, colored))
        .with_loyalty(loyalty);
    card.supertypes.insert(Supertype::Legendary);
    for ability in abilities {
        card = card.with_rules(rules(ability)?);
    }
    Ok(Arc::new(card))
}

/// `{1}{R}` planeswalker, loyalty 3: +1: it deals 1 damage to any target. -2: it deals 3 damage to target creature.
pub fn pyre_walker() -> Result<Arc<CardDefinition>, CorpusError> {
    walker(
        "Pyre Walker",
        (1, &[Color::Red]),
        3,
        &[
            r#"{"Loyalty":{"cost":{"Plus":1},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
            r#"{"Loyalty":{"cost":{"Minus":2},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
        ],
    )
}

/// `{2}{W}` planeswalker, loyalty 4: +1: you gain 2 life. 0: create a 1/1 white Soldier creature token. -3: destroy
/// target creature.
pub fn dawn_walker() -> Result<Arc<CardDefinition>, CorpusError> {
    walker(
        "Dawn Walker",
        (2, &[Color::White]),
        4,
        &[
            r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":2}}]}}"#,
            r#"{"Loyalty":{"cost":"Zero","effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}}]}}"#,
            r#"{"Loyalty":{"cost":{"Minus":3},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        ],
    )
}

/// `{2}{G}` 4/4 creature with trample.
pub fn charging_rhino() -> Arc<CardDefinition> {
    keyworded_creature(
        "Charging Rhino",
        2,
        &[Color::Green],
        4,
        4,
        &[Keyword::Trample],
    )
}

/// `{1}{U}` 2/2 creature with flying.
pub fn skyhawk() -> Arc<CardDefinition> {
    keyworded_creature("Skyhawk", 1, &[Color::Blue], 2, 2, &[Keyword::Flying])
}

/// The corpus planeswalkers.
pub const LOYALTY_CORPUS_CARDS: [&str; 4] = [
    "Jace, the Mind Sculptor",
    "Jace Beleren",
    "Ajani, Caller of the Pride",
    "Garruk Wildspeaker",
];

/// The cards of this module and the plain ones the deck is made of.
pub const LOYALTY_SYNTHETIC_CARDS: [&str; 6] = [
    "Pyre Walker",
    "Dawn Walker",
    "Charging Rhino",
    "Skyhawk",
    "Spark",
    "Grizzly Bears",
];

/// A legal Normal deck in five colours of planeswalkers and what attacks and kills them: the corpus planeswalkers, the
/// two above, creatures (one with trample, one with flying), burn that can hit a planeswalker and pumps.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn loyalty_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Red,
        Color::White,
        Color::Green,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in [
        ("Jace, the Mind Sculptor", 2),
        ("Jace Beleren", 2),
        ("Ajani, Caller of the Pride", 2),
        ("Garruk Wildspeaker", 2),
        ("Lightning Bolt", 4),
        ("Giant Growth", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (pyre_walker()?, 3),
        (dawn_walker()?, 3),
        (charging_rhino(), 4),
        (skyhawk(), 4),
        (burn("Spark", 2), 4),
        (bear(), 4),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`loyalty_deck`]s, fully determined by `seed`.
pub fn loyalty_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(loyalty_deck(&mut GameRng::substream(seed, 17_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the planeswalkers of this module use only what the engine runs and are valid rules, a loyalty deck is a legal sixty cards with every planeswalker and every card that attacks or kills one in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn the_synthetic_planeswalkers_use_primitives_the_engine_runs() {
        let caps = caps();
        for card in [pyre_walker().unwrap(), dawn_walker().unwrap()] {
            assert!(card.is_type(CardType::Planeswalker), "{}", card.name);
            assert!(card.loyalty.is_some_and(|loyalty| loyalty > 0));
            assert!(!card.rules.is_empty());
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn a_loyalty_deck_is_sixty_cards_with_every_planeswalker_and_what_attacks_and_kills_them() {
        let deck = loyalty_deck(&mut GameRng::substream(1, 17_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in LOYALTY_CORPUS_CARDS.iter().chain(&LOYALTY_SYNTHETIC_CARDS) {
            assert!(deck.iter().any(|card| card.name == *name), "{name}");
        }
        let setup = loyalty_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&loyalty_fuzz_setup(3, 5).unwrap()));
    }
}
