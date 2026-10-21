//! Auras, Equipment and legendary permanents (CR 303.4, 301.5, 704.5j, 704.5m to 704.5p), and the decks and games built from
//! them: the corpus Auras, Equipment and legends, synthetic Auras for creatures, lands and opponents' creatures, synthetic
//! Equipment, a spell that returns an Aura, and the removal that takes their hosts away.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, cost, land};
use crate::corpus::{self, CorpusError};

/// Lands in an Aura deck: five colours in turn.
const LANDS: usize = 20;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// An Aura of this cost that enchants what `enchant` selects (written as data) and has these abilities.
fn aura(
    name: &str,
    (generic, colored): (u32, &[Color]),
    enchant: &str,
    abilities: &[&str],
) -> Result<Arc<CardDefinition>, CorpusError> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_subtype("Aura")
        .with_mana_cost(cost(generic, colored))
        .with_rules(rules(&format!(r#"{{"Keyword":{{"Enchant":{enchant}}}}}"#))?);
    for ability in abilities {
        card = card.with_rules(rules(ability)?);
    }
    Ok(Arc::new(card))
}

/// `{1}{G}` Aura: enchant creature. Enchanted creature gets +2/+2.
pub fn mighty_aura() -> Result<Arc<CardDefinition>, CorpusError> {
    aura(
        "Mighty Aura",
        (1, &[Color::Green]),
        r#"[{"Type":"Creature"}]"#,
        &[r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}"#],
    )
}

/// `{1}{U}` Aura: enchant creature. Enchanted creature has flying.
pub fn winged_aura() -> Result<Arc<CardDefinition>, CorpusError> {
    aura(
        "Winged Aura",
        (1, &[Color::Blue]),
        r#"[{"Type":"Creature"}]"#,
        &[r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Flying"}]}}"#],
    )
}

/// `{B}` Aura: enchant creature an opponent controls. Enchanted creature gets -2/-2.
pub fn hex_aura() -> Result<Arc<CardDefinition>, CorpusError> {
    aura(
        "Hex Aura",
        (0, &[Color::Black]),
        r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#,
        &[
            r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":-2,"toughness":-2}}]}}"#,
        ],
    )
}

/// `{G}` Aura: enchant land. Enchanted land has "{T}: You gain 1 life."
pub fn rune_aura() -> Result<Arc<CardDefinition>, CorpusError> {
    aura(
        "Rune Aura",
        (0, &[Color::Green]),
        r#"[{"Type":"Land"}]"#,
        &[
            r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}}]}}"#,
        ],
    )
}

/// An Equipment of this cost whose equipped creature gets `power`/`toughness` and that equips for `equip` (a mana cost).
fn equipment(
    name: &str,
    (generic, colored): (u32, &[Color]),
    equip: &str,
    (power, toughness): (i32, i32),
) -> Result<Arc<CardDefinition>, CorpusError> {
    let pump = format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"ModifyPT":{{"power":{power},"toughness":{toughness}}}}}]}}}}"#
    );
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(generic, colored))
            .with_rules(rules(&pump)?)
            .with_rules(rules(&format!(r#"{{"Keyword":{{"Equip":"{equip}"}}}}"#))?),
    ))
}

/// `{1}` Equipment: equipped creature gets +1/+1. Equip {1}.
pub fn iron_blade() -> Result<Arc<CardDefinition>, CorpusError> {
    equipment("Iron Blade", (1, &[]), "{1}", (1, 1))
}

/// `{2}` Equipment: equipped creature gets +0/+3. Equip {3}.
pub fn heavy_plate() -> Result<Arc<CardDefinition>, CorpusError> {
    equipment("Heavy Plate", (2, &[]), "{3}", (0, 3))
}

/// `{1}{W}` instant: return target Aura card from your graveyard to the battlefield.
pub fn call_back() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new("Call Back")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::White]))
            .with_rules(rules(
                r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"Subtype":"Aura"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#,
            )?),
    ))
}

/// A legendary creature of this cost and stats with no abilities.
pub fn legend(
    name: &str,
    (generic, colored): (u32, &[Color]),
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_mana_cost(cost(generic, colored))
            .with_power_toughness(power, toughness),
    )
}

/// `{1}{G}` legendary 2/2.
pub fn legendary_bear() -> Arc<CardDefinition> {
    legend("Legendary Bear", (1, &[Color::Green]), 2, 2)
}

/// `{2}{R}` legendary 3/2.
pub fn legendary_ogre() -> Arc<CardDefinition> {
    legend("Legendary Ogre", (2, &[Color::Red]), 3, 2)
}

/// `{2}` legendary artifact with no abilities.
pub fn legendary_relic() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Legendary Relic")
            .with_type(CardType::Artifact)
            .with_supertype(Supertype::Legendary)
            .with_mana_cost(cost(2, &[])),
    )
}

/// The corpus Auras, Equipment and legends the deck holds.
pub const ATTACH_CORPUS_CARDS: [&str; 10] = [
    "Pacifism",
    "Epic Proportions",
    "Control Magic",
    "Squirrel Nest",
    "Shielded by Faith",
    "Arvad the Cursed",
    "Kongming, \"Sleeping Dragon\"",
    "Sram, Senior Edificer",
    "Bonesplitter",
    "Swiftfoot Boots",
];

/// The cards of this module and the plain ones the deck is made of.
pub const ATTACH_SYNTHETIC_CARDS: [&str; 13] = [
    "Mighty Aura",
    "Winged Aura",
    "Hex Aura",
    "Rune Aura",
    "Iron Blade",
    "Heavy Plate",
    "Call Back",
    "Legendary Bear",
    "Legendary Ogre",
    "Legendary Relic",
    "Grizzly Bears",
    "Spark",
    "Lightning Bolt",
];

/// A legal Normal deck of Auras, Equipment and legendary permanents: the corpus Auras, Equipment and legends, the synthetic
/// Auras for creatures, lands and opponents' creatures, the synthetic Equipment, the spell that returns an Aura, legends
/// that repeat so the legend rule applies often, and the burn that takes a host away.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn attach_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Red,
        Color::White,
        Color::Green,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in [
        ("Pacifism", 2),
        ("Epic Proportions", 1),
        ("Control Magic", 2),
        ("Squirrel Nest", 2),
        ("Shielded by Faith", 2),
        ("Arvad the Cursed", 2),
        ("Kongming, \"Sleeping Dragon\"", 2),
        ("Sram, Senior Edificer", 2),
        ("Bonesplitter", 2),
        ("Swiftfoot Boots", 1),
        ("Lightning Bolt", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (mighty_aura()?, 2),
        (winged_aura()?, 1),
        (hex_aura()?, 2),
        (rune_aura()?, 1),
        (iron_blade()?, 2),
        (heavy_plate()?, 1),
        (call_back()?, 1),
        (legendary_bear(), 3),
        (legendary_ogre(), 2),
        (legendary_relic(), 1),
        (burn("Spark", 2), 2),
        (bear(), 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`attach_deck`]s, fully determined by `seed`.
pub fn attach_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(attach_deck(&mut GameRng::substream(seed, 19_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the Auras, Equipment and spells of this module use only what the engine runs and are valid rules, an Aura deck is a legal sixty cards with every Aura, every Equipment, every legend and the removal in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn the_synthetic_cards_use_primitives_the_engine_runs() {
        let caps = caps();
        let cards = [
            mighty_aura().unwrap(),
            winged_aura().unwrap(),
            hex_aura().unwrap(),
            rune_aura().unwrap(),
            iron_blade().unwrap(),
            heavy_plate().unwrap(),
            call_back().unwrap(),
        ];
        for card in cards {
            assert_ne!(card.rules.len(), 0);
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
        for card in [mighty_aura().unwrap(), winged_aura().unwrap()] {
            assert!(card.subtypes.iter().any(|subtype| subtype.0 == "Aura"));
        }
        for card in [iron_blade().unwrap(), heavy_plate().unwrap()] {
            assert!(card.subtypes.iter().any(|subtype| subtype.0 == "Equipment"));
        }
        assert!(legendary_bear().supertypes.contains(&Supertype::Legendary));
    }

    #[test]
    fn an_aura_deck_is_sixty_cards_with_every_aura_legend_and_removal() {
        let deck = attach_deck(&mut GameRng::substream(1, 19_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in ATTACH_CORPUS_CARDS.iter().chain(&ATTACH_SYNTHETIC_CARDS) {
            assert!(deck.iter().any(|card| card.name == *name), "{name}");
        }
        let setup = attach_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&attach_fuzz_setup(3, 5).unwrap()));
    }
}
