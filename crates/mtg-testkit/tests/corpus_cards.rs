//! Goal: the 40 corpus cards reach the engine as their support records say (Verified with their abilities, Manual with the printed side alone), and the engine plays what it is given: Grizzly Bears is cast from two Forests, Llanowar Elves taps for mana, a Manual artifact resolves without its ability, a spell with X in its cost is never offered, and whole bot games with a corpus deck keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::names;
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::corpus::{card, corpus};
use mtg_testkit::{Driver, Scenario, ScriptError, play_game};

use Support::{Manual, Verified};

/// Every corpus card with the state the engine's capabilities give it and the number of abilities
/// its definition carries (mana abilities and rules together). The engine runs the plain mana ability
/// and spells and `{T}` abilities made of drawing, life and damage, so the four cards that need no more
/// are Verified (the Forest's ability comes from its land type).
const CARDS: [(&str, Support, usize); 40] = [
    ("Anointed Procession", Manual, 0),
    ("Archon of Cruelty", Manual, 0),
    ("Bloodbraid Elf", Manual, 0),
    ("Bonecrusher Giant // Stomp", Manual, 0),
    ("Bonesplitter", Manual, 0),
    ("Brainstorm", Manual, 0),
    ("Burst Lightning", Manual, 0),
    ("Counterspell", Manual, 0),
    ("Cultivate", Manual, 0),
    ("Delver of Secrets // Insectile Aberration", Manual, 0),
    ("Dockside Extortionist", Manual, 0),
    ("Doom Blade", Manual, 0),
    ("Eternal Witness", Manual, 0),
    ("Faithless Looting", Manual, 0),
    ("Felidar Sovereign", Manual, 0),
    ("Fire // Ice", Manual, 0),
    ("Fireball", Manual, 0),
    ("Flooded Strand", Manual, 0),
    ("Forest", Verified, 1),
    ("Gaea's Cradle", Manual, 0),
    ("Giant Growth", Manual, 0),
    ("Grizzly Bears", Verified, 0),
    ("Hero of Bladehold", Manual, 0),
    ("Izzet Charm", Manual, 0),
    ("Jace, the Mind Sculptor", Manual, 0),
    ("Lightning Bolt", Verified, 1),
    ("Llanowar Elves", Verified, 1),
    ("Mulldrifter", Manual, 0),
    ("Opt", Manual, 0),
    ("Pacifism", Manual, 0),
    ("Panharmonicon", Manual, 0),
    ("Questing Beast", Manual, 0),
    ("Rampant Growth", Manual, 0),
    ("Rhystic Study", Manual, 0),
    ("Serra Angel", Manual, 0),
    ("Sneak Attack", Manual, 0),
    ("Sol Ring", Manual, 0),
    ("Swords to Plowshares", Manual, 0),
    ("Tireless Tracker", Manual, 0),
    ("Wrath of God", Manual, 0),
];

#[test]
fn the_table_names_every_corpus_card_once() {
    let in_table: BTreeSet<&str> = CARDS.iter().map(|(name, ..)| *name).collect();
    let in_corpus: BTreeSet<&str> = corpus().unwrap().names().collect();
    assert_eq!(in_table.len(), 40);
    assert_eq!(in_table, in_corpus);
}

#[test]
fn each_corpus_card_arrives_as_its_support_record_says() {
    let corpus = corpus().unwrap();
    let mut wrong = Vec::new();
    for (name, state, abilities) in CARDS {
        let record = corpus.support(name).unwrap();
        let definition = corpus.card(name).unwrap();
        let carried = definition.abilities.len() + definition.rules.len();
        if (record.support, carried) != (state, abilities) {
            wrong.push(format!(
                "{name}: {:?} with {carried} abilities, wanted {state:?} with {abilities} ({:?})",
                record.support, record.clauses
            ));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[test]
fn grizzly_bears_is_cast_from_two_forests_and_enters_as_a_two_two() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Forest", "Forest"]),
    )?;
    driver.cast("Grizzly Bears")?;
    resolve_the_stack(&mut driver)?;
    let state = driver.state();
    assert_eq!(
        names(state, Zone::Battlefield),
        ["Forest", "Forest", "Grizzly Bears"]
    );
    let bears = driver.id_of("Grizzly Bears")?;
    let chars = mtg_engine::characteristics(state, bears).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    assert!(chars.is_creature());
    Ok(())
}

#[test]
fn llanowar_elves_taps_for_green_mana_from_its_assembled_ability() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Llanowar Elves", "Forest"]),
    )?;
    let events = driver.tap_for_mana("Llanowar Elves")?;
    let added = events.iter().any(|event| {
        matches!(
            event,
            Event::ManaAdded {
                mana: ManaType::Green,
                ..
            }
        )
    });
    assert!(added, "{events:?}");
    // The elf's mana and the Forest's pay for the Bears.
    driver.cast("Grizzly Bears")?;
    resolve_the_stack(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    Ok(())
}

#[test]
fn a_manual_artifact_resolves_without_its_ability() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Sol Ring"])
            .battlefield_named(0, ["Forest", "Forest"]),
    )?;
    driver.cast("Sol Ring")?;
    resolve_the_stack(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Sol Ring".to_owned()));
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert_eq!(
        options.mana_abilities.len(),
        1,
        "only the Forest that did not pay taps for mana: Sol Ring is left to the table"
    );
    Ok(())
}

#[test]
fn a_spell_with_x_in_its_cost_is_never_offered() -> Result<(), ScriptError> {
    let hydra = |x: u8| -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Hydra")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_mana_cost(ManaCost::parse("{G}").unwrap())
                .with_x_in_cost(x),
        )
    };
    let castable = |x: u8| -> Result<usize, ScriptError> {
        let driver = Driver::from_scenario(
            Scenario::new(2)
                .hand(0, [hydra(x)])
                .battlefield_named(0, ["Forest", "Forest"]),
        )?;
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority was expected");
        };
        Ok(options.castable.len())
    };
    assert_eq!(castable(0)?, 1, "the control costs {{G}} and is affordable");
    assert_eq!(castable(1)?, 0, "{{X}}{{G}} is not paid as {{G}}");
    assert_eq!(castable(2)?, 0);
    Ok(())
}

/// 40 Forests and four each of four cards a Forest can pay for (two of them Verified, two Manual)
/// and Fireball, which needs red mana and X and so stays dead in the draw: 60 in all.
fn corpus_deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = (0..40).map(|_| card("Forest").unwrap()).collect();
    for name in [
        "Grizzly Bears",
        "Llanowar Elves",
        "Giant Growth",
        "Sol Ring",
        "Fireball",
    ] {
        deck.extend((0..4).map(|_| card(name).unwrap()));
    }
    deck
}

#[test]
fn bot_games_with_corpus_cards_finish_and_keep_every_invariant() {
    assert_eq!(corpus_deck().len(), 60);
    for seed in 0..40 {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![corpus_deck(), corpus_deck()]);
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
    }
}
