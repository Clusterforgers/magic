//! Goal: the 72 corpus cards reach the engine as their support records say (Verified with their abilities, Partial with the abilities the engine runs, Manual with the printed side alone), the gaps a record names are exactly the primitives the engine lacks, and the engine plays what it is given: Grizzly Bears is cast from two Forests, Llanowar Elves taps for mana, a Manual artifact resolves without its ability, a spell with X in its cost is never offered, and whole bot games with a corpus deck keep every invariant.
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
use rstest::rstest;

use Support::{Manual, Partial, Verified};

/// Every corpus card with the state the engine's capabilities give it and the number of abilities
/// its definition carries (mana abilities and rules together). The engine runs the plain mana ability, spells,
/// `{T}` abilities and triggered abilities made of drawing, discarding, sacrificing, destroying, exiling,
/// countering, scrying, looking at, revealing and searching for cards, choosing and moving them (onto the battlefield
/// included), shuffling, life and damage, paying mana as an effect, creating tokens, adding mana, conditions, tapping
/// and untapping, winning the game, continuous effects and static abilities of every modification (types, subtypes, colours,
/// power and toughness, keywords, abilities, control and attack and block restrictions), characteristic-defining abilities
/// (power and toughness from a count, colours) and the keywords of combat and
/// flash, hexproof and the keywords that are triggers (battle cry, prowess, ward), so the fifty cards that need no more are Verified (the
/// Forest's ability comes from its land type) and a card whose other abilities are still missing carries what runs as
/// Partial: Mulldrifter's draw and flying, not its evoke; Faithless Looting's draw and discard, not its flashback;
/// Tireless Tracker's Clue, not its counter. Jace, the Mind Sculptor stays
/// with the table for its loyalty costs and Flooded Strand for the life it costs.
const CARDS: [(&str, Support, usize); 72] = [
    ("Act of Treason", Verified, 1),
    ("Anger", Verified, 2),
    ("Anointed Procession", Manual, 0),
    ("Archon of Cruelty", Verified, 2),
    ("Bloodbraid Elf", Partial, 1),
    ("Bonecrusher Giant // Stomp", Manual, 0),
    ("Boneyard Scourge", Verified, 2),
    ("Bonesplitter", Partial, 1),
    ("Brainstorm", Verified, 1),
    ("Burst Lightning", Manual, 0),
    ("Carnage Tyrant", Verified, 3),
    ("Counterspell", Verified, 1),
    ("Courtly Provocateur", Verified, 2),
    ("Covetous Dragon", Verified, 2),
    ("Cultivate", Verified, 1),
    ("Darksteel Juggernaut", Verified, 3),
    ("Delver of Secrets // Insectile Aberration", Manual, 0),
    ("Dockside Extortionist", Verified, 1),
    ("Doom Blade", Verified, 1),
    ("Elemental Masterpiece", Verified, 2),
    ("Eternal Witness", Verified, 1),
    ("Faithless Looting", Partial, 1),
    ("Favorable Winds", Verified, 1),
    ("Felidar Sovereign", Verified, 3),
    ("Fire // Ice", Manual, 0),
    ("Fireball", Manual, 0),
    ("Flooded Strand", Manual, 0),
    ("Forest", Verified, 1),
    ("Gaea's Cradle", Manual, 0),
    ("Giant Growth", Verified, 1),
    ("Glorious Anthem", Verified, 1),
    ("Goblin Chieftain", Verified, 2),
    ("Goblin Spy", Verified, 1),
    ("Grizzly Bears", Verified, 0),
    ("Hero of Bladehold", Verified, 2),
    ("Hulking Raptor", Verified, 2),
    ("Humility", Verified, 1),
    ("Izzet Charm", Verified, 1),
    ("Jace, the Mind Sculptor", Manual, 0),
    ("Jungle Lion", Verified, 1),
    ("Kormus Bell", Verified, 1),
    ("Levitation", Verified, 1),
    ("Lightning Bolt", Verified, 1),
    ("Llanowar Elves", Verified, 1),
    ("Maralen of the Mornsong", Verified, 2),
    ("Maro", Verified, 1),
    ("Mindlock Orb", Verified, 1),
    ("Monastery Swiftspear", Verified, 2),
    ("Mulldrifter", Partial, 2),
    ("Naban, Dean of Iteration", Verified, 1),
    ("Nightmare", Verified, 2),
    ("Opt", Verified, 1),
    ("Pacifism", Partial, 1),
    ("Panharmonicon", Verified, 1),
    ("Questing Beast", Partial, 3),
    ("Rampaging Ferocidon", Verified, 3),
    ("Rampant Growth", Verified, 1),
    ("Reassembling Skeleton", Verified, 1),
    ("Rhystic Study", Verified, 1),
    ("Serra Angel", Verified, 2),
    ("Sneak Attack", Verified, 1),
    ("Sol Ring", Manual, 0),
    ("Soul of Zendikar", Verified, 3),
    ("Spellbreaker Behemoth", Verified, 2),
    ("Squee, Goblin Nabob", Verified, 1),
    ("Supreme Verdict", Verified, 2),
    ("Swords to Plowshares", Verified, 1),
    ("Teysa Karlov", Verified, 2),
    ("Tireless Tracker", Partial, 1),
    ("Transguild Courier", Verified, 1),
    ("Warmonger Hellkite", Verified, 3),
    ("Wrath of God", Verified, 1),
];

#[test]
fn the_table_names_every_corpus_card_once() {
    let in_table: BTreeSet<&str> = CARDS.iter().map(|(name, ..)| *name).collect();
    let in_corpus: BTreeSet<&str> = corpus().unwrap().names().collect();
    assert_eq!(in_table.len(), 72);
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
    // Rhystic Study and Izzet Charm are covered whole: the payment the opponent or the spell's controller is offered
    // is played now, with the "may", the trigger, the modes, the counter and the damage around it.
    for name in ["Rhystic Study", "Izzet Charm"] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Mulldrifter's draw and flying are covered, its evoke is not.
    let mulldrifter = reasons("Mulldrifter");
    assert!(mulldrifter.contains("Keyword.Evoke"), "{mulldrifter}");
    assert!(
        !mulldrifter.contains("Keyword.Flying") && !mulldrifter.contains("Effect.Draw"),
        "{mulldrifter}"
    );
    // Faithless Looting's draw and discard are covered, its flashback is not.
    let looting = reasons("Faithless Looting");
    assert!(looting.contains("Keyword.Flashback"), "{looting}");
    assert!(
        !looting.contains("Effect.Draw") && !looting.contains("Effect.Discard"),
        "{looting}"
    );
    // Archon of Cruelty is covered whole: its sacrifice and discard are played now.
    assert_eq!(reasons("Archon of Cruelty"), "");
    // The removal and the counterspell are covered whole: destroy, exile and counter are played now.
    for name in [
        "Doom Blade",
        "Swords to Plowshares",
        "Counterspell",
        "Wrath of God",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Dockside Extortionist is covered whole: its Treasures are made and their mana ability runs.
    assert_eq!(reasons("Dockside Extortionist"), "");
    // Hero of Bladehold, Monastery Swiftspear and Hulking Raptor are covered whole: the keywords that are triggers
    // (battle cry, prowess, ward) run beside the tokens, the haste and the mana.
    for name in [
        "Hero of Bladehold",
        "Monastery Swiftspear",
        "Hulking Raptor",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Tireless Tracker makes its Clues; putting the counter on itself is not played yet.
    let tracker = reasons("Tireless Tracker");
    assert!(tracker.contains("Effect.PutCounters"), "{tracker}");
    assert!(
        !tracker.contains("Effect.CreateToken") && !tracker.contains("Trigger.Sacrificed"),
        "{tracker}"
    );
    // Sol Ring and Gaea's Cradle add mana from an activated ability of a shape the engine does not activate yet:
    // two mana at once, and as many as there are creatures.
    for name in ["Sol Ring", "Gaea's Cradle"] {
        let reason = reasons(name);
        assert!(
            reason.contains("Ability.ManaAbility.Complex"),
            "{name}: {reason}"
        );
        assert!(!reason.contains("Effect.AddMana"), "{name}: {reason}");
    }
    // The rules of the game run as static abilities and as effects: the cards that change one are covered whole.
    for name in [
        "Carnage Tyrant",
        "Courtly Provocateur",
        "Darksteel Juggernaut",
        "Goblin Spy",
        "Maralen of the Mornsong",
        "Mindlock Orb",
        "Naban, Dean of Iteration",
        "Panharmonicon",
        "Rampaging Ferocidon",
        "Spellbreaker Behemoth",
        "Supreme Verdict",
        "Teysa Karlov",
        "Warmonger Hellkite",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
}

/// The primitives of the card's rules the engine does not run.
fn lacking(name: &str) -> Vec<String> {
    let caps = mtg_engine::caps();
    let library = mtg_cards::builtin::library().unwrap();
    let missing: BTreeSet<String> = library
        .card_by_name(name)
        .into_iter()
        .flat_map(mtg_cards::card::Card::abilities)
        .filter(|(_, ability)| !ability.is_manual())
        .flat_map(|(_, ability)| caps.missing(&used_primitives(ability)))
        .collect();
    missing.into_iter().collect()
}

/// A "may", a choice, modes, a payment, a search and the library effects run now, so the corpus cards that use only
/// those are Verified (Brainstorm, Cultivate, Eternal Witness, Izzet Charm, Opt, Rampant Growth, Rhystic Study); each of
/// the others here is still held back by another primitive, which this table names.
#[rstest]
#[case::flooded_strand("Flooded Strand", &["Cost.PayLife"])]
#[case::delver(
    "Delver of Secrets // Insectile Aberration",
    &["Effect.Transform"]
)]
#[case::jace("Jace, the Mind Sculptor", &["Ability.Loyalty"])]
fn the_cards_with_a_may_a_choice_or_modes_still_lack_other_primitives(
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(lacking(name), expected, "{name}");
    let corpus = corpus().unwrap();
    let record = corpus.support(name).unwrap();
    let (state, _) = CARDS
        .iter()
        .find_map(|(card, state, abilities)| (*card == name).then_some((*state, *abilities)))
        .unwrap();
    assert_eq!(record.support, state, "{name}");
    assert_ne!(record.support, Verified, "{name}");
    for runs in ["Effect.May", "Effect.Choose", "Effect.Modal"] {
        assert!(
            !lacking(name).iter().any(|gap| gap == runs),
            "{name} is not held back by {runs}"
        );
    }
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

/// 40 Forests and four each of four cards a Forest can pay for (three of them Verified, one Manual)
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
