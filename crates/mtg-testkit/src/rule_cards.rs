//! Rules of the game (CR 508.1d, 509.1c, 614.5, 701.6) and the decks and games built from them: the corpus cards that
//! require an attack or a block, can't be countered, stop life, drawing or searching, reveal a library top or trigger
//! again, with removal, counterspells, draws, searches and life gain for them to bear on, and creatures that gain life
//! as they enter.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land, salve};
use crate::corpus::{self, CorpusError};

/// Lands in a rules deck: five colours in turn.
const LANDS: usize = 22;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// `{1}{G}` 1/1 creature: when it enters, you gain 1 life, which a rule that triggers again repeats.
pub fn chime_elf() -> Result<Arc<CardDefinition>, CorpusError> {
    let trigger = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Chime Elf")
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_power_toughness(1, 1)
            .with_rules(rules(trigger)?),
    ))
}

/// `{2}{W}` 2/2 creature with lifelink and "whenever a creature dies, you gain 1 life": gains that a rule can stop.
pub fn lifebond_hound() -> Result<Arc<CardDefinition>, CorpusError> {
    let trigger = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Lifebond Hound")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(2, &[Color::White]))
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(mtg_cards::keyword::Keyword::Lifelink))
            .with_rules(rules(trigger)?),
    ))
}

/// `{1}{W}` 1/3 creature that blocks each combat if able.
pub fn grudge_wall() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new("Grudge Wall")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::White]))
            .with_power_toughness(1, 3)
            .with_rules(rules(r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#)?),
    ))
}

/// The names of the cards that change a rule of the game, which a game uses when they enter the battlefield or are
/// cast.
pub const RULE_CARDS: &[&str] = &[
    "Darksteel Juggernaut",
    "Warmonger Hellkite",
    "Courtly Provocateur",
    "Supreme Verdict",
    "Carnage Tyrant",
    "Spellbreaker Behemoth",
    "Rampaging Ferocidon",
    "Maralen of the Mornsong",
    "Mindlock Orb",
    "Goblin Spy",
    "Teysa Karlov",
    "Naban, Dean of Iteration",
    "Panharmonicon",
];

/// A legal Normal deck in five colours of rules: the corpus cards above, Counterspell and Lightning Bolt to counter and
/// kill, Rampant Growth, Cultivate, Brainstorm and Opt to search and draw, Salve to gain life and draw, and two
/// creatures that gain life.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn rules_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in [
        ("Darksteel Juggernaut", 2),
        ("Warmonger Hellkite", 2),
        ("Courtly Provocateur", 2),
        ("Supreme Verdict", 2),
        ("Carnage Tyrant", 2),
        ("Spellbreaker Behemoth", 2),
        ("Rampaging Ferocidon", 2),
        ("Maralen of the Mornsong", 2),
        ("Mindlock Orb", 2),
        ("Goblin Spy", 2),
        ("Teysa Karlov", 2),
        ("Naban, Dean of Iteration", 2),
        ("Panharmonicon", 2),
        ("Counterspell", 2),
        ("Lightning Bolt", 3),
        ("Rampant Growth", 1),
        ("Brainstorm", 1),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(salve("Salve"), 1));
    deck.extend(std::iter::repeat_n(chime_elf()?, 1));
    deck.extend(std::iter::repeat_n(lifebond_hound()?, 1));
    deck.extend(std::iter::repeat_n(grudge_wall()?, 2));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`rules_deck`]s, fully determined by `seed`.
pub fn rules_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(rules_deck(&mut GameRng::substream(seed, 14_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic cards of the rules deck use only what the engine runs, a rules deck is a legal sixty cards with every card that changes a rule in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn the_synthetic_cards_use_primitives_the_engine_runs() {
        let caps = caps();
        for card in [
            chime_elf().unwrap(),
            lifebond_hound().unwrap(),
            grudge_wall().unwrap(),
        ] {
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn a_rules_deck_is_sixty_cards_with_every_card_that_changes_a_rule() {
        let deck = rules_deck(&mut GameRng::substream(1, 14_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in RULE_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = rules_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = rules_fuzz_setup(3, 5).unwrap();
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
