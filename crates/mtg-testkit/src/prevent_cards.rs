//! Damage prevention and regeneration (CR 615, 701.19) and the decks and games built from them: the corpus cards that
//! prevent damage or regenerate, with the spells that deal damage (Stomp, which stops prevention) and destroy for them to act on.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::{CardDefinition, builtin};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, elf, land};
use crate::corpus::{self, CorpusError};

/// Lands in a prevention deck: five colours in turn.
const LANDS: usize = 20;

/// Cards of the corpus the prevention deck holds, with how many of each.
pub const PREVENT_CARDS: [(&str, usize); 5] = [
    ("Fog", 3),
    ("Mending Hands", 3),
    ("Samite Healer", 3),
    ("Drudge Skeletons", 3),
    ("Death Ward", 3),
];

/// Cards of the corpus that deal damage, destroy and fight over the shields.
const CORPUS_CARDS: [(&str, usize); 14] = [
    ("Wrath of God", 2),
    ("Supreme Verdict", 1),
    ("Doom Blade", 2),
    ("Lightning Bolt", 3),
    ("Fireball", 1),
    ("Swords to Plowshares", 1),
    ("Bonecrusher Giant", 2),
    ("Furnace of Rath", 1),
    ("Serra Angel", 2),
    ("Hero of Bladehold", 1),
    ("Llanowar Elves", 1),
    ("Glorious Anthem", 1),
    ("Counterspell", 1),
    ("Brainstorm", 1),
];

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// `{1}` instant: destroy target creature (it can be regenerated).
pub fn smite(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[]))
            .with_rules(rules(spell)?),
    ))
}

/// Stomp, the adventure of Bonecrusher Giant, as an instant of its own for `{1}{R}`: the rules are those of its face in
/// the card file (damage can't be prevented this turn, 2 damage to any target).
pub fn stomp() -> Result<Arc<CardDefinition>, CorpusError> {
    let library =
        builtin::library().map_err(|errors| CorpusError::Data(errors.to_string()))?;
    let face = library
        .card_by_name("Stomp")
        .and_then(|card| card.faces.iter().find(|face| face.name == "Stomp"))
        .ok_or_else(|| CorpusError::UnknownCard("Stomp".to_owned()))?;
    Ok(Arc::new(
        face.abilities.iter().cloned().fold(
            CardDefinition::new("Stomp")
                .with_type(CardType::Instant)
                .with_mana_cost(cost(1, &[Color::Red])),
            CardDefinition::with_rules,
        ),
    ))
}

/// A legal Normal deck of shields and the damage and destruction they answer, derived entirely from `rng`.
pub fn prevent_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Green,
        Color::Blue,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in PREVENT_CARDS.into_iter().chain(CORPUS_CARDS) {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (smite("Smite")?, 2),
        (stomp()?, 3),
        (burn("Spark", 2), 2),
        (burn("Flame", 4), 1),
        (bear(), 3),
        (elf(), 1),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`prevent_deck`]s, fully determined by `seed`.
pub fn prevent_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(prevent_deck(&mut GameRng::substream(seed, 22_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the destroy spell of this module is a valid rule the engine runs, Stomp is an instant with the rules of the face in its card file, and a prevention deck is a legal sixty or more cards with every prevention card and every card they answer in it, fixed by its seed.
    use super::*;

    #[test]
    fn the_destroy_spell_has_one_ability_and_costs_one() {
        let card = smite("Smite").unwrap();
        assert_eq!(card.rules.len(), 1);
        assert_eq!(card.mana_cost.as_ref(), Some(&cost(1, &[])));
    }

    #[test]
    fn stomp_is_an_instant_for_two_with_the_rules_of_its_face() {
        let card = stomp().unwrap();
        assert_eq!(card.name, "Stomp");
        assert_eq!(card.mana_cost.as_ref(), Some(&cost(1, &[Color::Red])));
        assert_eq!(card.rules.len(), 1);
        let text = format!("{:?}", card.rules);
        assert!(text.contains("DamageCantBePrevented"), "{text}");
        assert!(text.contains("DealDamage"), "{text}");
    }

    #[test]
    fn a_prevention_deck_holds_every_prevention_card_and_is_fixed_by_its_seed() {
        let deck = prevent_deck(&mut GameRng::substream(7, 1)).unwrap();
        assert!(deck.len() >= 60, "{}", deck.len());
        assert!(deck.iter().any(|card| card.name == "Stomp"));
        for (name, _) in PREVENT_CARDS.into_iter().chain(CORPUS_CARDS) {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let again = prevent_deck(&mut GameRng::substream(7, 1)).unwrap();
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&deck), names(&again));
    }

    #[test]
    fn the_prevention_cards_of_the_corpus_carry_their_shields() {
        for (name, shield) in [
            ("Fog", "Shield"),
            ("Mending Hands", "Shield"),
            ("Samite Healer", "Shield"),
            ("Drudge Skeletons", "Regenerate"),
            ("Death Ward", "Regenerate"),
        ] {
            let card = corpus::card(name).unwrap();
            let text = format!("{:?}", card.rules);
            assert!(text.contains(shield), "{name}: {text}");
        }
    }
}
