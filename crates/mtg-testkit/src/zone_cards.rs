//! Abilities that work from a zone other than the battlefield (CR 113.6) and state triggers (CR 603.8), and the decks and
//! games built from them: the corpus cards that return from a graveyard, are exiled or discarded as a cost, apply from a
//! graveyard or wait for a state, and creatures that trigger from a hand, a graveyard or the stack or wait for a state,
//! with removal, discard and exile for them to bear on.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land, mind_rot};
use crate::corpus::{self, CorpusError};

/// Lands in a zones deck: five colours in turn.
const LANDS: usize = 22;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A creature with this ability, written as data.
fn creature_with(
    name: &str,
    generic: u32,
    colored: &[Color],
    (power, toughness): (i32, i32),
    subtype: &str,
    ability: &str,
) -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_mana_cost(cost(generic, colored))
            .with_power_toughness(power, toughness)
            .with_rules(rules(ability)?),
    ))
}

const UPKEEP: &str = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;

/// `{2}{B}` 3/3 creature: at the beginning of your upkeep, if it is in your graveyard, you gain life equal to its
/// power, which it keeps as it was when it has left by the time the ability resolves.
pub fn echo_wraith() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = format!(
        r#"{{"Triggered":{{"when":{UPKEEP},"from":"Graveyard","effects":[{{"GainLife":{{"amount":{{"Stat":{{"of":"This","stat":"Power"}}}}}}}}]}}}}"#
    );
    creature_with("Echo Wraith", 2, &[Color::Black], (3, 3), "Spirit", &ability)
}

/// `{1}{G}` 1/1 creature: at the beginning of your upkeep, if it is in your hand, you gain 1 life.
pub fn wary_hound() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = format!(
        r#"{{"Triggered":{{"when":{UPKEEP},"from":"Hand","effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    creature_with("Wary Hound", 1, &[Color::Green], (1, 1), "Dog", &ability)
}

/// `{2}{R}` 2/2 creature: when you cast it, you gain 2 life, which happens while it is on the stack.
pub fn thundering_cast() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"SpellCast":{"what":["This"],"by":"You"}},"from":"Stack","effects":[{"GainLife":{"amount":2}}]}}"#;
    creature_with(
        "Thundering Cast",
        2,
        &[Color::Red],
        (2, 2),
        "Giant",
        ability,
    )
}

/// `{1}{U}` 1/3 creature: whenever you have two or fewer cards in hand, you draw a card.
pub fn hollow_scholar() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Le","right":2}}}},"effects":[{"Draw":{}}]}}"#;
    creature_with(
        "Hollow Scholar",
        1,
        &[Color::Blue],
        (1, 3),
        "Wizard",
        ability,
    )
}

/// `{1}{G}` 3/3 creature: when you control no other creatures, sacrifice it.
pub fn lone_wolf() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}}}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#;
    creature_with("Lone Wolf", 1, &[Color::Green], (3, 3), "Wolf", ability)
}

/// `{W}` 1/1 creature: whenever you discard a card, you gain 1 life.
pub fn discard_watcher() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"ZoneChange":{"what":[],"from":"Hand","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
    creature_with(
        "Discard Watcher",
        0,
        &[Color::White],
        (1, 1),
        "Spirit",
        ability,
    )
}

/// `{R}` 2/2 Dragon with no abilities, for the cards that wait for a Dragon to die.
pub fn dragon_whelp() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dragon Whelp")
            .with_type(CardType::Creature)
            .with_subtype("Dragon")
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_power_toughness(2, 2),
    )
}

/// `{0}` instant: exile target card in a graveyard.
pub fn banish() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Banish")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(spell)?),
    ))
}

/// The corpus cards that work from a zone other than the battlefield or wait for a state.
pub const ZONE_CORPUS_CARDS: [&str; 7] = [
    "Reassembling Skeleton",
    "Soul of Zendikar",
    "Boneyard Scourge",
    "Squee, Goblin Nabob",
    "Anger",
    "Covetous Dragon",
    "Elemental Masterpiece",
];

/// The creatures of this module that work from a zone other than the battlefield or wait for a state.
pub const ZONE_SYNTHETIC_CARDS: [&str; 6] = [
    "Echo Wraith",
    "Wary Hound",
    "Thundering Cast",
    "Hollow Scholar",
    "Lone Wolf",
    "Discard Watcher",
];

/// A legal Normal deck in five colours of cards that work from other zones: the corpus cards and creatures above,
/// Dragons for the Scourge to wait for, Lightning Bolt and Doom Blade to put creatures in graveyards, Mind Rot to
/// discard, Banish to exile and Sol Ring for an artifact.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn zones_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in [
        ("Reassembling Skeleton", 3),
        ("Soul of Zendikar", 2),
        ("Boneyard Scourge", 2),
        ("Squee, Goblin Nabob", 2),
        ("Anger", 2),
        ("Covetous Dragon", 2),
        ("Elemental Masterpiece", 2),
        ("Sol Ring", 1),
        ("Lightning Bolt", 3),
        ("Doom Blade", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (echo_wraith()?, 2),
        (wary_hound()?, 2),
        (thundering_cast()?, 2),
        (hollow_scholar()?, 2),
        (lone_wolf()?, 2),
        (discard_watcher()?, 1),
        (dragon_whelp(), 2),
        (banish()?, 2),
        (mind_rot("Mind Rot"), 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`zones_deck`]s, fully determined by `seed`.
pub fn zones_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(zones_deck(&mut GameRng::substream(seed, 15_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic cards of the zones deck use only what the engine runs, a zones deck is a legal sixty cards with every card that works from another zone or waits for a state in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    fn synthetic() -> Vec<Arc<CardDefinition>> {
        vec![
            echo_wraith().unwrap(),
            wary_hound().unwrap(),
            thundering_cast().unwrap(),
            hollow_scholar().unwrap(),
            lone_wolf().unwrap(),
            discard_watcher().unwrap(),
            banish().unwrap(),
        ]
    }

    #[test]
    fn the_synthetic_cards_use_primitives_the_engine_runs() {
        let caps = caps();
        for card in synthetic() {
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
        assert_eq!(
            synthetic()
                .iter()
                .map(|card| card.name.as_str())
                .take(ZONE_SYNTHETIC_CARDS.len())
                .collect::<Vec<_>>(),
            ZONE_SYNTHETIC_CARDS
        );
    }

    #[test]
    fn a_zones_deck_is_sixty_cards_with_every_card_that_works_from_another_zone() {
        let deck = zones_deck(&mut GameRng::substream(1, 15_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in ZONE_CORPUS_CARDS.iter().chain(&ZONE_SYNTHETIC_CARDS) {
            assert!(deck.iter().any(|card| card.name == *name), "{name}");
        }
        let setup = zones_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = zones_fuzz_setup(3, 5).unwrap();
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&again));
    }
}
