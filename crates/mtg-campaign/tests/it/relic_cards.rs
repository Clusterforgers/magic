//! Goal: the relic pool holds together and plays: every relic is tagged by what its rules care about, risky relics name
//! no side and take no life, each role is well filled, a relic of a newer generation is marked so, the relics' words
//! happen in real games (costs, statics, triggers) and every relic sits in the command zones of a party of two to four
//! and plays a fight with the engine's invariants checked.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_campaign::relics::{FILTER_FROM, RARITY_FROM, Want};
use mtg_campaign::run::{CONTENT, CROWN_CASCADE_FROM};
use mtg_campaign::{CardKind, FightPlan, HeroDeck, Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::{cost, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::harness::{FailureKind, PlayOptions, play_brains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The words of a color letter as the card DSL names it.
fn color_name(letter: char) -> &'static str {
    match letter {
        'W' => "White",
        'U' => "Blue",
        'B' => "Black",
        'R' => "Red",
        _ => "Green",
    }
}

#[test]
fn each_relic_wants_what_its_rules_name() {
    let content = content().unwrap();
    for relic in content.relics() {
        let rules = serde_json::to_string(&relic.abilities).unwrap();
        for want in &relic.wants {
            let mark = match want {
                Want::Color(color) => format!("\"Color\":\"{}\"", color_name(*color)),
                Want::Type(card_type) => format!("\"Type\":\"{card_type:?}\""),
                Want::Subtype(subtype) => format!("\"Subtype\":\"{subtype}\""),
                Want::Tokens => "\"Token".to_owned(),
                Want::Counters => "\"+1/+1\"".to_owned(),
                Want::Graveyard => "\"Graveyard\"".to_owned(),
                Want::CreatureCommander => "\"Commander\"".to_owned(),
            };
            assert!(
                rules.contains(&mark),
                "{} wants {want:?}: {rules}",
                relic.id
            );
        }
        // A colour its rules name for the hero's own spells is a colour it wants.
        for letter in ['W', 'U', 'B', 'R', 'G'] {
            let named = rules.contains(&format!("\"Color\":\"{}\"", color_name(letter)));
            assert_eq!(
                named,
                relic.wants.contains(&Want::Color(letter)),
                "{}: {letter}",
                relic.id
            );
        }
    }
}

#[test]
fn a_risky_relic_names_no_side_and_takes_no_life() {
    let content = content().unwrap();
    let risky: Vec<_> = content
        .relics()
        .into_iter()
        .filter(|c| c.role == "Risky")
        .collect();
    assert!(risky.len() >= 6, "{} risky relics", risky.len());
    for relic in risky {
        let rules = serde_json::to_string(&relic.abilities).unwrap();
        for word in [
            "ControlledBy",
            "OwnedBy",
            "LoseLife",
            "DealDamage",
            "PayLife",
            "\"You\"",
            "Opponents",
        ] {
            assert!(!rules.contains(word), "{} names {word}: {rules}", relic.id);
        }
        assert!(relic.type_line.contains("Risky"), "{}", relic.id);
    }
}

#[test]
fn no_relic_takes_life_from_the_party() {
    let content = content().unwrap();
    for relic in content.relics() {
        let rules = serde_json::to_string(&relic.abilities).unwrap();
        // A life loss of a relic hits only the opponents of its holder.
        for part in rules.split("\"LoseLife\"").skip(1) {
            assert!(
                part.starts_with(":{\"who\":\"Opponents\""),
                "{}: {part}",
                relic.id
            );
        }
    }
}

#[test]
fn the_pool_covers_every_role_and_what_decks_care_about() {
    let content = content().unwrap();
    let relics = content.relics();
    assert!(relics.len() >= 50, "{} relics", relics.len());
    let mut roles: BTreeMap<&str, usize> = BTreeMap::new();
    for relic in &relics {
        *roles.entry(relic.role.as_str()).or_default() += 1;
    }
    for role in ["Healer", "Removal", "Damage", "Support"] {
        assert!(roles[role] >= 8, "{role}: {roles:?}");
    }
    assert!(roles["Risky"] >= 6, "{roles:?}");
    // What the pool covers: each colour, creatures, artifacts, enchantments, instants and sorceries, lands, tokens,
    // counters, the graveyard and the commander; and relics that fit any deck (life, cards, mana).
    let wanted = |want: &Want| relics.iter().filter(|c| c.wants.contains(want)).count();
    for letter in ['W', 'U', 'B', 'R', 'G'] {
        assert!(wanted(&Want::Color(letter)) >= 2, "{letter}");
    }
    for card_type in [
        CardType::Creature,
        CardType::Artifact,
        CardType::Enchantment,
        CardType::Instant,
        CardType::Sorcery,
        CardType::Land,
    ] {
        assert!(wanted(&Want::Type(card_type)) >= 2, "{card_type:?}");
    }
    for want in [Want::Tokens, Want::Counters, Want::Graveyard] {
        assert!(wanted(&want) >= 1, "{want:?}");
    }
    assert!(wanted(&Want::CreatureCommander) >= 1);
    assert!(relics.iter().filter(|c| c.wants.is_empty()).count() >= 10);
}

#[test]
fn the_larger_pool_belongs_to_the_filters_generation() {
    let content = content().unwrap();
    let older: Vec<&str> = content
        .relics()
        .into_iter()
        .filter(|c| c.since == 0)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(older.len(), 20, "the first twenty relics keep generation 0");
    for relic in content.relics() {
        assert!(relic.since <= CONTENT, "{}", relic.id);
        if relic.since > 0 {
            assert!(
                [FILTER_FROM, RARITY_FROM, CROWN_CASCADE_FROM].contains(&relic.since),
                "{}",
                relic.id
            );
        }
    }
    for card in content.cards.values() {
        if card.kind != CardKind::Relic {
            assert_eq!(card.since, 0, "{}", card.id);
            assert!(card.wants.is_empty(), "{}", card.id);
        }
    }
}

/// The hero (seat 0, a Bear and a land of each colour) against the boss (seat 1, a Brute and a Mountain) with the relics
/// `ids` in the hero's command zone.
fn table(ids: &[&str]) -> Scenario {
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(
            HERO,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(plains()),
                ready(island()),
                ready(swamp()),
                ready(mountain()),
                ready(forest()),
            ],
        )
        .battlefield(
            BOSS,
            [ready(free_creature("Brute", 3, 3)), ready(mountain())],
        )
        .library(HERO, (0..40).map(|_| mountain()))
        .library(BOSS, (0..40).map(|_| mountain()))
        .emblems(HERO, ids.iter().map(|id| custom(id)))
}

/// A nonland card of one type with a cost of `generic` and the `colored` mana.
fn card_of(
    name: &str,
    card_type: CardType,
    generic: u32,
    colored: &[Color],
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(card_type)
            .with_mana_cost(cost(generic, colored)),
    )
}

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A creature's power, toughness, haste and menace.
type Stats = (i32, i32, bool, bool);

fn stats(driver: &Driver, name: &str) -> Result<Stats, ScriptError> {
    let id = driver.id_of(name)?;
    let state = driver.state();
    let c = characteristics(state, id).unwrap();
    Ok((
        c.power.unwrap_or(0),
        c.toughness.unwrap_or(0),
        has_keyword(state, id, &Keyword::Haste),
        has_keyword(state, id, &Keyword::Menace),
    ))
}

/// A relic in the hero's command zone and what it makes of the hero's Bear and the boss's Brute.
#[rstest]
#[case::giants_ring_grows_all("giants_ring", (3, 3, false, false), (4, 4, false, false))]
#[case::hunting_horn_menaces_all("hunting_horn", (2, 2, false, true), (3, 3, false, true))]
#[case::the_spur_hastens_the_heroes("chargers_spur", (2, 2, true, false), (3, 3, false, false))]
#[case::ember_idol_is_the_heroes("ember_idol", (3, 2, false, false), (3, 3, false, false))]
#[case::the_cloak_is_the_heroes("trollhide_cloak", (2, 3, false, false), (3, 3, false, false))]
#[case::frenzy_is_for_all("battle_frenzy", (4, 2, false, false), (5, 3, false, false))]
#[case::the_hex_weakens_the_opponents("withering_hex", (2, 2, false, false), (2, 3, false, false))]
fn a_static_relic_changes_the_creatures_it_names(
    #[case] id: &str,
    #[case] bear: Stats,
    #[case] brute: Stats,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(&[id]))?;
    assert_eq!(stats(&driver, "Bear")?, bear, "{id}");
    assert_eq!(stats(&driver, "Brute")?, brute, "{id}");
    Ok(())
}

/// A spell of one type costing {1}{R} (the artifact {2}), cast by tapping one Mountain: castable with the relic, not
/// without it.
#[rstest]
#[case::artifacts("artisans_loupe", CardType::Artifact, 2, &[])]
#[case::enchantments("scribes_lens", CardType::Enchantment, 1, &[Color::Red])]
#[case::instants("spellbinders_ring", CardType::Instant, 1, &[Color::Red])]
#[case::sorceries("spellbinders_ring", CardType::Sorcery, 1, &[Color::Red])]
#[case::every_spell("spendthrifts_purse", CardType::Creature, 1, &[Color::Red])]
#[case::red_spells("ruby_focus", CardType::Creature, 1, &[Color::Red])]
#[case::risky_red_spells("wildfire_pact", CardType::Creature, 2, &[Color::Red])]
fn a_cost_relic_makes_its_spells_cheaper(
    #[case] id: &str,
    #[case] card_type: CardType,
    #[case] generic: u32,
    #[case] colored: &[Color],
) -> Result<(), ScriptError> {
    let spell = || card_of("Spell", card_type, generic, colored);
    let lands: &[&str] = &["Mountain"];
    let mut cheaper = Driver::from_scenario(table(&[id]).hand(HERO, [spell()]))?;
    cheaper.pass_until(Step::Main1)?;
    let cast = cheaper.cast_tapping("Spell", lands);
    assert!(cast.is_ok(), "{id}: {cast:?}");
    let mut full = Driver::from_scenario(table(&[]).hand(HERO, [spell()]))?;
    full.pass_until(Step::Main1)?;
    assert!(full.cast_tapping("Spell", lands).is_err(), "{id}");
    Ok(())
}

/// A hero's action that sets a relic's trigger off, and the life each side has after it resolves.
#[derive(Debug, Clone, Copy)]
enum Act {
    /// Casts a spell of the type and colour, tapping these lands.
    Cast(CardType, Color, &'static [&'static str]),
    /// Plays a Mountain from hand.
    Land,
}

#[rstest]
#[case::white_spells_heal("sun_idol", Act::Cast(CardType::Creature, Color::White, &["Plains"]), 21, 40)]
#[case::red_spells_burn("flame_brand", Act::Cast(CardType::Creature, Color::Red, &["Mountain"]), 20, 39)]
#[case::other_colours_do_nothing("flame_brand", Act::Cast(CardType::Creature, Color::White, &["Plains"]), 20, 40)]
#[case::creatures_entering_heal("soul_vessel", Act::Cast(CardType::Creature, Color::Green, &["Forest"]), 21, 40)]
#[case::artifacts_entering_heal("tinkers_apron", Act::Cast(CardType::Artifact, Color::Green, &["Forest", "Mountain"]), 22, 40)]
#[case::enchantments_entering_heal("warding_script", Act::Cast(CardType::Enchantment, Color::Green, &["Forest"]), 22, 40)]
#[case::lands_entering_heal("hearth_seed", Act::Land, 21, 40)]
#[case::lands_entering_burn("quarry_hammer", Act::Land, 20, 39)]
fn a_trigger_relic_works_when_its_event_happens(
    #[case] id: &str,
    #[case] act: Act,
    #[case] hero: i32,
    #[case] boss: i32,
) -> Result<(), ScriptError> {
    let scenario = match act {
        Act::Cast(card_type, color, _) => {
            // The artifact costs {1} more than its colour: {1}{G}; a creature or enchantment {G} or the colour.
            let generic = u32::from(card_type == CardType::Artifact);
            table(&[id]).hand(HERO, [card_of("Spell", card_type, generic, &[color])])
        }
        Act::Land => table(&[id]).hand(HERO, [mountain()]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    match act {
        Act::Cast(_, _, lands) => {
            driver.cast_tapping("Spell", lands)?;
        }
        Act::Land => {
            driver.play_land("Mountain")?;
        }
    }
    settle(&mut driver)?;
    assert_eq!(
        (life(&driver, HERO), life(&driver, BOSS)),
        (hero, boss),
        "{id}"
    );
    Ok(())
}

#[rstest]
fn the_charm_taxes_the_opponents_spells(
    #[values(true, false)] charm: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if charm { &["thorn_charm"] } else { &[] };
    let scenario = table(relics).hand(BOSS, [card_of("Imp", CardType::Creature, 0, &[Color::Red])]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    // The boss's one Mountain pays for the Imp, unless the charm adds {1}.
    assert_eq!(driver.cast_tapping("Imp", &["Mountain"]).is_ok(), !charm);
    Ok(())
}

#[rstest]
fn the_geode_adds_its_mana_to_the_precombat_main_phase(
    #[values(true, false)] geode: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if geode { &["crystal_geode"] } else { &[] };
    let scenario = Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(HERO, [ready(free_creature("Bear", 2, 2))])
        .battlefield(BOSS, [ready(mountain())])
        .library(HERO, (0..40).map(|_| mountain()))
        .library(BOSS, (0..40).map(|_| mountain()))
        .hand(HERO, [card_of("Trinket", CardType::Artifact, 1, &[])])
        .emblems(HERO, relics.iter().map(|id| custom(id)));
    let mut driver = Driver::from_scenario(scenario)?;
    // The scenario starts in the first main phase, so the trigger first fires in the hero's second turn.
    driver.idle_until(3, Step::Main1)?;
    settle(&mut driver)?;
    // The hero has no land: only the geode's mana pays for the Trinket.
    assert_eq!(driver.cast("Trinket").is_ok(), geode);
    Ok(())
}

#[test]
fn enemy_creatures_enter_tapped_under_the_anchor() -> Result<(), ScriptError> {
    let scenario = table(&["frostbound_anchor"])
        .hand(HERO, [free_creature("Cub", 1, 1)])
        .hand(BOSS, [free_creature("Whelp", 1, 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Cub")?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::Main1)?;
    driver.cast("Whelp")?;
    settle(&mut driver)?;
    let tapped = |name: &str| -> Result<bool, ScriptError> {
        let id = driver.id_of(name)?;
        Ok(driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped())
    };
    assert!(tapped("Whelp")?, "the boss's creature enters tapped");
    assert!(!tapped("Cub")?, "the hero's own does not");
    Ok(())
}

/// The party's seats at the first boss with the relic `relic` for each hero, and their decks: a precon each.
const PRECONS: [&str; 4] = [
    "precon_tramplesaurus_rex",
    "precon_calling_all_angels",
    "owner",
    "precon_wretched_ranks",
];

/// A deck card by name: a basic land from the testkit, anything else from the corpus.
fn card(name: &str) -> Arc<CardDefinition> {
    let basic = [
        ("Plains", Color::White),
        ("Island", Color::Blue),
        ("Swamp", Color::Black),
        ("Mountain", Color::Red),
        ("Forest", Color::Green),
    ];
    match basic.iter().find(|(basic, _)| *basic == name) {
        Some((_, color)) => mtg_testkit::cards::land(*color),
        None => corpus::card(name).unwrap_or_else(|_| mtg_testkit::cards::wastes()),
    }
}

fn plan_with(relic: &str, heroes: usize) -> FightPlan {
    let content = content().unwrap();
    let mut events = Vec::new();
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(at).unwrap(),
            deck: HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "WUBRG".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    let run = Run::replay(content, "warrens", 21, 0, &events).unwrap();
    let mut plan = run.fight_plan(content).unwrap().unwrap();
    for hero in &mut plan.heroes {
        hero.relics = vec![relic.to_owned()];
    }
    plan
}

fn setup_of(plan: &FightPlan, seed: u64) -> GameSetup {
    let content = content().unwrap();
    let mut decks: Vec<Vec<Arc<CardDefinition>>> = Vec::new();
    let mut commanders = Vec::new();
    for at in 0..plan.heroes.len() {
        let deck = fixture_deck(PRECONS[at % PRECONS.len()]).unwrap();
        decks.push(deck.cards.clone());
        commanders.push(deck.commanders.clone());
    }
    decks.push(
        plan.boss_deck()
            .iter()
            .flat_map(|(name, n)| (0..*n).map(move |_| card(name)))
            .collect(),
    );
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    plan.apply(content, setup).unwrap()
}

/// Plays the relic's fight to the answer limit with the invariants checked; only a game that runs on is allowed to stop.
fn plays(relic: &str, heroes: usize, seed: u64) {
    let content = content().unwrap();
    let plan = plan_with(relic, heroes);
    // The relic is an emblem in every hero's command zone from the start.
    let engine = Engine::new(setup_of(&plan, seed)).unwrap();
    let name = &content.card(relic).unwrap().name;
    for seat in 0..heroes {
        let state = engine.state();
        let held = state
            .zone(Zone::Command(PlayerId::from_index(seat)))
            .iter()
            .filter_map(|id| state.object(*id))
            .filter(|o| o.kind() == ObjectKind::Emblem && &o.card().name == name)
            .count();
        assert_eq!(held, 1, "{relic}: seat {seat}");
    }
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..plan.seats())
        .map(|seat| brain(Level::Normal, seed, PlayerId::from_index(seat), false))
        .collect();
    let options = PlayOptions {
        max_answers: 600,
        ..PlayOptions::default()
    };
    match play_brains(setup_of(&plan, seed), seed, options, &mut brains) {
        Ok(_) => {}
        Err(failure) if matches!(failure.kind, FailureKind::StepLimit(_)) => {}
        Err(failure) => panic!("{relic} at {heroes} seats: {failure}"),
    }
}

#[rstest]
fn every_relic_plays_a_real_fight(#[values(2, 3, 4)] heroes: usize) {
    let ids: Vec<String> = content()
        .unwrap()
        .relics()
        .iter()
        .map(|c| c.id.clone())
        .collect();
    for (at, id) in ids.iter().enumerate() {
        plays(id, heroes, 100 + at as u64);
    }
}
