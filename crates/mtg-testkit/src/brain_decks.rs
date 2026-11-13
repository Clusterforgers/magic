//! Duel decks for measuring bot brains against each other: a red-green deck of ramp, burn, pumps and tramplers, a
//! white-black deck of removal, auras, flyers and first strikers, a blue-red deck of counters, burn and flash
//! creatures, and two interaction-heavy control decks (blue-black and blue-white: counters, instant removal, card draw,
//! a wipe, flash threats), each 60 cards of basics, corpus cards and keyworded creatures.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;

use crate::cards::{keyworded_creature, land};
use crate::corpus::{self, CorpusError};

/// One of the duel decks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duel {
    /// Red-green: Llanowar Elves, Rampant Growth, Lightning Bolt, Giant Growth, Rancor, Fireball and big tramplers.
    Gruul,
    /// White-black: Doom Blade, Swords to Plowshares, Pacifism, Serra Angel, flyers, lifelinkers and first strikers.
    Orzhov,
    /// Blue-red: Counterspell, Dissipate, Lightning Bolt, flash creatures and flyers.
    Izzet,
    /// Blue-black control: Counterspell, Mana Drain, Memory Lapse, Doom Blade, Snuff Out, Fact or Fiction, Opt and
    /// flash creatures.
    Dimir,
    /// Blue-white control: Counterspell, Hinder, Essence Capture, Swords to Plowshares, Path to Exile, Wrath of God,
    /// Into the Roil, Brainstorm and flash flyers.
    Azorius,
}

impl Duel {
    /// Every duel deck.
    pub const ALL: [Duel; 5] = [
        Duel::Gruul,
        Duel::Orzhov,
        Duel::Izzet,
        Duel::Dimir,
        Duel::Azorius,
    ];

    /// The decks whose games turn on interaction: counters, instant removal and flash.
    pub const INTERACTION: [Duel; 3] = [Duel::Izzet, Duel::Dimir, Duel::Azorius];
}

/// A creature line: name, generic mana, coloured symbols, power, toughness, keywords, copies.
type Line = (
    &'static str,
    u32,
    &'static [Color],
    i32,
    i32,
    &'static [Keyword],
    usize,
);

const GRUUL: &[Line] = &[
    (
        "Duel Raptor",
        1,
        &[Color::Red],
        2,
        1,
        &[Keyword::FirstStrike, Keyword::Haste],
        4,
    ),
    ("Duel Brute", 1, &[Color::Red], 3, 2, &[], 4),
    ("Duel Ogre", 2, &[Color::Red], 3, 3, &[], 3),
    (
        "Duel Spider",
        1,
        &[Color::Green],
        2,
        4,
        &[Keyword::Reach],
        3,
    ),
    (
        "Duel Trampler",
        2,
        &[Color::Green],
        3,
        3,
        &[Keyword::Trample],
        4,
    ),
    (
        "Duel Wurm",
        4,
        &[Color::Green, Color::Green],
        5,
        5,
        &[Keyword::Trample],
        3,
    ),
    ("Duel Dragon", 4, &[Color::Red], 4, 4, &[Keyword::Flying], 2),
];

const GRUUL_SPELLS: &[(&str, usize)] = &[
    ("Llanowar Elves", 4),
    ("Rampant Growth", 2),
    ("Grizzly Bears", 4),
    ("Lightning Bolt", 4),
    ("Giant Growth", 3),
    ("Rancor", 2),
    ("Fireball", 1),
];

const ORZHOV: &[Line] = &[
    ("Duel Bat", 0, &[Color::Black], 1, 2, &[Keyword::Flying], 4),
    (
        "Duel Assassin",
        0,
        &[Color::Black],
        1,
        1,
        &[Keyword::Deathtouch],
        3,
    ),
    (
        "Duel Cleric",
        0,
        &[Color::White],
        1,
        3,
        &[Keyword::Lifelink],
        2,
    ),
    (
        "Duel Knight",
        1,
        &[Color::White],
        2,
        2,
        &[Keyword::FirstStrike],
        4,
    ),
    (
        "Duel Vampire",
        1,
        &[Color::Black],
        2,
        2,
        &[Keyword::Lifelink],
        4,
    ),
    (
        "Duel Specter",
        2,
        &[Color::Black],
        2,
        2,
        &[Keyword::Flying],
        3,
    ),
    (
        "Duel Paladin",
        2,
        &[Color::White],
        3,
        3,
        &[Keyword::Vigilance],
        4,
    ),
    (
        "Duel Wall",
        1,
        &[Color::White],
        0,
        5,
        &[Keyword::Defender],
        2,
    ),
    (
        "Duel Horror",
        3,
        &[Color::Black],
        4,
        4,
        &[Keyword::Menace],
        4,
    ),
    ("Duel Giant", 3, &[Color::White, Color::White], 5, 5, &[], 2),
];

const ORZHOV_SPELLS: &[(&str, usize)] = &[
    ("Doom Blade", 3),
    ("Swords to Plowshares", 3),
    ("Pacifism", 2),
    ("Serra Angel", 3),
];

const IZZET: &[Line] = &[
    (
        "Duel Drake",
        1,
        &[Color::Blue],
        2,
        2,
        &[Keyword::Flying, Keyword::Flash],
        4,
    ),
    ("Duel Wizard", 1, &[Color::Blue], 1, 3, &[Keyword::Flash], 4),
    ("Duel Elemental", 2, &[Color::Red], 3, 3, &[], 4),
    (
        "Duel Sphinx",
        3,
        &[Color::Blue, Color::Blue],
        4,
        4,
        &[Keyword::Flying],
        4,
    ),
    (
        "Duel Raptor",
        1,
        &[Color::Red],
        2,
        1,
        &[Keyword::FirstStrike, Keyword::Haste],
        4,
    ),
    ("Duel Brute", 1, &[Color::Red], 3, 2, &[], 4),
    ("Duel Serpent", 4, &[Color::Blue], 5, 5, &[], 4),
    ("Duel Ogre", 2, &[Color::Red], 3, 3, &[], 4),
];

const IZZET_SPELLS: &[(&str, usize)] =
    &[("Counterspell", 4), ("Dissipate", 3), ("Lightning Bolt", 4)];

const DIMIR: &[Line] = &[
    (
        "Duel Drake",
        1,
        &[Color::Blue],
        2,
        2,
        &[Keyword::Flying, Keyword::Flash],
        4,
    ),
    ("Duel Wizard", 1, &[Color::Blue], 1, 3, &[Keyword::Flash], 4),
    (
        "Duel Shade",
        1,
        &[Color::Black],
        2,
        1,
        &[Keyword::Deathtouch],
        4,
    ),
    (
        "Duel Horror",
        3,
        &[Color::Black],
        4,
        4,
        &[Keyword::Menace],
        4,
    ),
    (
        "Duel Sphinx",
        3,
        &[Color::Blue, Color::Blue],
        4,
        4,
        &[Keyword::Flying],
        4,
    ),
    (
        "Duel Lurker",
        3,
        &[Color::Black],
        3,
        3,
        &[Keyword::Flash],
        3,
    ),
];

const DIMIR_SPELLS: &[(&str, usize)] = &[
    ("Counterspell", 4),
    ("Mana Drain", 2),
    ("Memory Lapse", 2),
    ("Doom Blade", 4),
    ("Snuff Out", 2),
    ("Fact or Fiction", 2),
    ("Opt", 2),
    ("Mulldrifter", 2),
];

const AZORIUS: &[Line] = &[
    (
        "Duel Drake",
        1,
        &[Color::Blue],
        2,
        2,
        &[Keyword::Flying, Keyword::Flash],
        4,
    ),
    ("Duel Wizard", 1, &[Color::Blue], 1, 3, &[Keyword::Flash], 4),
    (
        "Duel Squire",
        1,
        &[Color::White],
        2,
        2,
        &[Keyword::FirstStrike],
        4,
    ),
    (
        "Duel Sphinx",
        3,
        &[Color::Blue, Color::Blue],
        4,
        4,
        &[Keyword::Flying],
        3,
    ),
    (
        "Duel Templar",
        2,
        &[Color::White],
        3,
        3,
        &[Keyword::Vigilance],
        3,
    ),
    (
        "Duel Archon",
        4,
        &[Color::White],
        4,
        5,
        &[Keyword::Flying, Keyword::Flash],
        2,
    ),
];

const AZORIUS_SPELLS: &[(&str, usize)] = &[
    ("Counterspell", 4),
    ("Hinder", 2),
    ("Essence Capture", 2),
    ("Swords to Plowshares", 3),
    ("Path to Exile", 3),
    ("Wrath of God", 2),
    ("Into the Roil", 2),
    ("Brainstorm", 2),
    ("Serra Angel", 3),
];

/// The 60 cards of `duel`, lands first.
pub fn duel_deck(duel: Duel) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let (colors, lines, spells) = match duel {
        Duel::Gruul => ([Color::Green, Color::Red], GRUUL, GRUUL_SPELLS),
        Duel::Orzhov => ([Color::White, Color::Black], ORZHOV, ORZHOV_SPELLS),
        Duel::Izzet => ([Color::Blue, Color::Red], IZZET, IZZET_SPELLS),
        Duel::Dimir => ([Color::Blue, Color::Black], DIMIR, DIMIR_SPELLS),
        Duel::Azorius => ([Color::Blue, Color::White], AZORIUS, AZORIUS_SPELLS),
    };
    let mut deck: Vec<Arc<CardDefinition>> = (0..17).map(|i| land(colors[i % 2])).collect();
    for (name, copies) in spells {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, *copies));
    }
    for (name, generic, colored, power, toughness, keywords, copies) in lines {
        let card = keyworded_creature(name, *generic, colored, *power, *toughness, keywords);
        deck.extend(std::iter::repeat_n(card, *copies));
    }
    Ok(deck)
}

/// A Normal game of `decks`, one per seat in table order, seeded with `seed`, under `house` rules (teams, say).
pub fn duel_setup(decks: &[Duel], seed: u64, house: HouseRules) -> Result<GameSetup, CorpusError> {
    let mut lists = Vec::with_capacity(decks.len());
    for duel in decks {
        lists.push(duel_deck(*duel)?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, lists).with_house_rules(house))
}

#[cfg(test)]
mod tests {
    //! Goal: every duel deck is legal 60-card Normal decks built from cards the corpus has.
    use mtg_engine::format::FormatRules;
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn each_duel_deck_is_a_legal_sixty(
        #[values(Duel::Gruul, Duel::Orzhov, Duel::Izzet, Duel::Dimir, Duel::Azorius)] duel: Duel,
    ) {
        let deck = duel_deck(duel).unwrap();
        assert_eq!(deck.len(), 60);
        assert!(Normal.validate_deck(&deck).is_ok());
    }
}
