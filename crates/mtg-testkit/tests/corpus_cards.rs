//! Goal: the 40 corpus cards reach the engine as their support records say (Verified with their abilities, Partial with the abilities the engine runs, Manual with the printed side alone), the gaps a record names are exactly the primitives the engine lacks, and the engine plays what it is given: Grizzly Bears is cast from two Forests, Llanowar Elves taps for mana, a Manual artifact resolves without its ability, a spell with X in its cost is never offered, and whole bot games with a corpus deck keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::names;
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_cards::validate::used_primitives;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::{card, corpus};
use mtg_testkit::{Driver, Scenario, ScriptError, play_game, ready};

use Support::{Manual, Partial, Verified};

/// Every corpus card with the state the engine's capabilities give it and the number of abilities
/// its definition carries (mana abilities and rules together). The engine runs the plain mana ability, spells,
/// `{T}` abilities and triggered abilities made of drawing, life and damage, so the four cards that need no more
/// are Verified (the Forest's ability comes from its land type). Mulldrifter is Partial: its draw runs, its flying
/// and evoke do not.
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
    ("Mulldrifter", Partial, 1),
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

/// The primitives a clause names as missing: the reason reads "the engine does not run A, B yet".
fn gaps_named(reason: &str) -> Option<Vec<String>> {
    let names = reason
        .strip_prefix("the engine does not run ")?
        .strip_suffix(" yet")?;
    Some(names.split(", ").map(str::to_owned).collect())
}

#[test]
fn the_gaps_a_record_names_are_the_primitives_the_engine_lacks_and_a_verified_card_has_none() {
    let caps = mtg_engine::caps();
    let library = mtg_cards::builtin::library().unwrap();
    let corpus = corpus().unwrap();
    for (name, state, _) in CARDS {
        let record = corpus.support(name).unwrap();
        // Worked out here from the engine's capabilities and the file alone.
        let lacking: BTreeSet<String> = library
            .card_by_name(name)
            .into_iter()
            .flat_map(mtg_cards::card::Card::abilities)
            .filter(|(_, ability)| !ability.is_manual())
            .flat_map(|(_, ability)| caps.missing(&used_primitives(ability)))
            .collect();
        let named: BTreeSet<String> = record
            .clauses
            .iter()
            .filter_map(|clause| gaps_named(&clause.reason))
            .flatten()
            .collect();
        // A card with a printed gap (a second face, X in its cost) is left to the table before its rules are read.
        if named.is_empty() {
            assert!(
                state != Verified || (lacking.is_empty() && record.clauses.is_empty()),
                "{name}: {lacking:?} {:?}",
                record.clauses
            );
        } else {
            assert_eq!(named, lacking, "{name}: {:?}", record.clauses);
        }
    }
}

#[test]
fn what_stands_between_the_other_cards_and_verified_is_named() {
    let corpus = corpus().unwrap();
    let reasons = |name: &str| -> String {
        let record = corpus.support(name).unwrap();
        let reasons: Vec<&str> = record.clauses.iter().map(|c| c.reason.as_str()).collect();
        reasons.join("; ")
    };
    // Fireball is left to the table because the engine cannot pay X.
    let fireball = reasons("Fireball");
    assert!(fireball.contains("{X}"), "{fireball}");
    // Burst Lightning's damage is covered, its kicker is not.
    let burst = reasons("Burst Lightning");
    assert!(burst.contains("Keyword.Kicker"), "{burst}");
    assert!(!burst.contains("Effect.DealDamage"), "{burst}");
    // Rhystic Study's trigger is there; the payment and the choice it ends in are not.
    let rhystic = reasons("Rhystic Study");
    assert!(
        rhystic.contains("Effect.Pay") && rhystic.contains("Effect.May"),
        "{rhystic}"
    );
    assert!(
        !rhystic.contains("Trigger.SpellCast") && !rhystic.contains("Ability.Triggered"),
        "{rhystic}"
    );
    // Mulldrifter's draw is covered, its flying and evoke are not.
    let mulldrifter = reasons("Mulldrifter");
    assert!(
        mulldrifter.contains("Keyword.Flying") && mulldrifter.contains("Keyword.Evoke"),
        "{mulldrifter}"
    );
    assert!(!mulldrifter.contains("Effect.Draw"), "{mulldrifter}");
    // A card whose file models nothing says nothing about the engine.
    assert_eq!(reasons("Panharmonicon"), "");
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
fn mulldrifter_draws_two_cards_when_it_enters_from_its_assembled_ability() -> Result<(), ScriptError>
{
    let lands = [
        ready(island()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
    ];
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Mulldrifter"])
            .battlefield(0, lands),
    )?;
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).len();
    driver.cast("Mulldrifter")?;
    resolve_the_stack(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Mulldrifter".to_owned()));
    assert_eq!(
        driver.state().zone(Zone::Hand(PlayerId(0))).len(),
        hand - 1 + 2,
        "the creature left the hand and its trigger drew two cards"
    );
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
