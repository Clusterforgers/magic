//! Fights (CR 701.14) and decayed creatures (CR 702.147), and the decks and games built from them: the corpus cards
//! that fight or give decayed, with a synthetic spell that makes two creatures fight and creatures with the keywords a
//! fight has to respect.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, free_creature, keyworded_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a fight deck: five colours in turn.
const LANDS: usize = 25;

/// Cards of the corpus the fight deck holds, with how many of each.
pub const CORPUS_CARDS: &[(&str, usize)] = &[
    ("Warbriar Blessing", 3),
    ("Lightning Bolt", 2),
    ("Giant Growth", 2),
    ("Mending Hands", 1),
    ("Doom Blade", 1),
    ("Jadar, Ghoulcaller of Nephalia", 1),
    ("Curse of the Restless Dead", 2),
    ("Level Up", 1),
    ("Idolized", 1),
    ("Elemental Mastery", 1),
];

/// The spells whose resolution is a fight.
pub const FIGHT_SPELLS: &[&str] = &["Brawl", "Warbriar Blessing"];

/// The creature cards of the deck that have decayed.
pub const DECAYED_CARDS: &[&str] = &["Zombie Husk"];

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// An instant for `{R}`: target creature you control fights target creature you don't control.
pub fn brawl(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Fight":{"what":{"Target":0},"with":{"Target":1}}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(rules(spell)?),
    ))
}

/// A legal Normal deck of fighters and the spells and keywords a fight meets, derived entirely from `rng`.
pub fn fight_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::White,
        Color::Black,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &(name, copies) in CORPUS_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (brawl("Brawl")?, 4),
        (burn("Spark", 2), 2),
        (bear(), 4),
        (free_creature("Ogre Brute", 3, 3), 4),
        (
            keyworded_creature(
                "Pit Viper",
                0,
                &[Color::Green],
                1,
                1,
                &[Keyword::Deathtouch],
            ),
            3,
        ),
        (
            keyworded_creature(
                "Blood Knight",
                0,
                &[Color::White],
                2,
                2,
                &[Keyword::Lifelink],
            ),
            3,
        ),
        (
            keyworded_creature("Wall", 0, &[Color::Blue], 0, 4, &[Keyword::Defender]),
            2,
        ),
        (
            keyworded_creature("Zombie Husk", 0, &[Color::Black], 2, 2, &[Keyword::Decayed]),
            4,
        ),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`fight_deck`]s, fully determined by `seed`.
pub fn fight_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(fight_deck(&mut GameRng::substream(seed, 38_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the fight spell is a valid rule with two targets and one fight, and a fight deck is a legal sixty or more cards with every listed card in it, fixed by its seed.
    use super::*;

    #[test]
    fn the_fight_spell_names_two_targets_and_one_fight() {
        let card = brawl("Brawl").unwrap();
        let [spell] = card.rules.as_slice() else {
            panic!("one spell ability, got {:?}", card.rules);
        };
        let Ability::Spell { targets, effects } = spell.as_ref() else {
            panic!("a spell ability, got {spell:?}");
        };
        assert_eq!((targets.len(), effects.len()), (2, 1));
    }

    #[test]
    fn a_deck_holds_every_listed_card_and_is_fixed_by_its_seed() {
        let deck = fight_deck(&mut GameRng::substream(1, 38_000)).unwrap();
        assert!(deck.len() >= 60);
        for &(name, _) in CORPUS_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = fight_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&fight_fuzz_setup(3, 5).unwrap()));
    }
}
