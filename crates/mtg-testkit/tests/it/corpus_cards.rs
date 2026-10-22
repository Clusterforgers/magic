//! Goal: the 179 corpus cards reach the engine as their support records say (Verified with their abilities, Partial with the abilities the engine runs, Manual with the printed side alone), the gaps a record names are exactly the primitives the engine lacks, and the engine plays what it is given: Grizzly Bears is cast from two Forests, Llanowar Elves taps for mana, a Manual artifact resolves without its ability, a spell with X in its cost is offered when X of nothing can be paid, and whole bot games with a corpus deck keep every invariant.
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
/// Partial (Aven Mindcensor). Cascade is played, so Bloodbraid Elf is Verified, and so are the madness and suspend cards.
/// Counters on permanents and players are played, so Tireless Tracker is Verified, and so are the planeswalkers whose
/// loyalty abilities and effects run (Jace, the Mind Sculptor, Jace Beleren, Ajani, Garruk).
/// Divided damage and counters are played, so Forked Bolt and Armament Corps are Verified; cost increases and reductions are
/// played too, so Fireball, Thalia, Guardian of Thraben and Foundry Inspector are Verified.
const CARDS: [(&str, Support, usize); 179] = [
    ("Act of Treason", Verified, 1),
    ("Solstice Zealot", Verified, 2),
    ("Copper-Leaf Angel", Verified, 2),
    ("Llanowar Behemoth", Verified, 1),
    ("Lurching Rotbeast", Verified, 1),
    ("Nantuko Husk", Verified, 1),
    ("Rummaging Goblin", Verified, 1),
    ("Soul Shepherd", Verified, 1),
    ("Souldrinker", Verified, 1),
    ("Tumble Magnet", Verified, 2),
    ("Ajani, Caller of the Pride", Verified, 3),
    ("Ancient Ziggurat", Verified, 1),
    ("Anger", Verified, 2),
    ("Anointed Procession", Verified, 1),
    ("Archon of Cruelty", Verified, 2),
    ("Batterskull", Verified, 4),
    ("Battlefield Forge", Verified, 2),
    ("Black Knight", Verified, 2),
    ("Bloodbraid Elf", Verified, 2),
    ("Fiery Temper", Verified, 2),
    ("Basking Rootwalla", Verified, 2),
    ("Arrogant Wurm", Verified, 2),
    ("Rift Bolt", Verified, 2),
    ("Ancestral Vision", Verified, 2),
    ("Durkwood Baloth", Verified, 1),
    ("Bonecrusher Giant // Stomp", Manual, 0),
    ("Boneyard Scourge", Verified, 2),
    ("Armguard Familiar", Verified, 3),
    ("Bonesplitter", Verified, 2),
    ("Bronzeplate Boar", Verified, 3),
    ("Corpsejack Menace", Verified, 1),
    ("Doubling Season", Verified, 2),
    ("Faithful Watchdog", Verified, 2),
    ("Furnace of Rath", Verified, 1),
    ("Hall of Triumph", Verified, 2),
    ("Hardened Scales", Verified, 1),
    ("Idyllic Beachfront", Verified, 3),
    ("Lizard Blades", Verified, 3),
    ("Brainstorm", Verified, 1),
    ("Burst Lightning", Verified, 2),
    ("Caress of Phyrexia", Verified, 1),
    ("Carnage Tyrant", Verified, 3),
    ("Cathars' Crusade", Verified, 1),
    ("Counterspell", Verified, 1),
    ("Courtly Provocateur", Verified, 2),
    ("Covetous Dragon", Verified, 2),
    ("Cultivate", Verified, 1),
    ("Darksteel Juggernaut", Verified, 3),
    ("Death Ward", Verified, 1),
    ("Delver of Secrets // Insectile Aberration", Manual, 0),
    ("Dockside Extortionist", Verified, 1),
    ("Doom Blade", Verified, 1),
    ("Drudge Skeletons", Verified, 1),
    ("Elemental Masterpiece", Verified, 2),
    ("Eternal Witness", Verified, 1),
    ("Faithless Looting", Verified, 2),
    ("Favorable Winds", Verified, 1),
    ("Felidar Sovereign", Verified, 3),
    ("Fire // Ice", Manual, 0),
    ("Fireball", Verified, 2),
    ("Flooded Strand", Verified, 1),
    ("Foundry Inspector", Verified, 1),
    ("Fog", Verified, 1),
    ("Forest", Verified, 1),
    ("Gaea's Cradle", Verified, 1),
    ("Garruk Wildspeaker", Verified, 3),
    ("Gatekeeper of Malakir", Verified, 2),
    ("Giant Growth", Verified, 1),
    ("Glorious Anthem", Verified, 1),
    ("Goblin Chieftain", Verified, 2),
    ("Goblin Spy", Verified, 1),
    ("Grizzly Bears", Verified, 0),
    ("Hero of Bladehold", Verified, 2),
    ("Hulking Raptor", Verified, 2),
    ("Humility", Verified, 1),
    ("Inspiring Call", Verified, 1),
    ("Izzet Charm", Verified, 1),
    ("Jace Beleren", Verified, 3),
    ("Jace, the Mind Sculptor", Verified, 4),
    ("Jungle Lion", Verified, 1),
    ("Kormus Bell", Verified, 1),
    ("Levitation", Verified, 1),
    ("Lightning Bolt", Verified, 1),
    ("Llanowar Elves", Verified, 1),
    ("Lotus Petal", Verified, 1),
    ("Maralen of the Mornsong", Verified, 2),
    ("Maro", Verified, 1),
    ("Mishra's Workshop", Verified, 1),
    ("Mending Hands", Verified, 1),
    ("Kor Firewalker", Verified, 2),
    ("Mindlock Orb", Verified, 1),
    ("Mother of Runes", Verified, 1),
    ("Monastery Swiftspear", Verified, 2),
    ("Mulldrifter", Verified, 3),
    ("Naban, Dean of Iteration", Verified, 1),
    ("Nightmare", Verified, 2),
    ("Opt", Verified, 1),
    ("Pacifism", Verified, 2),
    ("Panharmonicon", Verified, 1),
    ("Parallel Lives", Verified, 1),
    ("Pillar of the Paruns", Verified, 1),
    ("Questing Beast", Verified, 6),
    ("Rally the Ranks", Verified, 2),
    ("Rampaging Ferocidon", Verified, 3),
    ("Rampant Growth", Verified, 1),
    ("Reassembling Skeleton", Verified, 1),
    ("Rest in Peace", Verified, 2),
    ("Rosheen Meanderer", Verified, 1),
    ("Rhox Faithmender", Verified, 2),
    ("Rhystic Study", Verified, 1),
    ("Samite Healer", Verified, 1),
    ("Serra Angel", Verified, 2),
    ("Sneak Attack", Verified, 1),
    ("Sodden Verdure", Verified, 3),
    ("Sol Ring", Verified, 1),
    ("Soul of Zendikar", Verified, 3),
    ("Spellbreaker Behemoth", Verified, 2),
    ("Squee, Goblin Nabob", Verified, 1),
    ("Steam Vents", Verified, 3),
    ("Supreme Verdict", Verified, 2),
    ("Swiftfoot Boots", Verified, 2),
    ("Swords to Plowshares", Verified, 1),
    ("Thalia, Guardian of Thraben", Verified, 2),
    ("Teysa Karlov", Verified, 2),
    ("Thought Reflection", Verified, 1),
    ("Tireless Tracker", Verified, 2),
    ("Transguild Courier", Verified, 1),
    ("Warmonger Hellkite", Verified, 3),
    ("Wolfbriar Elemental", Verified, 2),
    ("Wrath of God", Verified, 1),
    ("Arvad the Cursed", Verified, 3),
    ("Control Magic", Verified, 2),
    ("Epic Proportions", Verified, 3),
    ("Kongming, \"Sleeping Dragon\"", Verified, 1),
    ("Shielded by Faith", Verified, 3),
    ("Squirrel Nest", Verified, 2),
    ("Sram, Senior Edificer", Verified, 1),
    ("Blaze", Verified, 1),
    ("Stroke of Genius", Verified, 1),
    ("Kessig Wolf Run", Verified, 2),
    ("Tezzeret the Seeker", Verified, 3),
    ("Forked Bolt", Verified, 1),
    ("Armament Corps", Verified, 1),
    ("Cyclonic Rift", Verified, 2),
    ("Mizzium Mortars", Verified, 2),
    ("Omniscience", Verified, 1),
    ("Vedalken Orrery", Verified, 1),
    ("Ajani's Pridemate", Verified, 1),
    ("Thieving Magpie", Verified, 2),
    ("Underworld Dreams", Verified, 1),
    ("Megrim", Verified, 1),
    ("Manabarbs", Verified, 1),
    ("Mesmeric Orb", Verified, 1),
    ("Wanderbrine Preacher", Verified, 1),
    ("Renewed Faith", Verified, 3),
    ("Skullclamp", Verified, 3),
    ("Lat-Nam Adept", Verified, 1),
    ("Fear of Failed Tests", Verified, 1),
    ("Sanguine Bond", Verified, 1),
    ("Sword of Body and Mind", Verified, 3),
    ("The Thing, Ben Grimm", Verified, 2),
    ("Bloodghast", Verified, 3),
    ("Stoke the Flames", Verified, 2),
    ("Gurmag Angler", Verified, 1),
    ("Treasure Cruise", Verified, 2),
    ("Murderous Cut", Verified, 2),
    ("Siege Wurm", Verified, 2),
    ("Walking Ballista", Verified, 3),
    ("Hangarback Walker", Verified, 3),
    ("Gadwick, the Wizened", Verified, 2),
    ("Disembowel", Verified, 1),
    ("Forced March", Verified, 1),
    ("Invisible Stalker", Verified, 2),
    ("Aven Mindcensor", Partial, 2),
    ("Courser of Kruphix", Partial, 2),
    ("Behold the Multiverse", Verified, 2),
    ("Saw It Coming", Verified, 2),
    ("Demon Bolt", Verified, 2),
    ("Augury Raven", Verified, 2),
];

#[test]
fn the_table_names_every_corpus_card_once() {
    let in_table: BTreeSet<&str> = CARDS.iter().map(|(name, ..)| *name).collect();
    let in_corpus: BTreeSet<&str> = corpus().unwrap().names().collect();
    assert_eq!(in_table.len(), 179);
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
#[case::delver(
    "Delver of Secrets // Insectile Aberration",
    &["Effect.Transform"]
)]
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
