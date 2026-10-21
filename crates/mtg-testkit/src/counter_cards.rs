//! Counters on permanents and players (CR 122), and the decks and games built from them: the corpus cards that put
//! counters on creatures and players, and cards that put, remove and move +1/+1 and -1/-1 counters, give players poison,
//! energy and experience counters, and read the counters of a creature that has died.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, cost, land};
use crate::corpus::{self, CorpusError};

/// Lands in a counters deck: five colours in turn.
const LANDS: usize = 22;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A card of this type and cost with these abilities, written as data.
fn card(
    name: &str,
    card_type: CardType,
    (generic, colored): (u32, &[Color]),
    stats: Option<(i32, i32)>,
    abilities: &[&str],
) -> Result<Arc<CardDefinition>, CorpusError> {
    let mut card = CardDefinition::new(name)
        .with_type(card_type)
        .with_mana_cost(cost(generic, colored));
    if let Some((power, toughness)) = stats {
        card = card.with_power_toughness(power, toughness);
    }
    for ability in abilities {
        card = card.with_rules(rules(ability)?);
    }
    Ok(Arc::new(card))
}

const A_CREATURE: &str = r#"{"what":{"Object":[{"Type":"Creature"}]}}"#;

/// `{G}` instant: put a +1/+1 counter on target creature.
pub fn growth_spurt() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{A_CREATURE}],"effects":[{{"PutCounters":{{"on":{{"Target":0}},"counter":"+1/+1"}}}}]}}}}"#
    );
    card(
        "Growth Spurt",
        CardType::Instant,
        (0, &[Color::Green]),
        None,
        &[&spell],
    )
}

/// `{B}` instant: put a -1/-1 counter on target creature.
pub fn sap() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{A_CREATURE}],"effects":[{{"PutCounters":{{"on":{{"Target":0}},"counter":"-1/-1"}}}}]}}}}"#
    );
    card(
        "Sap",
        CardType::Instant,
        (0, &[Color::Black]),
        None,
        &[&spell],
    )
}

/// `{2}{B}` sorcery: put two -1/-1 counters on each creature.
pub fn wither_wave() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"-1/-1","count":2}}]}}"#;
    card(
        "Wither Wave",
        CardType::Sorcery,
        (2, &[Color::Black]),
        None,
        &[spell],
    )
}

/// `{B}` instant: target player gets four poison counters.
pub fn venomous_bite() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"poison","count":4}}]}}"#;
    card(
        "Venomous Bite",
        CardType::Instant,
        (0, &[Color::Black]),
        None,
        &[spell],
    )
}

/// `{1}{G}` sorcery: move all the +1/+1 counters from target creature onto another target creature.
pub fn counter_siphon() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{A_CREATURE},{A_CREATURE}],"effects":[{{"MoveCounters":{{"from":{{"Target":0}},"to":{{"Target":1}},"counter":"+1/+1","count":{{"Counters":{{"on":{{"Target":0}},"counter":"+1/+1"}}}}}}}}]}}}}"#
    );
    card(
        "Counter Siphon",
        CardType::Sorcery,
        (1, &[Color::Green]),
        None,
        &[&spell],
    )
}

/// `{U}` sorcery: remove a +1/+1 counter from target creature you control, then draw a card.
pub fn soul_siphon() -> Result<Arc<CardDefinition>, CorpusError> {
    let spell = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":"+1/+1"}},{"Draw":{}}]}}"#;
    card(
        "Soul Siphon",
        CardType::Sorcery,
        (0, &[Color::Blue]),
        None,
        &[spell],
    )
}

/// `{1}{G}` 1/1 creature: `{G}`, `{T}`: put a +1/+1 counter on it.
pub fn graftling() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Activated":{"cost":[{"Mana":"{G}"},"Tap"],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
    card(
        "Graftling",
        CardType::Creature,
        (1, &[Color::Green]),
        Some((1, 1)),
        &[ability],
    )
}

/// `{1}{R}` 1/1 creature: whenever it attacks, put a +1/+1 counter on it.
pub fn rampaging_cub() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
    card(
        "Rampaging Cub",
        CardType::Creature,
        (1, &[Color::Red]),
        Some((1, 1)),
        &[ability],
    )
}

/// `{1}{B}` 2/1 creature: whenever a creature you control dies, draw a card for each +1/+1 counter it had.
pub fn death_watcher() -> Result<Arc<CardDefinition>, CorpusError> {
    let ability = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{"count":{"Counters":{"on":"Triggering","counter":"+1/+1"}}}}]}}"#;
    card(
        "Death Watcher",
        CardType::Creature,
        (1, &[Color::Black]),
        Some((2, 1)),
        &[ability],
    )
}

/// `{1}` artifact: when it enters, you get two energy counters; `{T}`: remove an energy counter from you and gain
/// 1 life.
pub fn energy_cell() -> Result<Arc<CardDefinition>, CorpusError> {
    let enters = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}}]}}"#;
    let spend = r#"{"Activated":{"cost":["Tap"],"effects":[{"RemoveCounters":{"from":"You","counter":{"Other":"energy"}}},{"GainLife":{"amount":1}}]}}"#;
    card(
        "Energy Cell",
        CardType::Artifact,
        (1, &[]),
        None,
        &[enters, spend],
    )
}

/// `{1}{W}` 1/2 creature: whenever a creature you control dies, you get an experience counter; at the beginning of your
/// end step, you gain life equal to the experience counters you have.
pub fn veteran_recruiter() -> Result<Arc<CardDefinition>, CorpusError> {
    let dies = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"experience"}}}]}}"#;
    let end = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"GainLife":{"amount":{"Counters":{"on":"You","counter":{"Other":"experience"}}}}}]}}"#;
    card(
        "Veteran Recruiter",
        CardType::Creature,
        (1, &[Color::White]),
        Some((1, 2)),
        &[dies, end],
    )
}

/// The corpus cards that put counters on creatures and players, or read them.
pub const COUNTER_CORPUS_CARDS: [&str; 4] = [
    "Tireless Tracker",
    "Cathars' Crusade",
    "Inspiring Call",
    "Caress of Phyrexia",
];

/// The cards of this module.
pub const COUNTER_SYNTHETIC_CARDS: [&str; 11] = [
    "Growth Spurt",
    "Sap",
    "Wither Wave",
    "Venomous Bite",
    "Counter Siphon",
    "Soul Siphon",
    "Graftling",
    "Rampaging Cub",
    "Death Watcher",
    "Energy Cell",
    "Veteran Recruiter",
];

/// A legal Normal deck in five colours of cards that put, remove and move counters: the corpus cards and the cards above,
/// with Grizzly Bears to carry the counters and Lightning Bolt to kill what carries them.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn counters_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Green,
        Color::Black,
        Color::White,
        Color::Red,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in [
        ("Tireless Tracker", 3),
        ("Cathars' Crusade", 2),
        ("Inspiring Call", 2),
        ("Caress of Phyrexia", 2),
        ("Lightning Bolt", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (growth_spurt()?, 3),
        (sap()?, 2),
        (wither_wave()?, 1),
        (venomous_bite()?, 2),
        (counter_siphon()?, 2),
        (soul_siphon()?, 2),
        (graftling()?, 3),
        (rampaging_cub()?, 3),
        (death_watcher()?, 2),
        (energy_cell()?, 1),
        (veteran_recruiter()?, 2),
        (bear(), 4),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`counters_deck`]s, fully determined by `seed`.
pub fn counters_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(counters_deck(&mut GameRng::substream(seed, 16_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic cards of the counters deck use only what the engine runs and are valid rules, a counters deck is a legal sixty cards with every card that puts or reads counters in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    fn synthetic() -> Vec<Arc<CardDefinition>> {
        vec![
            growth_spurt().unwrap(),
            sap().unwrap(),
            wither_wave().unwrap(),
            venomous_bite().unwrap(),
            counter_siphon().unwrap(),
            soul_siphon().unwrap(),
            graftling().unwrap(),
            rampaging_cub().unwrap(),
            death_watcher().unwrap(),
            energy_cell().unwrap(),
            veteran_recruiter().unwrap(),
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
                .collect::<Vec<_>>(),
            COUNTER_SYNTHETIC_CARDS
        );
    }

    #[test]
    fn a_counters_deck_is_sixty_cards_with_every_card_that_puts_or_reads_counters() {
        let deck = counters_deck(&mut GameRng::substream(1, 16_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in COUNTER_CORPUS_CARDS.iter().chain(&COUNTER_SYNTHETIC_CARDS) {
            assert!(deck.iter().any(|card| card.name == *name), "{name}");
        }
        let setup = counters_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = counters_fuzz_setup(3, 5).unwrap();
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
