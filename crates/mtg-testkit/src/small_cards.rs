//! Synthetic cards for the small effects (a condition that picks what happens, tapping, untapping, milling, winning
//! the game) and for flash, and the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, TargetSpec, Targetable};
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, draw, gain_life, keyworded_creature, land, one_target};
use crate::trigger_cards::{artifact_costing, only_if, trigger, when};

/// Lands in a deck of small effects.
const LANDS: usize = 24;

fn a_creature() -> TargetSpec {
    one_target(Targetable::Object(Selector(vec![Pred::Type(
        CardType::Creature,
    )])))
}

/// The creatures `who` controls, as a reference.
fn creatures_of(who: Ref) -> Ref {
    Ref::Each(Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(who),
    ]))
}

fn spell(
    name: &str,
    card_type: CardType,
    generic: u32,
    colors: &[Color],
    targets: Vec<TargetSpec>,
    effects: Vec<Effect>,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(card_type)
            .with_mana_cost(cost(generic, colors))
            .with_rules(Ability::Spell { targets, effects }),
    )
}

/// An instant for `{U}`: tap target creature.
pub fn frost(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        0,
        &[Color::Blue],
        vec![a_creature()],
        vec![Effect::Tap {
            what: Ref::Target(0),
        }],
    )
}

/// An instant for `{G}`: untap target permanent.
pub fn thaw(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        0,
        &[Color::Green],
        vec![one_target(Targetable::Object(Selector::default()))],
        vec![Effect::Untap {
            what: Ref::Target(0),
        }],
    )
}

/// An instant for `{2}{U}`: tap every creature your opponents control.
pub fn blizzard(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        2,
        &[Color::Blue],
        Vec::new(),
        vec![Effect::Tap {
            what: creatures_of(Ref::Opponents),
        }],
    )
}

/// An instant for `{1}{G}`: untap every creature you control.
pub fn reveille(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        1,
        &[Color::Green],
        Vec::new(),
        vec![Effect::Untap {
            what: creatures_of(Ref::You),
        }],
    )
}

/// A sorcery for `{1}{U}`: target player mills three cards.
pub fn grind(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Sorcery,
        1,
        &[Color::Blue],
        vec![one_target(Targetable::Player)],
        vec![Effect::Mill {
            who: Ref::Target(0),
            count: Amount::Fixed(3),
        }],
    )
}

/// A sorcery for `{2}{B}`: each player mills two cards.
pub fn rot(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Sorcery,
        2,
        &[Color::Black],
        Vec::new(),
        vec![Effect::Mill {
            who: Ref::EachPlayer,
            count: Amount::Fixed(2),
        }],
    )
}

/// An instant for `{2}{G}`: draw a card; if you have 25 life or more, draw another, otherwise gain 2 life.
pub fn gambit(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        2,
        &[Color::Green],
        Vec::new(),
        vec![
            draw(1),
            Effect::If {
                condition: Condition::Compare {
                    left: Amount::Life(Ref::You),
                    op: Cmp::Ge,
                    right: Amount::Fixed(25),
                },
                then: vec![draw(1)],
                otherwise: vec![gain_life(2)],
            },
        ],
    )
}

/// An instant for `{1}{U}`: if it is your turn, untap every creature you control; otherwise tap every creature your
/// opponents control.
pub fn mirror(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        1,
        &[Color::Blue],
        Vec::new(),
        vec![Effect::If {
            condition: Condition::YourTurn,
            then: vec![Effect::Untap {
                what: creatures_of(Ref::You),
            }],
            otherwise: vec![Effect::Tap {
                what: creatures_of(Ref::Opponents),
            }],
        }],
    )
}

/// A sorcery for `{1}{U}`: mill two cards; if you control two or more creatures, draw a card.
pub fn study(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Sorcery,
        1,
        &[Color::Blue],
        Vec::new(),
        vec![
            Effect::Mill {
                who: Ref::You,
                count: Amount::Fixed(2),
            },
            Effect::If {
                condition: Condition::Compare {
                    left: Amount::Count(Selector(vec![
                        Pred::Type(CardType::Creature),
                        Pred::ControlledBy(Ref::You),
                    ])),
                    op: Cmp::Ge,
                    right: Amount::Fixed(2),
                },
                then: vec![draw(1)],
                otherwise: Vec::new(),
            },
        ],
    )
}

/// An artifact for `{2}`: at the beginning of your upkeep, if you control five or more creatures, you win the game.
pub fn laurel(name: &str) -> Arc<CardDefinition> {
    let rules = only_if(
        trigger(when::your_upkeep(), vec![Effect::WinGame { who: Ref::You }]),
        Condition::Compare {
            left: Amount::Count(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ])),
            op: Cmp::Ge,
            right: Amount::Fixed(5),
        },
    );
    artifact_costing(name, 2, vec![rules])
}

/// A 2/2 with flash for `{2}{G}`.
pub fn ambusher(name: &str) -> Arc<CardDefinition> {
    keyworded_creature(name, 2, &[Color::Green], 2, 2, &[Keyword::Flash])
}

/// A legal Normal deck in blue, green, white and black whose spells tap and untap, mill, choose by a condition and
/// win the game: three Frost, two each of Thaw, Blizzard, Reveille, Grind, Rot, Mirror, Study and Laurel, three
/// Gambit, four ambushers with flash, creatures and lands.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn small_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Green, Color::White, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 4])).collect();
    for (card, copies) in [
        (frost("Small Frost"), 3),
        (thaw("Small Thaw"), 2),
        (blizzard("Small Blizzard"), 2),
        (reveille("Small Reveille"), 2),
        (grind("Small Grind"), 2),
        (rot("Small Rot"), 2),
        (gambit("Small Gambit"), 3),
        (mirror("Small Mirror"), 2),
        (study("Small Study"), 2),
        (laurel("Small Laurel"), 2),
        (ambusher("Small Ambusher"), 4),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    for slot in 0..5 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Small Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(4)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck
}

/// A normal game of `players` seats whose decks are [`small_deck`]s, fully determined by `seed`.
pub fn small_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| small_deck(&mut GameRng::substream(seed, 13_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic cards of the small effects use only what the engine runs, so every one is a card the game offers, and a deck of them is a legal sixty cards.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn every_synthetic_card_uses_primitives_the_engine_runs() {
        let caps = caps();
        let cards = [
            frost("Frost"),
            thaw("Thaw"),
            blizzard("Blizzard"),
            reveille("Reveille"),
            grind("Grind"),
            rot("Rot"),
            gambit("Gambit"),
            mirror("Mirror"),
            study("Study"),
            laurel("Laurel"),
            ambusher("Ambusher"),
        ];
        for card in cards {
            assert!(!card.rules.is_empty(), "{}", card.name);
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn a_small_deck_has_sixty_cards_and_is_the_same_every_time() {
        let deck = small_deck(&mut GameRng::substream(1, 1));
        assert_eq!(deck.len(), 60);
        assert_eq!(deck, small_deck(&mut GameRng::substream(1, 1)));
    }
}
