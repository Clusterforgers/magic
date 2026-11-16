//! Which relics a deck can use. Each relic names what its rules care about (`CustomCard::wants`: spells of a colour,
//! cards of a type or subtype, token makers, counters, the graveyard, a creature commander); the deck's makeup, counted
//! from its commanders and library when the deck is chosen and from the Satchel as it grows, says whether the hero
//! holds enough of them. A relic offer then proposes only relics the deck can use.

use std::collections::BTreeMap;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::content::{CustomCard, Theme};

/// The content generation from which relic offers follow the deck and the larger relic pool is offered
/// (`RunEvent::Content`); older runs draw from the first twenty relics and every deck alike.
pub const FILTER_FROM: u32 = 2;
/// The cards of what a relic wants a deck holds at least for the relic to be of use, the commanders and the Satchel
/// included; a creature commander is wanted once.
pub const MIN_CARDS: u32 = 8;

/// What a relic's rules care about in a deck; a relic with none is of use to any deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Want {
    /// Spells of a colour, as a WUBRG letter.
    Color(char),
    /// Cards of a type.
    Type(CardType),
    /// Cards of a subtype ("Rat", "Equipment", "Forest").
    Subtype(String),
    /// Cards that create tokens.
    Tokens,
    /// Cards that put or use +1/+1 counters.
    Counters,
    /// Cards that read or use the graveyard (flashback, reanimation, milling, counting what lies in it).
    Graveyard,
    /// A commander that is a creature.
    CreatureCommander,
}

/// What a deck is made of, counted when it is chosen: its commanders and library (the Satchel is counted from the
/// theme's pool as it grows). A card counts once for each colour, type and subtype it has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckMakeup {
    /// Cards of each colour, by WUBRG letter; colourless cards count for none.
    pub colors: BTreeMap<String, u32>,
    /// Cards of each card type, by name ("Creature", "Land").
    pub types: BTreeMap<String, u32>,
    /// Cards of each subtype, by name ("Rat", "Equipment").
    pub subtypes: BTreeMap<String, u32>,
    /// Cards that create tokens.
    #[serde(default)]
    pub tokens: u32,
    /// Cards that put or use +1/+1 counters.
    #[serde(default)]
    pub counters: u32,
    /// Cards that read or use the graveyard.
    #[serde(default)]
    pub graveyard: u32,
    /// Commanders that are creatures.
    #[serde(default)]
    pub creature_commanders: u32,
}

impl DeckMakeup {
    /// Counts one card of the deck; `commander` says it leads the deck.
    pub fn add_card(&mut self, card: &CardDefinition, commander: bool) {
        let colors: String = card.colors().iter().map(color_letter).collect();
        self.add(
            &colors,
            card.types.iter().copied(),
            card.subtypes.iter().map(|subtype| subtype.0.as_str()),
        );
        if commander && card.types.contains(&CardType::Creature) {
            self.creature_commanders += 1;
        }
        // What the rules do, read from the card's abilities as the engine runs them.
        let rules = serde_json::to_string(&card.rules).unwrap_or_default();
        self.tokens += u32::from(TOKENS.iter().any(|word| rules.contains(word)));
        self.counters += u32::from(rules.contains("\"+1/+1\""));
        self.graveyard += u32::from(GRAVEYARD.iter().any(|word| rules.contains(word)));
    }

    /// Counts one Satchel card from its colours and its type line ("Legendary Creature — Elf Druid").
    pub fn add_type_line(&mut self, colors: &str, type_line: &str) {
        let (front, back) = type_line.split_once('—').unwrap_or((type_line, ""));
        let types = front.split_whitespace().filter_map(card_type);
        self.add(colors, types, back.split_whitespace());
    }

    fn add<S: AsRef<str>>(
        &mut self,
        colors: &str,
        types: impl IntoIterator<Item = CardType>,
        subtypes: impl IntoIterator<Item = S>,
    ) {
        for color in colors.chars() {
            *self.colors.entry(color.to_string()).or_default() += 1;
        }
        for card_type in types {
            *self
                .types
                .entry(type_key(card_type).to_owned())
                .or_default() += 1;
        }
        for subtype in subtypes {
            *self
                .subtypes
                .entry(subtype.as_ref().to_owned())
                .or_default() += 1;
        }
    }

    /// How many cards the deck holds of what `want` names.
    pub fn count(&self, want: &Want) -> u32 {
        let found = |counted: Option<&u32>| counted.copied().unwrap_or(0);
        match want {
            Want::Color(color) => found(self.colors.get(&color.to_string())),
            Want::Type(card_type) => found(self.types.get(type_key(*card_type))),
            Want::Subtype(subtype) => found(self.subtypes.get(subtype)),
            Want::Tokens => self.tokens,
            Want::Counters => self.counters,
            Want::Graveyard => self.graveyard,
            Want::CreatureCommander => self.creature_commanders,
        }
    }

    /// The makeup with the Satchel's cards counted: each from the theme's pool (colours and type line); a card the pool
    /// lacks, a Mirror Pool copy of a card of the deck, is not counted again.
    #[must_use]
    pub fn with_satchel(&self, satchel: &[String], theme: &Theme) -> DeckMakeup {
        let mut out = self.clone();
        for name in satchel {
            let Some((_, colors)) = theme.pool.iter().find(|(card, _)| card == name) else {
                continue;
            };
            let line = theme.pool_types.get(name).map_or("", String::as_str);
            out.add_type_line(colors, line);
        }
        out
    }
}

/// Whether a deck of this makeup can use `relic`: it wants nothing (life, cards, mana and the like), or the deck holds
/// a creature commander it wants, or holds [`MIN_CARDS`] cards of what it wants together (an instant and a sorcery
/// count alike for a relic about both).
pub fn usable(relic: &CustomCard, makeup: &DeckMakeup) -> bool {
    if relic.wants.is_empty() {
        return true;
    }
    let (commander, counted): (Vec<&Want>, Vec<&Want>) = relic
        .wants
        .iter()
        .partition(|want| matches!(want, Want::CreatureCommander));
    commander.iter().any(|want| makeup.count(want) >= 1)
        || counted.iter().map(|want| makeup.count(want)).sum::<u32>() >= MIN_CARDS
}

/// The relics of `fits` the deck can use; every one of `fits` when it can use none, so an offer is never empty while a
/// relic is left to offer.
pub fn offerable<'a>(fits: Vec<&'a CustomCard>, makeup: &DeckMakeup) -> Vec<&'a CustomCard> {
    let used: Vec<&CustomCard> = fits
        .iter()
        .copied()
        .filter(|relic| usable(relic, makeup))
        .collect();
    if used.is_empty() { fits } else { used }
}

/// The marks of a card that makes tokens in the rules as the engine reads them.
const TOKENS: [&str; 2] = ["\"CreateToken\"", "\"Amass\""];
/// The marks of a card that reads or uses the graveyard.
const GRAVEYARD: [&str; 8] = [
    "\"In\":\"Graveyard\"",
    "\"from\":\"Graveyard\"",
    "\"Mill\"",
    "\"Flashback\"",
    "\"Escape\"",
    "\"Unearth\"",
    "\"Dredge\"",
    "\"Delve\"",
];

/// The letter a colour goes by.
fn color_letter(color: Color) -> char {
    match color {
        Color::White => 'W',
        Color::Blue => 'U',
        Color::Black => 'B',
        Color::Red => 'R',
        Color::Green => 'G',
    }
}

/// The name a card type counts under.
fn type_key(card_type: CardType) -> &'static str {
    match card_type {
        CardType::Artifact => "Artifact",
        CardType::Battle => "Battle",
        CardType::Creature => "Creature",
        CardType::Enchantment => "Enchantment",
        CardType::Instant => "Instant",
        CardType::Kindred => "Kindred",
        CardType::Land => "Land",
        CardType::Planeswalker => "Planeswalker",
        CardType::Sorcery => "Sorcery",
    }
}

/// The card type a word of a type line names.
fn card_type(word: &str) -> Option<CardType> {
    Some(match word {
        "Artifact" => CardType::Artifact,
        "Battle" => CardType::Battle,
        "Creature" => CardType::Creature,
        "Enchantment" => CardType::Enchantment,
        "Instant" => CardType::Instant,
        "Kindred" | "Tribal" => CardType::Kindred,
        "Land" => CardType::Land,
        "Planeswalker" => CardType::Planeswalker,
        "Sorcery" => CardType::Sorcery,
        _ => return None,
    })
}
