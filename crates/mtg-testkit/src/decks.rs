//! Seeded random 60-card decks for fuzz games: basic lands and vanilla creatures, and decks whose cards use the
//! effect pipeline.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{
    bottomer, burn, cost, creature, creature_costing, damage, drain, draw, edict, gain_life,
    keyworded_creature, land, library_of_cards, mind_rot, pinger, revealer, salve, scryer, stirrer,
    walker, wastes,
};
use crate::corpus::{self, CorpusError};
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
        .map(|seat| trigger_deck(&mut GameRng::substream(seed, 5_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

/// A legal Normal deck in black and green whose cards make players choose among cards: four Archon of Cruelty (the
/// corpus card, repriced so a bot pays for it), four edicts, four spells that make a player discard two cards, four
/// planeswalkers and creatures between them to sacrifice, and lands.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn archon_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Black, Color::Green];
    let archon = Arc::new(
        (*corpus::card("Archon of Cruelty")?)
            .clone()
            .with_mana_cost(cost(2, &[])),
    );
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS - 2).map(|i| land(colors[i % 2])).collect();
    for card in [
        archon,
        edict("Archon Edict"),
        mind_rot("Archon Rot"),
        walker("Archon Walker", 1),
    ] {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    // Twenty-two creatures make the 60 cards.
    for slot in 0..11 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Archon Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(2)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`archon_deck`]s, fully determined by `seed`.
pub fn archon_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(archon_deck(&mut GameRng::substream(seed, 6_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A legal Normal deck in blue and green of Opt, Brainstorm and spells that scry, look at the top of libraries, put
/// cards on the bottom of them, shuffle and reveal, with creatures to fill it, derived entirely from `rng`.
pub fn library_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS - 2).map(|i| land(colors[i % 2])).collect();
    for card in [
        corpus::card("Opt")?,
        corpus::card("Brainstorm")?,
        scryer("Library Scry", 1, 3),
        bottomer("Library Bottom"),
        stirrer("Library Stir"),
        revealer("Library Reveal"),
    ] {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    // Twenty creatures make the 60 cards.
    for slot in 0..10 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Library Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(2)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`library_deck`]s, fully determined by `seed`.
pub fn library_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(library_deck(&mut GameRng::substream(seed, 9_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A legal Normal deck in green and blue of Rampant Growth and Cultivate (the corpus cards, as printed) with creatures
/// to fill it, derived entirely from `rng`.
pub fn ramp_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Green, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 2])).collect();
    for name in ["Rampant Growth", "Cultivate"] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, 4));
    }
    // Twenty-eight creatures make the 60 cards.
    for slot in 0..14 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Ramp Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(2)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`ramp_deck`]s, fully determined by `seed`.
pub fn ramp_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(ramp_deck(&mut GameRng::substream(seed, 11_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A legal Normal deck in blue, red and black of the cards that return cards of a graveyard or hands back by an exact
/// count of X targets or by a pick at random (the corpus cards, as printed), instants to fill the graveyards, and
/// creatures, derived entirely from `rng`.
pub fn pick_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Surreal Memoir", 3),
        ("Charmbreaker Devils", 2),
        ("Distorting Wake", 3),
        ("Entreat the Dead", 3),
        ("Back in Town", 2),
        ("Reap the Past", 2),
        ("Lightning Bolt", 4),
        ("Counterspell", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    // Fifteen creatures make the 60 cards.
    for slot in 0..15 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Pick Fodder {slot} {generic} {power}/{toughness}");
        deck.push(creature(
            &name,
            generic,
            &[colors[rng.index(3)]],
            power,
            toughness,
        ));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`pick_deck`]s, fully determined by `seed`.
pub fn pick_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(pick_deck(&mut GameRng::substream(seed, 12_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A legal Normal deck in white, blue and black whose cards remove and counter: four each of Doom Blade, Swords to
/// Plowshares and Counterspell and two Wrath of God (the corpus cards, as printed), creatures that watch deaths, die
/// with a trigger aimed at an opponent or leave the battlefield with one, creatures that cannot be destroyed, and lands.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn removal_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::White, Color::Blue, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Doom Blade", 4),
        ("Swords to Plowshares", 4),
        ("Counterspell", 4),
        ("Wrath of God", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let small = |name: &str, color: Color, rules: Vec<Ability>| {
        with_rules(&creature(name, 1, &[color], 2, 2), rules)
    };
    let leaves = TriggerPattern::ZoneChange {
        what: Selector::this(),
        from: Some(ZoneKind::Battlefield),
        to: None,
    };
    let cards = [
        small(
            "Removal Mourner",
            Color::White,
            vec![trigger(when::any_creature_dies(), vec![gain_life(1)])],
        ),
        small(
            "Removal Shade",
            Color::Black,
            vec![aimed_at(
                trigger(
                    when::this_dies(),
                    vec![Effect::LoseLife {
                        who: Ref::Target(0),
                        amount: Amount::Fixed(1),
                    }],
                ),
                Targetable::Opponent,
            )],
        ),
        small(
            "Removal Wisp",
            Color::Blue,
            vec![trigger(leaves, vec![draw(1)])],
        ),
        keyworded_creature(
            "Removal Rock",
            2,
            &[Color::Black],
            3,
            3,
            &[Keyword::Indestructible],
        ),
    ];
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    // Ten plain creatures make the 60 cards.
    for slot in 0..5 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Removal Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`removal_deck`]s, fully determined by `seed`.
pub fn removal_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(removal_deck(&mut GameRng::substream(seed, 7_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// The keywords of combat, in the order the decks pick from.
pub fn combat_keywords() -> [Keyword; 12] {
    [
        Keyword::Flying,
        Keyword::Reach,
        Keyword::Menace,
        Keyword::Defender,
        Keyword::Haste,
        Keyword::Vigilance,
        Keyword::FirstStrike,
        Keyword::DoubleStrike,
        Keyword::Deathtouch,
        Keyword::Trample,
        Keyword::Lifelink,
        Keyword::Indestructible,
    ]
}

/// A legal Normal deck in two colors whose creatures hold up to three keywords of combat, derived entirely from `rng`.
///
/// It has its own generator, so [`random_deck`] and the games seeded from it stay as they were.
pub fn keyword_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let first = Color::ALL[rng.index(Color::ALL.len())];
    let second =
        Color::ALL[(first.index() + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()];
    let colors = [first, second];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 2])).collect();
    for slot in 0..DISTINCT_CREATURES {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let cost_colors = [colors[rng.index(2)]];
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let mut pool = combat_keywords().to_vec();
        let held: Vec<Keyword> = (0..rng.index(4))
            .map(|_| pool.remove(rng.index(pool.len())))
            .collect();
        let words: Vec<&str> = held.iter().map(Keyword::name).collect();
        let name = format!("Combat {slot} {power}/{toughness} {}", words.join(" "));
        let card = keyworded_creature(&name, generic, &cost_colors, power, toughness, &held);
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    deck
}

/// A normal game of `players` seats whose decks are [`keyword_deck`]s, fully determined by `seed`.
pub fn keyword_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| keyword_deck(&mut GameRng::substream(seed, 4_000 + seat)))
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

    proptest! {
        #[test]
        fn keyword_decks_are_legal_reproducible_and_hold_keywords(seed in any::<u64>()) {
            let deck = keyword_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            prop_assert_eq!(&deck, &keyword_deck(&mut GameRng::substream(seed, 0)));
            for card in deck.iter().filter(|c| c.is_type(mtg_core::card::CardType::Creature)) {
                prop_assert!(card.rules.len() <= 3);
            }
        }
    }

    #[test]
    fn across_many_seeds_the_decks_hold_every_combat_keyword() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..60 {
            for card in keyword_deck(&mut GameRng::substream(seed, 0)) {
                for rule in &card.rules {
                    if let mtg_cards::ability::Ability::Keyword(keyword) = &**rule {
                        seen.insert(keyword.name());
                    }
                }
            }
        }
        let want: std::collections::BTreeSet<&str> =
            combat_keywords().iter().map(Keyword::name).collect();
        assert_eq!(seen, want);
    }

    #[test]
    fn setups_have_one_deck_per_seat() {
        for players in [2, 3, 8] {
            let setup = keyword_fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
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
