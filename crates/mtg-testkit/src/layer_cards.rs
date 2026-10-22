//! Synthetic cards for the derivation of CR 613 and the decks and games built from them: effects that depend on each
//! other (creatures are Elves, Elves are artifacts, Elves are creatures), a loop (Elves are not artifacts, artifact
//! creatures are Elves), effects that read power, a creature whose power and toughness a characteristic-defining
//! ability counts, and an ability given to every creature that is itself a static ability, beside the corpus cards that
//! define their own characteristics (Nightmare, Maro, Transguild Courier) or change many at once (Kormus Bell,
//! Humility, Levitation, Favorable Winds, Goblin Chieftain, Glorious Anthem).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a layers deck: six Swamps, six Forests and four each of three other colours.
const LANDS: [(Color, usize); 5] = [
    (Color::Black, 6),
    (Color::Green, 6),
    (Color::Blue, 4),
    (Color::Red, 4),
    (Color::White, 4),
];

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// An enchantment whose only ability is a static one over `affected` with the modifications `mods`.
fn enchantment(
    name: &str,
    generic: u32,
    colors: &[Color],
    affected: &str,
    mods: &str,
) -> Result<Arc<CardDefinition>, CorpusError> {
    let json = format!(r#"{{"Static":{{"affected":{affected},"mods":{mods}}}}}"#);
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(generic, colors))
            .with_rules(rules(&json)?),
    ))
}

const CREATURES: &str = r#"{"Each":[{"Type":"Creature"}]}"#;
const ELVES: &str = r#"{"Each":[{"Subtype":"Elf"}]}"#;

/// `{1}{G}` enchantment: creatures are Elves.
pub fn veil_of_leaves() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Veil of Leaves",
        1,
        &[Color::Green],
        CREATURES,
        r#"[{"AddSubtype":"Elf"}]"#,
    )
}

/// `{1}{U}` enchantment: Elves are artifacts (it applies to what Veil of Leaves makes, whichever came first).
pub fn elven_forge() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Elven Forge",
        1,
        &[Color::Blue],
        ELVES,
        r#"[{"AddType":"Artifact"}]"#,
    )
}

/// `{1}{R}` enchantment: Elves are not artifacts (it loops with Gear Chorus).
pub fn rust_hex() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Rust Hex",
        1,
        &[Color::Red],
        ELVES,
        r#"[{"RemoveType":"Artifact"}]"#,
    )
}

/// `{1}{W}` enchantment: artifact creatures are Elves (it loops with Rust Hex).
pub fn gear_chorus() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Gear Chorus",
        1,
        &[Color::White],
        r#"{"Each":[{"Type":"Artifact"},{"Type":"Creature"}]}"#,
        r#"[{"AddSubtype":"Elf"}]"#,
    )
}

/// `{2}{U}` enchantment: Elves are creatures (it waits for whatever decides who is an Elf).
pub fn tin_awakening() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Tin Awakening",
        2,
        &[Color::Blue],
        ELVES,
        r#"[{"AddType":"Creature"}]"#,
    )
}

/// `{2}{G}` enchantment: creatures with power 3 or more get +1/+1 (who they are depends on every other change of power).
pub fn big_hearts() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Big Hearts",
        2,
        &[Color::Green],
        r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":1}}]"#,
    )
}

/// `{1}{R}` enchantment: creatures get +1/+0.
pub fn rally_cry() -> Result<Arc<CardDefinition>, CorpusError> {
    enchantment(
        "Rally Cry",
        1,
        &[Color::Red],
        CREATURES,
        r#"[{"ModifyPT":{"power":1,"toughness":0}}]"#,
    )
}

/// `{3}` artifact: creatures you control have "this creature gets +1/+1": a static ability that is given.
pub fn banner_of_pride() -> Result<Arc<CardDefinition>, CorpusError> {
    let json = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}}]}}"#;
    Ok(Arc::new(
        CardDefinition::new("Banner of Pride")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(3, &[]))
            .with_rules(rules(json)?),
    ))
}

/// `{3}{G}` creature whose power is the number of creatures its controller has and whose toughness is one more.
pub fn hulk() -> Result<Arc<CardDefinition>, CorpusError> {
    let count = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;
    let json = format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{count},"toughness":{{"Sum":[{count},1]}}}}}}],"defining":true}}}}"#
    );
    Ok(Arc::new(
        CardDefinition::new("Hulk")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(3, &[Color::Green]))
            .with_rules(rules(&json)?),
    ))
}

/// `{1}{R}` Goblin 1/1, whom Goblin Chieftain pumps.
pub fn goblin_scrap() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Goblin Scrap")
            .with_type(CardType::Creature)
            .with_subtype("Goblin")
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_power_toughness(1, 1),
    )
}

/// The names of the cards a game of a layers deck is expected to bring into play: every card that changes the layers
/// but Nightmare, which costs six and may never be cast.
pub const LAYER_CARDS: &[&str] = &[
    "Veil of Leaves",
    "Elven Forge",
    "Rust Hex",
    "Gear Chorus",
    "Tin Awakening",
    "Big Hearts",
    "Rally Cry",
    "Banner of Pride",
    "Hulk",
    "Maro",
    "Transguild Courier",
    "Kormus Bell",
    "Humility",
    "Levitation",
    "Favorable Winds",
    "Goblin Chieftain",
    "Glorious Anthem",
];

/// A legal Normal deck of sixty in five colours: the synthetic cards above, the corpus cards that define their own
/// characteristics or change many (Nightmare, Maro, Transguild Courier, Kormus Bell, Humility, Levitation, Favorable
/// Winds, Goblin Chieftain, Glorious Anthem), Giant Growth and Lightning Bolt to change and kill creatures, and
/// creatures for the rest.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn layers_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    for (color, count) in LANDS {
        deck.extend(std::iter::repeat_n(land(color), count));
    }
    for (name, copies) in [
        ("Nightmare", 2),
        ("Maro", 1),
        ("Transguild Courier", 1),
        ("Kormus Bell", 1),
        ("Humility", 1),
        ("Levitation", 1),
        ("Favorable Winds", 1),
        ("Goblin Chieftain", 1),
        ("Glorious Anthem", 1),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (veil_of_leaves()?, 2),
        (elven_forge()?, 2),
        (rust_hex()?, 1),
        (gear_chorus()?, 1),
        (tin_awakening()?, 1),
        (big_hearts()?, 2),
        (rally_cry()?, 2),
        (banner_of_pride()?, 1),
        (hulk()?, 2),
        (goblin_scrap(), 3),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
    deck.extend(std::iter::repeat_n(
        creature("Layer Fodder", 1, &[Color::Green], power, 2),
        deck.len().abs_diff(60),
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`layers_deck`]s, fully determined by `seed`.
pub fn layers_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(layers_deck(&mut GameRng::substream(seed, 14_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic layer cards use only what the engine runs and carry the ability they say, a layers deck is a legal sixty cards with every card a game is expected to bring into play, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    fn synthetic() -> Vec<Arc<CardDefinition>> {
        vec![
            veil_of_leaves().unwrap(),
            elven_forge().unwrap(),
            rust_hex().unwrap(),
            gear_chorus().unwrap(),
            tin_awakening().unwrap(),
            big_hearts().unwrap(),
            rally_cry().unwrap(),
            banner_of_pride().unwrap(),
            hulk().unwrap(),
        ]
    }

    #[test]
    fn every_synthetic_layer_card_uses_primitives_the_engine_runs() {
        let caps = caps();
        for card in synthetic() {
            assert_eq!(card.rules.len(), 1, "{}", card.name);
            for ability in &card.rules {
                assert!(
                    matches!(**ability, Ability::Static { .. }),
                    "{} has a static ability",
                    card.name
                );
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn a_layers_deck_is_sixty_cards_with_every_card_a_game_brings_into_play() {
        let deck = layers_deck(&mut GameRng::substream(1, 14_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in LAYER_CARDS.iter().copied().chain(["Nightmare"]) {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = layers_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = layers_fuzz_setup(3, 5).unwrap();
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
