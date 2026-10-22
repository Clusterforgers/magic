//! Cards with an X (CR 107.3): spells and abilities that cost `{X}` and a planeswalker whose ability costs `-X`, and the
//! decks and games built from them. Each reads X as it resolves, so a test sees the number the player announced.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, creature, land};
use crate::corpus::CorpusError;

/// Lands in an X deck: three colours in turn.
const LANDS: usize = 24;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

fn mana(text: &str) -> Result<ManaCost, CorpusError> {
    ManaCost::parse(text).map_err(|error| CorpusError::Data(format!("{text}: {error}")))
}

/// A card of this type whose mana cost is `rest` besides `x` symbols of `{X}`, with these abilities written as data.
fn card(
    name: &str,
    kind: CardType,
    (rest, x): (&str, u8),
    abilities: &[&str],
) -> Result<Arc<CardDefinition>, CorpusError> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(mana(rest)?)
        .with_x_in_cost(x);
    for ability in abilities {
        card = card.with_rules(rules(ability)?);
    }
    Ok(Arc::new(card))
}

/// A sorcery for `{X}{R}`: it deals X damage to any target.
pub fn blaze() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "Blaze",
        CardType::Sorcery,
        ("{R}", 1),
        &[
            r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#,
        ],
    )
}

/// An instant for `{X}{U}{U}`: target player draws X cards.
pub fn stroke() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "Stroke",
        CardType::Instant,
        ("{U}{U}", 1),
        &[
            r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0},"count":"X"}}]}}"#,
        ],
    )
}

/// An instant for `{X}{X}{W}`: you gain X life, so each X costs X.
pub fn twin_font() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "Twin Font",
        CardType::Instant,
        ("{W}", 2),
        &[r#"{"Spell":{"effects":[{"GainLife":{"amount":"X"}}]}}"#],
    )
}

/// An instant for `{X}{R/W}`: you gain X life; the hybrid symbol is paid with either colour.
pub fn hybrid_font() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "Hybrid Font",
        CardType::Instant,
        ("{R/W}", 1),
        &[r#"{"Spell":{"effects":[{"GainLife":{"amount":"X"}}]}}"#],
    )
}

/// An artifact for `{1}` with "{X}{R}, {T}: it deals X damage to any target."
pub fn x_pinger() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "X Pinger",
        CardType::Artifact,
        ("{1}", 0),
        &[
            r#"{"Activated":{"cost":[{"Mana":"{X}{R}"},"Tap"],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#,
        ],
    )
}

/// An artifact for `{2}` with "{X}{X}, {T}: you gain X life."
pub fn twin_well() -> Result<Arc<CardDefinition>, CorpusError> {
    card(
        "Twin Well",
        CardType::Artifact,
        ("{2}", 0),
        &[
            r#"{"Activated":{"cost":[{"Mana":"{X}{X}"},"Tap"],"effects":[{"GainLife":{"amount":"X"}}]}}"#,
        ],
    )
}

/// A legendary planeswalker for `{2}{R}{R}`, loyalty 4: +1: you gain 1 life. -X: it deals X damage to any target.
pub fn x_walker() -> Result<Arc<CardDefinition>, CorpusError> {
    let mut walker = CardDefinition::new("X Walker")
        .with_type(CardType::Planeswalker)
        .with_mana_cost(mana("{2}{R}{R}")?)
        .with_loyalty(4);
    walker.supertypes.insert(Supertype::Legendary);
    for ability in [
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
        r#"{"Loyalty":{"cost":"MinusX","targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#,
    ] {
        walker = walker.with_rules(rules(ability)?);
    }
    Ok(Arc::new(walker))
}

/// The cards of this module.
pub const X_CARDS: &[&str] = &[
    "Blaze",
    "Stroke",
    "Twin Font",
    "Hybrid Font",
    "X Pinger",
    "Twin Well",
    "X Walker",
];

/// A legal Normal deck of lands in three colours, the seven cards above and creatures to aim them at, derived entirely
/// from `rng`.
pub fn x_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Blue, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for card in [
        blaze()?,
        stroke()?,
        twin_font()?,
        hybrid_font()?,
        x_pinger()?,
        twin_well()?,
        x_walker()?,
    ] {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    deck.extend(std::iter::repeat_n(burn("Spark", 2), 3));
    deck.extend(std::iter::repeat_n(bear(), 4));
    for slot in 0..4 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("X Fodder {slot} {power}/{power}");
        let fodder = creature(&name, 1, &[colors[rng.index(3)]], power, power);
        deck.extend(std::iter::repeat_n(fodder, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`x_deck`]s, fully determined by `seed`.
pub fn x_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(x_deck(&mut GameRng::substream(seed, 19_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the cards of this module are valid rules that use only what the engine runs, each X card has the X its cost says, and an X deck is a legal sixty cards with every X card in it and a game built from one is fixed by its seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    fn all() -> Vec<Arc<CardDefinition>> {
        vec![
            blaze().unwrap(),
            stroke().unwrap(),
            twin_font().unwrap(),
            hybrid_font().unwrap(),
            x_pinger().unwrap(),
            twin_well().unwrap(),
            x_walker().unwrap(),
        ]
    }

    #[test]
    fn the_cards_use_primitives_the_engine_runs() {
        let caps = caps();
        for card in all() {
            assert_ne!(card.rules.len(), 0, "{}", card.name);
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn the_names_match_the_list() {
        let names: Vec<String> = all().iter().map(|card| card.name.clone()).collect();
        assert_eq!(names, X_CARDS);
    }

    #[test]
    fn the_x_of_a_cost_is_the_number_of_x_symbols_printed() {
        let symbols: Vec<u8> = all().iter().map(|card| card.x_in_cost).collect();
        assert_eq!(symbols, [1, 1, 2, 1, 0, 0, 0]);
    }

    #[test]
    fn a_deck_is_sixty_cards_with_every_x_card_and_a_seed_fixes_it() {
        let deck = |seed: u64| x_deck(&mut GameRng::substream(seed, 19_000)).unwrap();
        let first = deck(5);
        assert_eq!(first.len(), 60);
        for &name in X_CARDS {
            assert!(first.iter().any(|card| card.name == name), "{name}");
        }
        let names = |cards: &[Arc<CardDefinition>]| -> Vec<String> {
            cards.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&first), names(&deck(5)));
        assert_ne!(names(&first), names(&deck(6)));
    }
}
