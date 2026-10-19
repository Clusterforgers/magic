//! Seeded random 60-card decks of basic lands and vanilla creatures for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{
    burn, cost, creature, creature_costing, damage, drain, draw, gain_life, land, library_of_cards,
    pinger, salve, wastes,
};
use crate::trigger_cards::{
    aimed_at, artifact_costing, hourglass, once_each_turn, only_if, trigger, vigil, when,
    with_rules,
};

/// Lands in a generated deck.
const LANDS: usize = 24;
/// Distinct creatures in a generated deck.
const DISTINCT_CREATURES: usize = 12;
/// Copies of each creature.
const COPIES: usize = 3;

/// A legal Normal deck in two colors, derived entirely from `rng`.
pub fn random_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let first = Color::ALL[rng.index(Color::ALL.len())];
    let second =
        Color::ALL[(first.index() + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()];
    let colors = [first, second];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 2])).collect();
    for slot in 0..DISTINCT_CREATURES {
        let generic = u32::try_from(rng.index(4)).unwrap_or(0);
        let pips = 1 + rng.index(2);
        let cost_colors: Vec<Color> = (0..pips).map(|_| colors[rng.index(2)]).collect();
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Fuzz {slot} {generic}+{pips} {power}/{toughness}");
        let card = creature(&name, generic, &cost_colors, power, toughness);
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    deck
}

/// A normal game of `players` seats with random decks, fully determined by `seed`.
pub fn fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| random_deck(&mut GameRng::substream(seed, 1_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

/// A random cost symbol of any kind, mostly in the deck's two colours so that some of it can be paid in mana.
fn random_symbol(rng: &mut GameRng, colors: [Color; 2]) -> ManaSymbol {
    let own = |rng: &mut GameRng| colors[rng.index(2)];
    let any = |rng: &mut GameRng| Color::ALL[rng.index(Color::ALL.len())];
    let other = |rng: &mut GameRng, first: Color| {
        Color::ALL[(first.index() + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()]
    };
    match rng.index(9) {
        0 => ManaSymbol::Colored(own(rng)),
        1 => ManaSymbol::Hybrid(colors[0], colors[1]),
        2 => ManaSymbol::MonoHybrid(own(rng)),
        3 => ManaSymbol::Phyrexian(any(rng)),
        4 => {
            let first = any(rng);
            ManaSymbol::HybridPhyrexian(first, other(rng, first))
        }
        5 => ManaSymbol::ColorlessHybrid(own(rng)),
        6 => ManaSymbol::ColorlessPhyrexian,
        7 => ManaSymbol::Colored(any(rng)),
        _ => {
            let first = any(rng);
            ManaSymbol::Hybrid(first, other(rng, first))
        }
    }
}

/// A legal Normal deck in two colors whose creatures cost hybrid and Phyrexian symbols, derived entirely from `rng`.
///
/// It has its own generator, so [`random_deck`] and the games seeded from it stay as they were.
pub fn symbol_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let first = Color::ALL[rng.index(Color::ALL.len())];
    let second =
        Color::ALL[(first.index() + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()];
    let colors = [first, second];
    // Two Wastes give the colorless symbols something to pay with.
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS - 2).map(|i| land(colors[i % 2])).collect();
    deck.extend(std::iter::repeat_n(wastes(), 2));
    for slot in 0..DISTINCT_CREATURES {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let symbols: Vec<ManaSymbol> = std::iter::once(ManaSymbol::Generic(generic))
            .chain((0..=rng.index(3)).map(|_| random_symbol(rng, colors)))
            .collect();
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let cost = ManaCost::from_symbols(symbols.iter().copied());
        let name = format!("Symbol {slot} {cost} {power}/{toughness}");
        let card = creature_costing(&name, &symbols, power, toughness);
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    deck
}

/// A normal game of `players` seats whose decks are [`symbol_deck`]s, fully determined by `seed`.
pub fn symbol_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| symbol_deck(&mut GameRng::substream(seed, 2_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

/// A legal Normal deck in red, green and black whose spells and abilities run through the effect pipeline: burn
/// at any target, a spell that damages, gains life and draws, one that drains an opponent, creatures that ping
/// when tapped, an artifact that draws.
///
/// It has its own generator, so [`random_deck`] and the games seeded from it stay as they were.
pub fn spell_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    let spells = [
        burn("Spell Shock", 2),
        burn("Spell Bolt", 3),
        burn("Spell Spark", 1),
        salve("Spell Salve"),
        drain("Spell Siphon"),
        pinger("Spell Pinger", 0),
        pinger("Spell Prodigy", 1),
        library_of_cards("Spell Library", 2),
    ];
    let distinct_spells = spells.len();
    for card in spells {
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    for slot in distinct_spells..DISTINCT_CREATURES {
        let generic = u32::try_from(rng.index(4)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Spell Fuzz {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    deck
}

/// A normal game of `players` seats whose decks are [`spell_deck`]s, fully determined by `seed`.
pub fn spell_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| spell_deck(&mut GameRng::substream(seed, 3_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

/// A legal Normal deck in red, green and black whose cards trigger: creatures that draw or gain life as they enter,
/// attack or die, one that aims its trigger at a player, one with two triggers on the same event (so the order is
/// asked), artifacts that watch the table, upkeep and casting, two that set up a delayed trigger (one that triggers once and
/// one that lasts the turn), and the burn that kills creatures.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn trigger_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green, Color::Black];
    // Twenty-one lands and thirteen kinds of spell, three of each, make the 60 cards.
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS - 3).map(|i| land(colors[i % 3])).collect();
    let priced = |card: Arc<CardDefinition>, generic: u32| {
        Arc::new((*card).clone().with_mana_cost(cost(generic, &[])))
    };
    let small = |name: &str, color: Color, rules: Vec<Ability>| {
        with_rules(&creature(name, 1, &[color], 2, 2), rules)
    };
    let life_at_most_14 = Condition::Compare {
        left: Amount::Life(Ref::You),
        op: Cmp::Le,
        right: Amount::Fixed(14),
    };
    let cards = [
        burn("Trigger Shock", 2),
        pinger("Trigger Pinger", 0),
        drain("Trigger Siphon"),
        small(
            "Trigger Sage",
            Color::Green,
            vec![trigger(when::any_creature_dies(), vec![gain_life(1)])],
        ),
        small(
            "Trigger Drifter",
            Color::Green,
            vec![trigger(when::this_enters(), vec![draw(1)])],
        ),
        small(
            "Trigger Hawk",
            Color::Red,
            vec![aimed_at(
                trigger(
                    when::this_attacks(),
                    vec![Effect::LoseLife {
                        who: Ref::Target(0),
                        amount: Amount::Fixed(1),
                    }],
                ),
                Targetable::Opponent,
            )],
        ),
        small(
            "Trigger Ghost",
            Color::Black,
            vec![aimed_at(
                trigger(when::this_dies(), vec![damage(Ref::Target(0), 1)]),
                Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(Ref::Opponents),
                ])),
            )],
        ),
        small(
            "Trigger Twins",
            Color::Black,
            vec![
                trigger(
                    when::either(when::this_enters(), when::this_attacks()),
                    vec![gain_life(1)],
                ),
                trigger(when::this_enters(), vec![draw(1)]),
            ],
        ),
        artifact_costing(
            "Trigger Bell",
            1,
            vec![once_each_turn(trigger(
                when::your_land_enters(),
                vec![gain_life(1)],
            ))],
        ),
        artifact_costing(
            "Trigger Clock",
            1,
            vec![only_if(
                trigger(when::your_upkeep(), vec![gain_life(1)]),
                life_at_most_14,
            )],
        ),
        artifact_costing(
            "Trigger Mirror",
            2,
            vec![trigger(when::opponent_casts(), vec![gain_life(1)])],
        ),
        priced(hourglass("Trigger Hourglass", 2), 1),
        priced(vigil("Trigger Vigil", 1), 1),
    ];
    for card in cards {
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    rng.shuffle(&mut deck);
    deck
}

/// A normal game of `players` seats whose decks are [`trigger_deck`]s, fully determined by `seed`.
pub fn trigger_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| trigger_deck(&mut GameRng::substream(seed, 4_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

#[cfg(test)]
mod tests {
    //! Goal: generated decks are always legal Normal decks and are reproducible from the seed.
    use super::*;
    use mtg_core::card::CardType;
    use mtg_engine::format::FormatRules;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn decks_are_legal_and_reproducible(seed in any::<u64>()) {
            let deck = random_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            let again = random_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck, again);
        }
    }

    #[test]
    fn setups_have_one_deck_per_seat() {
        for players in [2, 3, 8] {
            let setup = fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
            let setup = symbol_fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
            let setup = spell_fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
            let setup = trigger_fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
        }
    }

    /// How many triggered abilities a card has.
    fn triggers(card: &Arc<CardDefinition>) -> usize {
        card.rules
            .iter()
            .filter(|rules| matches!(***rules, Ability::Triggered { .. }))
            .count()
    }

    /// Whether an ability of the card sets up a delayed trigger.
    fn delays(card: &Arc<CardDefinition>) -> bool {
        card.rules.iter().any(|rules| match &**rules {
            Ability::Activated { effects, .. } => effects
                .iter()
                .any(|effect| matches!(effect, Effect::DelayedTrigger { .. })),
            _ => false,
        })
    }

    proptest! {
        #[test]
        fn trigger_decks_are_legal_reproducible_and_hold_every_kind_of_trigger(seed in any::<u64>()) {
            let deck = trigger_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            prop_assert_eq!(&deck, &trigger_deck(&mut GameRng::substream(seed, 0)));
            prop_assert!(deck.iter().any(|c| triggers(c) == 2), "a card with two triggers");
            prop_assert!(deck.iter().any(|c| triggers(c) == 1 && c.is_type(CardType::Artifact)));
            prop_assert!(deck.iter().any(|c| triggers(c) == 1 && c.is_type(CardType::Creature)));
            prop_assert!(deck.iter().any(delays));
        }
    }

    proptest! {
        #[test]
        fn spell_decks_are_legal_reproducible_and_hold_spells_and_abilities(seed in any::<u64>()) {
            let deck = spell_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            prop_assert_eq!(&deck, &spell_deck(&mut GameRng::substream(seed, 0)));
            prop_assert!(deck.iter().any(|c| !c.rules.is_empty() && c.is_type(mtg_core::card::CardType::Instant)));
            prop_assert!(deck.iter().any(|c| !c.rules.is_empty() && c.is_type(mtg_core::card::CardType::Creature)));
        }
    }

    proptest! {
        #[test]
        fn symbol_decks_are_legal_reproducible_and_use_every_kind(seed in any::<u64>()) {
            let deck = symbol_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            prop_assert_eq!(&deck, &symbol_deck(&mut GameRng::substream(seed, 0)));
            prop_assert!(deck.iter().any(|c| c.mana_cost.as_ref().is_some_and(|m| !m.is_plain())));
        }
    }

    #[test]
    fn across_many_seeds_the_decks_hold_every_kind_of_symbol() {
        let mut seen = [false; 9];
        for seed in 0..60 {
            for card in symbol_deck(&mut GameRng::substream(seed, 0)) {
                for symbol in card.mana_cost.iter().flat_map(ManaCost::symbols) {
                    let kind = match symbol {
                        ManaSymbol::Generic(_) => 0,
                        ManaSymbol::Colored(_) => 1,
                        ManaSymbol::Colorless => 2,
                        ManaSymbol::Hybrid(..) => 3,
                        ManaSymbol::MonoHybrid(_) => 4,
                        ManaSymbol::Phyrexian(_) => 5,
                        ManaSymbol::HybridPhyrexian(..) => 6,
                        ManaSymbol::ColorlessHybrid(_) => 7,
                        ManaSymbol::ColorlessPhyrexian => 8,
                    };
                    seen[kind] = true;
                }
            }
        }
        // {C} alone is not generated: colorless only comes in the hybrid and Phyrexian forms.
        assert_eq!(
            seen,
            [true, true, false, true, true, true, true, true, true]
        );
    }
}
