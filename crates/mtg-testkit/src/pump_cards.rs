//! Synthetic spells that change creatures for a while (CR 611), and the decks and games built from them: grants of a
//! keyword, a shrink, a pump of every creature one controls with two changes at once, and a bounce, beside the corpus
//! Giant Growth.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, TargetSpec, Targetable};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, keyworded_creature, land, one_target};
use crate::corpus::{self, CorpusError};

/// Lands in a pump deck.
const LANDS: usize = 22;

fn a_creature() -> TargetSpec {
    one_target(Targetable::Object(Selector(vec![Pred::Type(
        CardType::Creature,
    )])))
}

fn instant(
    name: &str,
    generic: u32,
    colors: &[Color],
    targets: Vec<TargetSpec>,
    effects: Vec<Effect>,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(generic, colors))
            .with_rules(Ability::Spell { targets, effects }),
    )
}

fn until_end_of_turn(affected: Ref, mods: Vec<Modification>) -> Effect {
    Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    }
}

/// An instant for `{R}`: target creature gains `keyword` until end of turn.
pub fn grant(name: &str, keyword: Keyword) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Red],
        vec![a_creature()],
        vec![until_end_of_turn(
            Ref::Target(0),
            vec![Modification::AddKeyword(keyword)],
        )],
    )
}

/// Rush, `{R}`: target creature gains haste until end of turn.
pub fn rush() -> Arc<CardDefinition> {
    grant("Rush", Keyword::Haste)
}

/// Gust, `{R}`: target creature gains flying until end of turn.
pub fn gust() -> Arc<CardDefinition> {
    grant("Gust", Keyword::Flying)
}

/// An instant for `{R}`: target creature gets -`by`/-`by` until end of turn.
pub fn shrink(name: &str, by: i32) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Red],
        vec![a_creature()],
        vec![until_end_of_turn(
            Ref::Target(0),
            vec![Modification::ModifyPT {
                power: Amount::Fixed(-by),
                toughness: Amount::Fixed(-by),
            }],
        )],
    )
}

/// An instant for `{1}{G}`: creatures you control get +1/+1 and gain trample until end of turn, two changes of one
/// set of creatures that is fixed when the spell resolves.
pub fn surge(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        1,
        &[Color::Green],
        Vec::new(),
        vec![until_end_of_turn(
            Ref::Each(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ])),
            vec![
                Modification::ModifyPT {
                    power: Amount::Fixed(1),
                    toughness: Amount::Fixed(1),
                },
                Modification::AddKeyword(Keyword::Trample),
            ],
        )],
    )
}

/// An instant for `{U}`: return target creature to its owner's hand.
pub fn unsummon(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Blue],
        vec![a_creature()],
        vec![Effect::Move {
            what: Ref::Target(0),
            to: Destination::Hand,
        }],
    )
}

/// A legal Normal deck in red, green and blue whose spells change creatures for a while: four Giant Growth, Rush, Gust
/// and Lightning Bolt each, three shrinks, two surges and three bounces, creatures with and without defender, and lands.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn pump_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Green, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [("Giant Growth", 4), ("Lightning Bolt", 3)] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (rush(), 3),
        (gust(), 3),
        (shrink("Pump Shrivel", 2), 3),
        (surge("Pump Surge"), 2),
        (unsummon("Pump Unsummon"), 3),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    let wall = keyworded_creature("Pump Wall", 1, &[Color::Green], 0, 4, &[Keyword::Defender]);
    deck.extend(std::iter::repeat_n(wall, 2));
    for slot in 0..5 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Pump Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`pump_deck`]s, fully determined by `seed`.
pub fn pump_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(pump_deck(&mut GameRng::substream(seed, 11_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic pump spells use only what the engine runs, so every one is a spell the game offers, and a pump deck is a legal sixty cards.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn every_synthetic_spell_uses_primitives_the_engine_runs() {
        let caps = caps();
        for card in [
            rush(),
            gust(),
            shrink("Shrivel", 2),
            surge("Surge"),
            unsummon("Unsummon"),
        ] {
            for ability in &card.rules {
                for name in used_primitives_of(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    fn used_primitives_of(ability: &Ability) -> Vec<String> {
        used_primitives(ability).into_iter().collect()
    }

    #[test]
    fn a_pump_deck_has_sixty_cards() {
        let deck = pump_deck(&mut GameRng::substream(1, 1)).unwrap();
        assert_eq!(deck.len(), 60);
    }
}
