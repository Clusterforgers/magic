//! Cards that add turns and phases, and a seeded deck of them and the corpus cards that do for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, PhaseKind};
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land};
use crate::corpus;

/// The corpus cards of the deck of turns besides lands: spells that take extra turns, with a skipped untap step or for
/// another player, and spells and abilities that add a combat phase and a main phase after the main phase they are used in.
pub const TURN_CARDS: &[&str] = &[
    "Time Walk",
    "Temporal Manipulation",
    "Capture of Jingzhou",
    "Time Warp",
    "Time Stretch",
    "Savor the Moment",
    "Aggravated Assault",
    "Seize the Day",
    "Time Sieve",
    "Magistrate's Scepter",
    "Hellkite Charger",
    "Lightning Runner",
    "Godo, Bandit Warlord",
    "Scourge of the Throne",
    "Hexplate Wallbreaker",
    "Tifa, Martial Artist",
];

/// A free artifact with a rule: "If an opponent would begin an extra turn, that player skips that turn instead."
pub fn stranglehold() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Fuzz Stranglehold")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Rule {
                rule: GameRule::SkipExtraTurns(Ref::Opponents),
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// A free sorcery whose effects are `effects`.
fn free_spell(name: &str, card_type: CardType, effects: Vec<Effect>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(card_type)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects,
            }),
    )
}

/// A free sorcery: "`who` takes `count` extra turns after this one", the untap step of each skipped or not.
pub fn time_spell(name: &str, who: Ref, count: i32, skip_untap: bool) -> Arc<CardDefinition> {
    free_spell(
        name,
        CardType::Sorcery,
        vec![Effect::ExtraTurn {
            who,
            count: Amount::Fixed(count),
            skip_untap,
        }],
    )
}

/// A free instant that adds `phases` after the phase it resolves in.
pub fn phase_spell(name: &str, phases: &[PhaseKind]) -> Arc<CardDefinition> {
    free_spell(
        name,
        CardType::Instant,
        vec![Effect::AddPhase {
            phases: phases.to_vec(),
        }],
    )
}

/// A legal Normal deck of lands, three of each card of [`TURN_CARDS`] and of the spells that add turns and phases,
/// derived entirely from `rng`.
pub fn turn_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Blue, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..36).map(|i| land(colors[i % 4])).collect();
    for card in TURN_CARDS.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    deck.extend(std::iter::repeat_n(
        time_spell("Fuzz Hourglass", Ref::EachPlayer, 1, true),
        2,
    ));
    deck.extend(std::iter::repeat_n(stranglehold(), 2));
    deck.extend(std::iter::repeat_n(
        phase_spell(
            "Fuzz Assault",
            &[PhaseKind::Combat, PhaseKind::Main, PhaseKind::Beginning],
        ),
        2,
    ));
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`turn_deck`]s, fully determined by `seed`.
pub fn turn_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| turn_deck(&mut GameRng::substream(seed, 83_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
