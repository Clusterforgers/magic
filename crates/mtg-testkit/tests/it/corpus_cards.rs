//! Goal: every corpus card reaches the engine as its support record says (Verified with its abilities, Partial with the abilities the engine runs, Manual with the printed side alone), the gaps a record names are exactly the primitives the engine lacks, and the engine plays what it is given: Grizzly Bears is cast from two Forests, Llanowar Elves taps for mana, a Manual artifact resolves without its ability, a spell with X in its cost is offered when X of nothing can be paid, and whole bot games with a corpus deck keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

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

use Support::{Partial, Verified};

/// The corpus cards the engine does not play whole, each with the state its capabilities give it and the number of
/// abilities its definition carries (mana abilities and rules together). Every other corpus card is Verified and carries
/// the abilities of its file, and a basic land type adds the mana ability of that land type. A Manual card carries none
/// (the table resolves it); a Partial card carries what the engine runs. A
/// card joins this list when the engine cannot yet play it whole and leaves it when it can.
const NOT_VERIFIED: &[(&str, Support, usize)] = &[
    ("Aven Mindcensor", Partial, 2),
    ("Courser of Kruphix", Partial, 2),
];

/// The abilities a Verified card carries: those of its file the engine runs, and the mana ability each basic land type gives.
fn abilities_of_a_verified_card(name: &str) -> usize {
    let library = mtg_cards::builtin::library().unwrap();
    let file = library
        .card_by_name(name)
        .unwrap()
        .abilities()
        .filter(|(_, ability)| !ability.is_manual())
        .count();
    let definition = card(name).unwrap();
    let land_types = if definition.types.contains(&CardType::Land) {
        definition
            .subtypes
            .iter()
            .filter(|subtype| {
                matches!(
                    subtype.0.as_str(),
                    "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"
                )
            })
            .count()
    } else {
        0
    };
    file + land_types
}

/// The state and the number of abilities a corpus card is expected to arrive with.
fn expected_state(name: &str) -> (Support, usize) {
    NOT_VERIFIED.iter().find(|row| row.0 == name).map_or_else(
        || (Verified, abilities_of_a_verified_card(name)),
        |row| (row.1, row.2),
    )
}

#[test]
fn the_cards_that_are_not_verified_are_corpus_cards_listed_once_and_none_of_them_is_verified() {
    let corpus = corpus().unwrap();
    let names: BTreeSet<&str> = NOT_VERIFIED.iter().map(|row| row.0).collect();
    assert_eq!(names.len(), NOT_VERIFIED.len(), "a card twice");
    for (name, state, _) in NOT_VERIFIED {
        assert!(corpus.support(name).is_ok(), "{name} is not a corpus card");
        assert_ne!(
            *state, Verified,
            "{name}: Verified is the default and needs no row"
        );
    }
}

#[test]
fn each_corpus_card_arrives_as_its_support_record_says() {
    let corpus = corpus().unwrap();
    let mut wrong = Vec::new();
    for name in corpus.names() {
        let record = corpus.support(name).unwrap();
        let definition = corpus.card(name).unwrap();
        // A card with several faces carries the abilities of each face; its whole is the front face or both halves.
        let carried: usize = if definition.is_multi_face() {
            definition
                .faces
                .iter()
                .map(|face| face.abilities.len() + face.rules.len())
                .sum()
        } else {
            definition.abilities.len() + definition.rules.len()
        };
        let (state, abilities) = expected_state(name);
        if (record.support, carried) != (state, abilities) {
            wrong.push(format!(
                "{name}: {:?} with {carried} abilities, wanted {state:?} with {abilities} ({:?}); a card \
                 the engine cannot play whole gets a row in NOT_VERIFIED (docs/cards-dsl.md, \"Adding a corpus card\")",
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
    for name in corpus.names() {
        let record = corpus.support(name).unwrap();
        let state = record.support;
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
    // Fireball is covered whole: its cost for the targets beyond the first, its X and the even split of its damage.
    // Thalia and Foundry Inspector change what other spells cost, which the engine plays.
    for name in [
        "Fireball",
        "Thalia, Guardian of Thraben",
        "Foundry Inspector",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // The kicker and multikicker cards are covered whole: the costs are paid at announcement and read at resolution.
    for name in [
        "Burst Lightning",
        "Gatekeeper of Malakir",
        "Wolfbriar Elemental",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Rhystic Study and Izzet Charm are covered whole: the payment the opponent or the spell's controller is offered
    // is played now, with the "may", the trigger, the modes, the counter and the damage around it.
    for name in ["Rhystic Study", "Izzet Charm"] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Mulldrifter and Faithless Looting are covered whole: evoke and flashback are alternative costs chosen with the cast.
    for name in ["Mulldrifter", "Faithless Looting"] {
        assert_eq!(reasons(name), "", "{name}");
    }
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
    // Counters are played now: Tireless Tracker makes its Clues and puts the counter on itself, Cathars' Crusade puts
    // one on each creature, Inspiring Call counts and shields the creatures that have one and Caress of Phyrexia
    // gives a player poison counters.
    for name in [
        "Tireless Tracker",
        "Cathars' Crusade",
        "Inspiring Call",
        "Caress of Phyrexia",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // Planeswalkers are played now: their loyalty abilities run with the draws, the library effects, the bounce,
    // the exile, the mill, the counters, the pump, the untap and the tokens around them.
    for name in [
        "Jace, the Mind Sculptor",
        "Jace Beleren",
        "Ajani, Caller of the Pride",
        "Garruk Wildspeaker",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // The mana abilities of several mana, of as many mana as there are creatures, of a choice of colour with damage to
    // their controller and of a sacrifice are activated at priority and paid with: the cards are covered whole.
    for name in [
        "Sol Ring",
        "Gaea's Cradle",
        "Battlefield Forge",
        "Lotus Petal",
        "Ancient Ziggurat",
        "Mishra's Workshop",
        "Pillar of the Paruns",
        "Rosheen Meanderer",
    ] {
        assert_eq!(reasons(name), "", "{name}");
    }
    // The costs of an activated ability beyond `{T}`, mana and the sacrifice of the source are announced and paid:
    // life, a permanent sacrificed or tapped, a card discarded or exiled from a graveyard, counters removed, cycling.
    for name in [
        "Flooded Strand",
        "Llanowar Behemoth",
        "Lurching Rotbeast",
        "Nantuko Husk",
        "Rummaging Goblin",
        "Soul Shepherd",
        "Souldrinker",
        "Tumble Magnet",
    ] {
        assert_eq!(reasons(name), "", "{name}");
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

/// A card that makes a predefined token needs what the abilities of that token need, so its support counts them: a
/// Treasure sacrifices itself for mana, a Clue pays mana and draws.
#[rstest]
#[case::dockside_extortionist("Dockside Extortionist", &["Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana"])]
#[case::tireless_tracker("Tireless Tracker", &["Cost.Mana", "Cost.SacrificeSelf", "Effect.Draw"])]
fn a_card_that_makes_a_predefined_token_needs_what_its_abilities_need(
    #[case] name: &str,
    #[case] needed: &[&str],
) {
    let library = mtg_cards::builtin::library().unwrap();
    let used: BTreeSet<String> = library
        .card_by_name(name)
        .into_iter()
        .flat_map(mtg_cards::card::Card::abilities)
        .flat_map(|(_, ability)| used_primitives(ability))
        .collect();
    for primitive in needed {
        assert!(
            used.contains(*primitive),
            "{name} needs {primitive}: {used:?}"
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
    // Five lands pay its mana cost and its evoke cost: it is cast for the mana cost, the first way offered.
    driver.begin_cast("Mulldrifter")?;
    driver.answer(mtg_engine::Answer::ChooseOption(
        mtg_engine::choose::OptionChoice(0),
    ))?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
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
fn sol_ring_resolves_and_is_a_mana_source_of_two() -> Result<(), ScriptError> {
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
    let counts: Vec<u32> = options.mana_abilities.iter().map(|m| m.count).collect();
    assert_eq!(
        counts.iter().sum::<u32>(),
        3,
        "the Forest that did not pay adds one mana and Sol Ring two: {counts:?}"
    );
    assert!(counts.contains(&2));
    Ok(())
}

#[test]
fn a_spell_with_x_in_its_cost_is_offered_when_x_of_nothing_can_be_paid() -> Result<(), ScriptError>
{
    let hydra = |x: u8| -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Hydra")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_mana_cost(ManaCost::parse("{G}").unwrap())
                .with_x_in_cost(x),
        )
    };
    let castable = |x: u8, forests: usize| -> Result<usize, ScriptError> {
        let driver = Driver::from_scenario(
            Scenario::new(2)
                .hand(0, [hydra(x)])
                .battlefield_named(0, vec!["Forest"; forests]),
        )?;
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority was expected");
        };
        Ok(options.castable.len())
    };
    assert_eq!(
        castable(0, 2)?,
        1,
        "the control costs {{G}} and is affordable"
    );
    assert_eq!(castable(1, 2)?, 1, "{{X}}{{G}} is cast for X = 0 or 1");
    assert_eq!(castable(2, 2)?, 1);
    assert_eq!(castable(1, 1)?, 1, "X = 0 leaves just the {{G}}");
    assert_eq!(castable(1, 0)?, 0, "the {{G}} is not paid for");
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
