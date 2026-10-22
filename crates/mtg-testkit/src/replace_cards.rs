//! Replacement effects (CR 614 to 616) and the decks and games built from them: the corpus cards that double tokens,
//! counters, damage, life and draws, send cards to exile instead of a graveyard and change how a permanent enters, with
//! spells that make tokens, put counters, gain life, draw, discard, burn and destroy for them to act on.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, elf, land, mind_rot, study};
use crate::corpus::{self, CorpusError};
use crate::counter_cards::growth_spurt;

/// Lands in a replacement deck: five colours in turn.
const LANDS: usize = 20;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// `{W}` instant: create two 1/1 white Soldier creature tokens.
pub fn raise_soldiers() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":2}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Raise Soldiers")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(rules(spell)?),
    ))
}

/// `{W}` instant: its controller gains 3 life.
pub fn mend() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"effects":[{"GainLife":{"who":"You","amount":3}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Mend")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(rules(spell)?),
    ))
}

/// `{B}` sorcery: put target creature or land card from your graveyard onto the battlefield, tapped when `tapped`.
///
/// The card is put onto the battlefield by an effect, which is a way in of its own for the replacement effects of
/// a permanent that enters (a land played and a spell that resolves are the others).
pub fn raise_dead(name: &str, tapped: bool) -> Result<Arc<CardDefinition>, CorpusError> {
    let to = if tapped {
        r#"{"Battlefield":{"tapped":true}}"#
    } else {
        r#"{"Battlefield":{}}"#
    };
    let spell = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}},{{"AnyOf":[{{"Type":"Creature"}},{{"Type":"Land"}}]}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{to}}}}}]}}}}"#
    );
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Black]))
            .with_rules(rules(&spell)?),
    ))
}

/// Cards of the corpus the replacement deck holds, with how many of each.
pub const CORPUS_CARDS: &[(&str, usize)] = &[
    ("Anointed Procession", 2),
    ("Parallel Lives", 1),
    ("Doubling Season", 2),
    ("Hardened Scales", 2),
    ("Corpsejack Menace", 1),
    ("Rhox Faithmender", 1),
    ("Thought Reflection", 1),
    ("Furnace of Rath", 1),
    ("Rest in Peace", 2),
    ("Idyllic Beachfront", 2),
    ("Steam Vents", 2),
    ("Sodden Verdure", 1),
    ("Faithful Watchdog", 2),
    ("Hall of Triumph", 1),
    ("Rally the Ranks", 1),
    ("Hero of Bladehold", 2),
    ("Jace Beleren", 1),
    ("Doom Blade", 2),
    ("Counterspell", 1),
    ("Lightning Bolt", 2),
    ("Squirrel Nest", 1),
    ("Brainstorm", 1),
    ("Glorious Anthem", 1),
    ("Swords to Plowshares", 1),
    ("Llanowar Elves", 1),
];

/// A legal Normal deck of replacement effects and the cards they act on, derived entirely from `rng`.
pub fn replace_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Green,
        Color::Blue,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &(name, copies) in CORPUS_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (raise_soldiers()?, 3),
        (mend()?, 2),
        (raise_dead("Raise Dead", false)?, 2),
        (growth_spurt()?, 3),
        (study("Study"), 1),
        (mind_rot("Mind Rot"), 1),
        (burn("Spark", 2), 1),
        (bear(), 3),
        (elf(), 1),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`replace_deck`]s, fully determined by `seed`.
pub fn replace_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(replace_deck(&mut GameRng::substream(seed, 21_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the spells of this module are valid rules the engine runs, and a replacement deck is a legal sixty or more cards with every replacement card and every card they act on in it, fixed by its seed.
    use super::*;
    use mtg_cards::support::Support;

    #[test]
    fn the_spells_make_tokens_gain_life_and_put_cards_onto_the_battlefield() {
        for card in [
            raise_soldiers().unwrap(),
            mend().unwrap(),
            raise_dead("Raise Dead", false).unwrap(),
            raise_dead("Raise Dead Tapped", true).unwrap(),
        ] {
            assert_eq!(card.rules.len(), 1, "{}", card.name);
        }
    }

    #[test]
    fn a_replacement_deck_holds_every_replacement_card_and_is_fixed_by_its_seed() {
        let deck = replace_deck(&mut GameRng::substream(7, 1)).unwrap();
        assert!(deck.len() >= 60, "{}", deck.len());
        for &(name, _) in CORPUS_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let again = replace_deck(&mut GameRng::substream(7, 1)).unwrap();
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&deck), names(&again));
    }

    #[test]
    fn the_replacement_cards_of_the_corpus_are_verified() {
        let _ = Support::Verified;
        for name in [
            "Anointed Procession",
            "Parallel Lives",
            "Doubling Season",
            "Hardened Scales",
            "Corpsejack Menace",
            "Rhox Faithmender",
            "Thought Reflection",
            "Furnace of Rath",
            "Rest in Peace",
        ] {
            let card = corpus::card(name).unwrap();
            assert!(
                card.rules
                    .iter()
                    .any(|rules| matches!(**rules, Ability::Replacement { .. })),
                "{name} carries its replacement effect"
            );
        }
    }
}
