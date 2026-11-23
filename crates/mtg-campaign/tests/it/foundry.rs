//! Goal: the Ember Foundry holds together and its bosses do what their words say: fifteen lieutenants cover every colour
//! and colour pair and two final bosses close it, each naming its precon, a five-level innate, two phases and a satchel
//! of real cards; only a run at Heat 3 or more draws the Foundry; a Foundry ladder is four rooms ending in a final boss;
//! in real games the innates and phases act as written; and every boss plays a real fight at level V.
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_campaign::content::{Boss, Theme};
use mtg_campaign::innate::{LEVELS, innate_id};
use mtg_campaign::{CardKind, Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::controller_of;
use mtg_engine::format::Commander;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::harness::{FailureKind, PlayOptions, play_brains};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;
/// The least Heat that draws the Foundry.
const FOUNDRY_HEAT: u8 = 3;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn foundry() -> &'static Theme {
    content().unwrap().theme("foundry").unwrap()
}

fn boss(id: &str) -> &'static Boss {
    foundry().boss(id).unwrap()
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The innate `id` at `level`.
fn leveled(id: &str, level: u8) -> Arc<CardDefinition> {
    custom(&innate_id(id, level))
}

/// `id` as a phase that begins at the end of round 1.
fn phase(id: &str) -> BossPhase {
    BossPhase {
        begins: PhaseStart::AtRound(1),
        emblem: custom(id),
    }
}

/// The hero (seat 0) with `heroes` on the battlefield and `dead` in the graveyard against the boss (seat 1) with
/// `boss` and the emblems; both draw from libraries of Islands.
fn table(
    heroes: Vec<Arc<CardDefinition>>,
    dead: Vec<Arc<CardDefinition>>,
    boss: Vec<Arc<CardDefinition>>,
    emblems: Vec<Arc<CardDefinition>>,
) -> Scenario {
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(HERO, heroes.into_iter().map(ready))
        .graveyard(HERO, dead)
        .battlefield(BOSS, boss.into_iter().map(ready))
        .library(HERO, (0..40).map(|_| island()))
        .library(BOSS, (0..40).map(|_| island()))
        .emblems(BOSS, emblems)
        .at(1, HERO, Step::Main1)
}

/// The battlefield permanents named `name` that `seat` controls.
fn controlled(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == name))
        .filter(|id| controller_of(state, **id) == Some(pid(seat)))
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn colours(boss: &Boss) -> BTreeSet<char> {
    boss.colors.chars().collect()
}

#[test]
fn the_lieutenants_cover_every_colour_and_pair_and_two_final_bosses_close_it() {
    let (finals, lieutenants): (Vec<&Boss>, Vec<&Boss>) =
        foundry().bosses.iter().partition(|b| b.final_boss);
    assert_eq!(finals.len(), 2);
    let mut covered: BTreeSet<BTreeSet<char>> = lieutenants.iter().map(|b| colours(b)).collect();
    let wubrg = ['W', 'U', 'B', 'R', 'G'];
    for (at, first) in wubrg.iter().enumerate() {
        assert!(covered.remove(&BTreeSet::from([*first])), "{first}");
        for second in &wubrg[at + 1..] {
            assert!(
                covered.remove(&BTreeSet::from([*first, *second])),
                "{first}{second}"
            );
        }
    }
    assert!(covered.is_empty(), "{covered:?}");
    assert_eq!(lieutenants.len(), 15);
}

#[test]
fn every_foundry_boss_holds_its_record() {
    let content = content().unwrap();
    let corpus = corpus::corpus().unwrap();
    let mut decks = BTreeSet::new();
    for boss in &foundry().bosses {
        assert_eq!(boss.since, mtg_campaign::run::CONTENT, "{}", boss.id);
        assert!(decks.insert(&boss.deck_file), "{} shares its deck", boss.id);
        let deck = content.deck(&boss.deck_file).unwrap();
        assert_eq!(deck.cards.iter().map(|(_, n)| n).sum::<u32>(), 100);
        for level in 1..=LEVELS {
            let card = content.card(&innate_id(&boss.innate, level)).unwrap();
            assert_eq!((card.kind, card.level), (CardKind::Innate, level));
            assert!(
                !card.text.contains("{N}") && !card.text.contains('$'),
                "{}",
                card.text
            );
        }
        assert_eq!(boss.round_phases.len(), 2);
        for (at, id) in boss.round_phases.iter().enumerate() {
            let card = content.card(id).unwrap();
            assert_eq!(card.kind, CardKind::Phase, "{id}");
            assert!(card.name.starts_with(&format!("Phase {}", at + 1)), "{id}");
        }
        let reusable: Vec<bool> = boss.satchel.iter().map(|s| s.reusable).collect();
        assert_eq!(reusable, [true, true, true, false], "{}", boss.id);
        for card in &boss.satchel {
            let support = corpus.support(&card.card).unwrap().support;
            assert!(
                matches!(support, Support::Verified | Support::Read),
                "{}",
                card.card
            );
            assert!(card.cue.is_some(), "{}", card.card);
        }
    }
}

#[test]
fn only_a_run_at_heat_three_or_more_draws_the_foundry() {
    let content = content().unwrap();
    let generation = mtg_campaign::run::CONTENT;
    let draw = |seed, heat| content.theme_at(seed, generation, heat).unwrap().id.clone();
    for heat in 0..FOUNDRY_HEAT {
        assert!(
            (0..300).all(|seed| draw(seed, heat) != "foundry"),
            "Heat {heat}"
        );
        assert!((0..300).all(|seed| draw(seed, heat) == draw(seed, 0)));
    }
    assert!((0..300).all(|seed| content.theme_for(seed, generation).unwrap().id == draw(seed, 0)));
    for heat in FOUNDRY_HEAT..=mtg_campaign::scale::MAX_HEAT {
        let drawn: BTreeSet<String> = (0..300).map(|seed| draw(seed, heat)).collect();
        assert_eq!(
            drawn,
            BTreeSet::from(["foundry".into(), "spire".into(), "warrens".into()])
        );
    }
}

/// A run of the Foundry at Heat 3 with `heroes` heroes, started.
fn foundry_run(seed: u64, heroes: u8) -> Run {
    let mut events = vec![RunEvent::Content {
        version: mtg_campaign::run::CONTENT,
    }];
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: at,
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "R".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content().unwrap(), "foundry", seed, FOUNDRY_HEAT, &events).unwrap()
}

#[rstest]
fn a_foundry_ladder_is_three_lieutenants_then_a_final_boss(#[values(1, 2, 3, 99)] seed: u64) {
    let run = foundry_run(seed, 1);
    let bosses: Vec<&Boss> = run
        .map
        .nodes
        .iter()
        .filter_map(|n| n.boss.as_deref())
        .map(boss)
        .collect();
    assert_eq!(bosses.len(), 4);
    let finals: Vec<bool> = bosses.iter().map(|b| b.final_boss).collect();
    assert_eq!(finals, [false, false, false, true]);
    // Heat 3 starts the first boss at level II.
    let plan = run.fight_plan(content().unwrap()).unwrap().unwrap();
    assert_eq!(plan.boss.innate, innate_id(&bosses[0].innate, 2));
}

#[test]
fn scrap_fire_builds_a_servo_each_upkeep_and_burns_the_heroes_when_an_artifact_dies()
-> Result<(), ScriptError> {
    let scenario = table(
        vec![],
        vec![],
        vec![],
        vec![leveled("grizzek_scrap_fire", 1)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(controlled(&driver, BOSS, "Servo"), 1);
    assert_eq!(life(&driver, HERO), 20);
    driver.idle_until(4, Step::Main1)?;
    assert_eq!(controlled(&driver, BOSS, "Servo"), 2);
    Ok(())
}

#[rstest]
#[case::level_one(1, 1)]
#[case::level_five(5, 2)]
fn ember_tithe_burns_each_hero_at_the_bosss_end_step(
    #[case] level: u8,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = table(
        vec![],
        vec![],
        vec![],
        vec![leveled("vashka_ember_tithe", level)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(life(&driver, HERO), 20 - damage);
    assert_eq!(life(&driver, BOSS), 40);
    Ok(())
}

#[test]
fn foundry_tax_makes_an_inkling_for_a_hero_who_cannot_pay() -> Result<(), ScriptError> {
    let scenario = table(
        vec![],
        vec![],
        vec![],
        vec![leveled("calvaine_foundry_tax", 1)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(controlled(&driver, BOSS, "Inkling"), 1);
    Ok(())
}

#[rstest]
#[case::small_rises(0, true)]
#[case::big_stays(5, false)]
fn stoke_the_dead_raises_a_heros_fallen_creature_for_the_boss(
    #[case] cost: u32,
    #[case] rises: bool,
) -> Result<(), ScriptError> {
    let dead = mtg_testkit::cards::creature("Stoker", cost, &[], 2, 2);
    let scenario = table(
        vec![],
        vec![dead],
        vec![],
        vec![leveled("haldric_stoke_the_dead", 1)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(controlled(&driver, BOSS, "Stoker"), usize::from(rises));
    Ok(())
}

#[test]
fn the_hour_strikes_taps_the_heroes_creatures_through_their_next_untap() -> Result<(), ScriptError>
{
    let scenario = table(vec![free_creature("Bear", 2, 2)], vec![], vec![], vec![])
        .phases(BOSS, [phase("ottoline_hour_strikes")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    let bear = driver.id_of("Bear")?;
    let tapped = |driver: &Driver| {
        driver
            .state()
            .object(bear)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    assert!(tapped(&driver), "it skips the untap after the phase began");
    driver.idle_until(5, Step::Main1)?;
    assert!(!tapped(&driver));
    Ok(())
}

#[test]
fn the_forge_claims_each_heros_greatest_artifact_once() -> Result<(), ScriptError> {
    let scenario = table(
        vec![
            corpus::card("Sol Ring").unwrap(),
            corpus::card("Hedron Archive").unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    )
    .phases(BOSS, [phase("kaldrek_forge_claims")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(controlled(&driver, BOSS, "Hedron Archive"), 1);
    assert_eq!(controlled(&driver, HERO, "Sol Ring"), 1);
    driver.idle_until(5, Step::Main1)?;
    assert_eq!(controlled(&driver, HERO, "Sol Ring"), 1);
    Ok(())
}

/// Two heroes on fixture precons against the boss at innate level V with its precon, phases and satchel.
fn fight_setup(id: &str, seed: u64) -> GameSetup {
    let content = content().unwrap();
    let run = foundry_run(seed, 2);
    let mut plan = run.fight_plan(content).unwrap().unwrap();
    plan.boss = run.boss_seat(content, boss(id), 1, (2, 12)).unwrap();
    let mut decks = Vec::new();
    let mut commanders = Vec::new();
    for name in ["precon_tramplesaurus_rex", "precon_calling_all_angels"] {
        let deck = fixture_deck(name).unwrap();
        decks.push(deck.cards.clone());
        commanders.push(deck.commanders.clone());
    }
    decks.push(crate::common::boss_deck(&plan));
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    plan.apply(content, setup).unwrap()
}

#[rstest]
fn a_real_foundry_fight_at_level_v_plays_on(
    #[values(
        "brannoc_anvil",
        "ottoline_clockworks",
        "morvane_tomb",
        "grizzek_scrapyard",
        "veyla_grove",
        "ilsabet_flues",
        "haldric_stokehold",
        "vashka_pyre",
        "kassa_quench",
        "ironbark_hollow",
        "calvaine_counting_house",
        "ivra_workshop",
        "winnow_ashfield",
        "teodora_reforge",
        "quillon_moldworks",
        "kaldrek_great_forge",
        "ignivar_furnace"
    )]
    id: &str,
) {
    let seed = 7;
    let setup = fight_setup(id, seed);
    let innate = innate_id(&boss(id).innate, LEVELS);
    let name = &content().unwrap().card(&innate).unwrap().name;
    let engine = Engine::new(setup.clone()).unwrap();
    let state = engine.state();
    assert!(
        state
            .zone(Zone::Command(pid(2)))
            .iter()
            .filter_map(|o| state.object(*o))
            .any(|o| &o.card().name == name),
        "{id} holds {name}"
    );
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..3)
        .map(|seat| brain(Level::Normal, seed, pid(seat), false))
        .collect();
    let options = PlayOptions {
        max_answers: 600,
        ..PlayOptions::default()
    };
    match play_brains(setup, seed, options, &mut brains) {
        Ok(_) => {}
        Err(failure) if matches!(failure.kind, FailureKind::StepLimit(_)) => {}
        Err(failure) => panic!("{id}: {failure}"),
    }
}
