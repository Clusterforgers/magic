//! Cards that change what spells cost (CR 601.2f): permanents with a cost increase or reduction, and a seeded deck of them
//! beside spells to cast under them, for fuzz games. Each modifier is written as data, the way a card file holds it.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Ref, Targetable};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{
    burn, cost, creature, creature_costing, damage, hybrid_bear, land, one_target, phyrexian_bear,
};
use crate::corpus::{self, CorpusError};
use crate::x_cards::{blaze, hybrid_font};

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

fn mana(text: &str) -> Result<ManaCost, CorpusError> {
    ManaCost::parse(text).map_err(|error| CorpusError::Data(format!("{text}: {error}")))
}

/// An artifact costing `printed` with the cost modifier `modifier`, written as the card file holds it.
fn artifact(name: &str, printed: &str, modifier: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(mana(printed)?)
            .with_rules(rules(modifier)?),
    ))
}

/// A modifier of `mana` more or less (`direction`: `Increase` or `Reduce`) for the spells `applies_to` names; `extra`
/// holds the fields after the mana (`,"per":...`, `,"condition":...`).
fn change(applies_to: &str, direction: &str, mana: &str, extra: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":{applies_to},"change":"{direction}","mana":"{mana}"{extra}}}}}"#
    )
}

/// An artifact for `{1}`: noncreature spells, whoever casts them, cost {1} more.
pub fn tax_rock() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Tax Rock",
        "{1}",
        &change(
            r#"["Spell",{"Not":{"Type":"Creature"}}]"#,
            "Increase",
            "{1}",
            "",
        ),
    )
}

/// An artifact for `{2}`: spells, whoever casts them, cost {1} more.
pub fn sphere() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Sphere",
        "{2}",
        &change(r#"["Spell"]"#, "Increase", "{1}", ""),
    )
}

/// An artifact for `{2}`: spells your opponents cast cost {1} more.
pub fn hostile_tax() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Hostile Tax",
        "{2}",
        &change(
            r#"["Spell",{"ControlledBy":"Opponents"}]"#,
            "Increase",
            "{1}",
            "",
        ),
    )
}

/// An artifact for `{2}`: spells, whoever casts them, cost {1} less.
pub fn helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact("Helm", "{2}", &change(r#"["Spell"]"#, "Reduce", "{1}", ""))
}

/// An artifact for `{2}`: spells you cast cost {1} less.
pub fn my_helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "My Helm",
        "{2}",
        &change(r#"["Spell",{"ControlledBy":"You"}]"#, "Reduce", "{1}", ""),
    )
}

/// An artifact for `{3}`: spells you cast cost {2} less.
pub fn deep_helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Deep Helm",
        "{3}",
        &change(r#"["Spell",{"ControlledBy":"You"}]"#, "Reduce", "{2}", ""),
    )
}

/// An artifact for `{2}`: spells you cast cost {R} less.
pub fn red_helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Red Helm",
        "{2}",
        &change(r#"["Spell",{"ControlledBy":"You"}]"#, "Reduce", "{R}", ""),
    )
}

/// An artifact for `{2}`: spells you cast cost {1} less for each creature you control.
pub fn crowd_helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Crowd Helm",
        "{2}",
        &change(
            r#"["Spell",{"ControlledBy":"You"}]"#,
            "Reduce",
            "{1}",
            r#","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
        ),
    )
}

/// An artifact for `{2}`: spells cost {1} less to cast during the turn of the player who controls it.
pub fn day_helm() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Day Helm",
        "{2}",
        &change(
            r#"["Spell"]"#,
            "Reduce",
            "{1}",
            r#","condition":"YourTurn""#,
        ),
    )
}

/// An artifact for `{2}`: red spells cost {U} more.
pub fn blue_tax() -> Result<Arc<CardDefinition>, CorpusError> {
    artifact(
        "Blue Tax",
        "{2}",
        &change(r#"["Spell",{"Color":"Red"}]"#, "Increase", "{U}", ""),
    )
}

/// A sorcery for `{3}{R}`: 4 damage to any target.
pub fn big_bolt() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Big Bolt")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(3, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 4)],
            }),
    )
}

/// An artifact for `{1}` with no ability: something to cast under the modifiers that name artifacts.
pub fn trinket() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trinket")
            .with_type(CardType::Artifact)
            .with_mana_cost(ManaCost::from_symbols([ManaSymbol::Generic(1)])),
    )
}

/// A spell with no mana cost to speak of: a creature with the printed cost `{0}`.
pub fn free_bear() -> Arc<CardDefinition> {
    creature("Free Bear", 0, &[], 2, 2)
}

/// A 2/2 for `{W/P}{W/P}{1}`: Phyrexian symbols beside generic mana.
pub fn double_phyrexian() -> Arc<CardDefinition> {
    creature_costing(
        "Double Phyrexian",
        &[
            ManaSymbol::Generic(1),
            ManaSymbol::Phyrexian(Color::White),
            ManaSymbol::Phyrexian(Color::White),
        ],
        2,
        2,
    )
}

/// The cards of this module that are permanents with a modifier.
pub const MODIFIER_CARDS: &[&str] = &[
    "Tax Rock",
    "Sphere",
    "Hostile Tax",
    "Helm",
    "My Helm",
    "Deep Helm",
    "Red Helm",
    "Crowd Helm",
    "Day Helm",
    "Blue Tax",
];

/// The modifier permanents, in the order of [`MODIFIER_CARDS`].
pub fn modifiers() -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    Ok(vec![
        tax_rock()?,
        sphere()?,
        hostile_tax()?,
        helm()?,
        my_helm()?,
        deep_helm()?,
        red_helm()?,
        crowd_helm()?,
        day_helm()?,
        blue_tax()?,
    ])
}

/// The modifier permanent called `name` ([`MODIFIER_CARDS`]).
pub fn modifier(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    modifiers()?
        .into_iter()
        .find(|card| card.name == name)
        .ok_or_else(|| CorpusError::Data(format!("no modifier card called {name}")))
}

/// The spell called `name` among those a modifier changes: the cards of this module, `Bolt` and the hybrid and
/// Phyrexian bears.
pub fn spell(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(match name {
        "Bolt" => burn("Bolt", 3),
        "Big Bolt" => big_bolt(),
        "Trinket" => trinket(),
        "Free Bear" => free_bear(),
        "Hybrid Bear" => hybrid_bear(),
        "Phyrexian Bear" => phyrexian_bear(),
        "Double Phyrexian" => double_phyrexian(),
        "Blaze" => blaze()?,
        "Hybrid Font" => hybrid_font()?,
        other => corpus::card(other)?,
    })
}

/// A legal Normal deck of lands, the modifier permanents, spells that are changed by them (X spells, hybrid and
/// Phyrexian costs, Fireball, the corpus cards Thalia and Foundry Inspector) and creatures, derived entirely from `rng`.
pub fn cost_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    let mut cards = modifiers().unwrap_or_default();
    cards.extend(
        [
            "Fireball",
            "Thalia, Guardian of Thraben",
            "Foundry Inspector",
        ]
        .iter()
        .filter_map(|name| corpus::card(name).ok()),
    );
    cards.extend(
        [
            blaze().ok(),
            hybrid_font().ok(),
            Some(big_bolt()),
            Some(burn("Bolt", 3)),
            Some(trinket()),
            Some(hybrid_bear()),
            Some(phyrexian_bear()),
            Some(double_phyrexian()),
            Some(free_bear()),
        ]
        .into_iter()
        .flatten(),
    );
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    for slot in 0..4 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Cost Fodder {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(3)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`cost_deck`]s, fully determined by `seed`.
pub fn cost_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| cost_deck(&mut GameRng::substream(seed, 21_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
