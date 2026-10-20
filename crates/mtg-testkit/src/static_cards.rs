//! Synthetic static abilities (CR 604) and the decks and games built from them: a lord of a type another card gives,
//! a switch of power and toughness, an anthem that works only below a life total, a loss of flying, abilities given to
//! every creature (one that triggers as it dies, one that gains life), a creature that cannot attack and a hold on
//! every small creature, beside the corpus anthem, Levitation, Humility, Goblin Chieftain, Favorable Winds and Jungle Lion.

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

/// Lands in a statics deck: six of each of four colours.
const LANDS: usize = 24;

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A permanent of `kind` whose only ability is the one `json` says.
fn permanent(
    name: &str,
    kind: CardType,
    generic: u32,
    colors: &[Color],
    json: &str,
) -> Result<CardDefinition, CorpusError> {
    Ok(CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(cost(generic, colors))
        .with_rules(rules(json)?))
}

/// What a static ability of "creatures you control" names.
const MINE: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

fn static_over(affected: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{affected},"mods":{mods}}}}}"#)
}

/// `{2}` artifact: creatures you control have "When this creature dies, each opponent loses 1 life."
pub fn dread_banner() -> Result<Arc<CardDefinition>, CorpusError> {
    let granted = r#"{"GrantAbility":{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"LoseLife":{"who":"Opponents","amount":1}}]}}}"#;
    Ok(Arc::new(permanent(
        "Dread Banner",
        CardType::Artifact,
        2,
        &[],
        &static_over(MINE, &format!("[{granted}]")),
    )?))
}

/// `{1}{U}` enchantment: creatures you control have their power and toughness switched.
pub fn mirror_charm() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(permanent(
        "Mirror Charm",
        CardType::Enchantment,
        1,
        &[Color::Blue],
        &static_over(MINE, r#"["SwitchPT"]"#),
    )?))
}

/// `{1}{G}` enchantment: creatures you control are artifacts and Elves.
pub fn chameleon_veil() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(permanent(
        "Chameleon Veil",
        CardType::Enchantment,
        1,
        &[Color::Green],
        &static_over(MINE, r#"[{"AddType":"Artifact"},{"AddSubtype":"Elf"}]"#),
    )?))
}

/// `{1}{G}` 2/2 creature: other Elves get +1/+1.
pub fn elf_marshal() -> Result<Arc<CardDefinition>, CorpusError> {
    let lord = static_over(
        r#"{"Each":[{"Subtype":"Elf"},"Other"]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":1}}]"#,
    );
    Ok(Arc::new(
        CardDefinition::new("Elf Marshal")
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_power_toughness(2, 2)
            .with_rules(rules(&lord)?),
    ))
}

/// `{2}{R}` enchantment: creatures your opponents control lose flying.
pub fn gale_anchor() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(permanent(
        "Gale Anchor",
        CardType::Enchantment,
        2,
        &[Color::Red],
        &static_over(
            r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#,
            r#"[{"RemoveKeyword":"Flying"}]"#,
        ),
    )?))
}

/// `{2}{R}` enchantment: as long as you have 10 life or less, creatures you control get +2/+2.
pub fn desperate_anthem() -> Result<Arc<CardDefinition>, CorpusError> {
    let json = format!(
        r#"{{"Static":{{"affected":{MINE},"mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}}],"condition":{{"Compare":{{"left":{{"Life":"You"}},"op":"Le","right":10}}}}}}}}"#
    );
    Ok(Arc::new(permanent(
        "Desperate Anthem",
        CardType::Enchantment,
        2,
        &[Color::Red],
        &json,
    )?))
}

/// `{3}` artifact: creatures you control have "{T}: You gain 1 life."
pub fn lifegiver_totem() -> Result<Arc<CardDefinition>, CorpusError> {
    let granted =
        r#"{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}}"#;
    Ok(Arc::new(permanent(
        "Lifegiver Totem",
        CardType::Artifact,
        3,
        &[],
        &static_over(MINE, &format!("[{granted}]")),
    )?))
}

/// `{2}{U}` enchantment: you control each creature with power 2 or less.
pub fn thiefs_hold() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(permanent(
        "Thief's Hold",
        CardType::Enchantment,
        2,
        &[Color::Blue],
        &static_over(
            r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}"#,
            r#"[{"SetController":"You"}]"#,
        ),
    )?))
}

/// `{1}{R}` 3/1 creature that can't attack.
pub fn sloth_lord() -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new("Sloth Lord")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_power_toughness(3, 1)
            .with_rules(rules(&static_over(r#""This""#, r#"["CantAttack"]"#))?),
    ))
}

/// The names of the cards that have a static ability, which a game uses when they enter the battlefield.
pub const STATIC_CARDS: [&str; 15] = [
    "Glorious Anthem",
    "Levitation",
    "Goblin Chieftain",
    "Humility",
    "Favorable Winds",
    "Jungle Lion",
    "Dread Banner",
    "Mirror Charm",
    "Chameleon Veil",
    "Elf Marshal",
    "Gale Anchor",
    "Desperate Anthem",
    "Lifegiver Totem",
    "Thief's Hold",
    "Sloth Lord",
];

/// A legal Normal deck in four colours of static abilities: the corpus cards that have them, the synthetic ones above,
/// Giant Growth and Lightning Bolt to change and kill creatures, and a creature or two for the rest.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn statics_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Green, Color::Blue, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 4])).collect();
    for (name, copies) in [
        ("Glorious Anthem", 2),
        ("Levitation", 2),
        ("Goblin Chieftain", 2),
        ("Humility", 2),
        ("Favorable Winds", 2),
        ("Jungle Lion", 2),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (dread_banner()?, 2),
        (mirror_charm()?, 2),
        (chameleon_veil()?, 2),
        (elf_marshal()?, 2),
        (gale_anchor()?, 1),
        (desperate_anthem()?, 2),
        (lifegiver_totem()?, 2),
        (thiefs_hold()?, 2),
        (sloth_lord()?, 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
    deck.extend(std::iter::repeat_n(
        creature("Static Fodder", 1, &[Color::Green], power, 2),
        1,
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`statics_deck`]s, fully determined by `seed`.
pub fn statics_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(statics_deck(&mut GameRng::substream(seed, 13_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic static abilities use only what the engine runs and carry the ability they say, a statics deck is a legal sixty cards with every card that has a static ability in it, and the games built from one are fixed by their seed.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    fn synthetic() -> Vec<Arc<CardDefinition>> {
        vec![
            dread_banner().unwrap(),
            mirror_charm().unwrap(),
            chameleon_veil().unwrap(),
            elf_marshal().unwrap(),
            gale_anchor().unwrap(),
            desperate_anthem().unwrap(),
            lifegiver_totem().unwrap(),
            thiefs_hold().unwrap(),
            sloth_lord().unwrap(),
        ]
    }

    #[test]
    fn every_synthetic_static_ability_uses_primitives_the_engine_runs() {
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
    fn a_statics_deck_is_sixty_cards_with_every_card_that_has_a_static_ability() {
        let deck = statics_deck(&mut GameRng::substream(1, 13_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in STATIC_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = statics_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = statics_fuzz_setup(3, 5).unwrap();
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
