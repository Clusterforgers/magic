//! Evasion (CR 509.1b) and damage that can't be prevented (CR 615.12), and the decks and games built from them: the corpus
//! cards with such abilities, creatures that can't be blocked by some creatures, and a spell that makes a creature
//! unblockable for a turn.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in an evasion deck: green twice, then white, blue and red in turn.
const LANDS: usize = 24;

/// The corpus cards of an evasion deck, with their copies.
pub const EVASION_CARDS: [(&str, usize); 7] = [
    ("Questing Beast", 3),
    ("Invisible Stalker", 3),
    ("Fog", 2),
    ("Mending Hands", 2),
    ("Ajani, Caller of the Pride", 2),
    ("Garruk Wildspeaker", 2),
    ("Jace Beleren", 1),
];

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A free creature with the static ability `mods` (a JSON list of modifications that apply to it) and these keywords.
pub fn evader(
    name: &str,
    power: i32,
    toughness: i32,
    mods: &str,
    keywords: &[Keyword],
) -> Result<Arc<CardDefinition>, CorpusError> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(power, toughness)
        .with_rules(rules(&format!(
            r#"{{"Static":{{"affected":"This","mods":{mods}}}}}"#
        ))?);
    Ok(Arc::new(keywords.iter().fold(card, |card, keyword| {
        card.with_rules(Ability::Keyword(keyword.clone()))
    })))
}

/// `{R}` instant: target creature can't be blocked this turn.
pub fn sneak(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(rules(spell)?),
    ))
}

/// A legal Normal deck of the evasion cards, creatures they meet in combat and burn, derived entirely from `rng`.
pub fn evasion_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Green,
        Color::Green,
        Color::White,
        Color::Blue,
        Color::Red,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in EVASION_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let small = r#"[{"CantBeBlockedBy":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}]"#;
    let fliers = r#"[{"CantBeBlockedBy":[{"HasKeyword":"Flying"}]}]"#;
    let ground = r#"[{"CantBeBlockedBy":[{"Not":{"AnyOf":[{"HasKeyword":"Flying"},{"HasKeyword":"Reach"}]}}]}]"#;
    deck.extend(std::iter::repeat_n(
        evader("Shade", 2, 2, small, &[Keyword::Menace])?,
        3,
    ));
    deck.extend(std::iter::repeat_n(
        evader("Skyhunter", 3, 3, fliers, &[])?,
        2,
    ));
    deck.extend(std::iter::repeat_n(
        evader("Wing", 2, 2, ground, &[Keyword::Flying])?,
        2,
    ));
    deck.extend(std::iter::repeat_n(sneak("Sneak")?, 3));
    deck.extend(std::iter::repeat_n(free_creature("Mite", 1, 1), 4));
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`evasion_deck`]s, fully determined by `seed`.
pub fn evasion_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(evasion_deck(&mut GameRng::substream(seed, 23_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the evasion spell and the creatures with a static restriction are valid rules the engine runs, and an evasion deck holds every evasion card and enough cards to play, fixed by its seed.
    use super::*;

    #[test]
    fn an_evader_has_the_static_ability_and_its_keywords() {
        let card = evader("Shade", 2, 2, r#"["CantBeBlocked"]"#, &[Keyword::Menace]).unwrap();
        assert_eq!(card.rules.len(), 2);
        assert!(matches!(card.rules[0], Ability::Static { .. }));
    }

    #[test]
    fn a_wrong_modification_is_a_data_error() {
        assert!(evader("Shade", 2, 2, r#"["Fly"]"#, &[]).is_err());
    }

    #[test]
    fn the_spell_costs_one_red_and_has_one_ability() {
        let card = sneak("Sneak").unwrap();
        assert_eq!(card.rules.len(), 1);
        assert_eq!(card.mana_cost.as_ref(), Some(&cost(0, &[Color::Red])));
    }

    #[test]
    fn a_deck_has_every_card_and_is_fixed_by_its_seed() {
        let first = evasion_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = evasion_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert!(first.len() >= 60, "{}", first.len());
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&first), names(&again));
        for (name, _) in EVASION_CARDS {
            assert!(names(&first).iter().any(|held| held == name), "{name}");
        }
    }
}
