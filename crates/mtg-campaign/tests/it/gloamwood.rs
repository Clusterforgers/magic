//! Goal: the Gloamwood holds together and its bosses do what their words say: fifteen lieutenants cover every colour
//! and colour pair and two final bosses close it, each naming its precon, a five-level innate, two phases and a satchel
//! of real cards; only a run of the Gloamwood's generation draws it, at every Heat; a Gloamwood ladder is four rooms
//! ending in a final boss; in real games the innates and phases act as written at their levels; and every boss plays a
//! real fight at level V.
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_campaign::content::{Boss, GLOAMWOOD_FROM, Theme};
use mtg_campaign::innate::{LEVELS, innate_id};
use mtg_campaign::{CardKind, Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::format::Commander;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::{creature, free_creature, island};
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::harness::{FailureKind, PlayOptions, play_brains};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn gloamwood() -> &'static Theme {
    content().unwrap().theme("gloamwood").unwrap()
}

fn boss(id: &str) -> &'static Boss {
    gloamwood().boss(id).unwrap()
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

/// What a table holds: the hero's battlefield, graveyard and hand, and the boss's battlefield and graveyard.
#[derive(Default)]
struct Board {
    heroes: Vec<Arc<CardDefinition>>,
    hero_dead: Vec<Arc<CardDefinition>>,
    hero_hand: Vec<Arc<CardDefinition>>,
    boss: Vec<Arc<CardDefinition>>,
    boss_dead: Vec<Arc<CardDefinition>>,
}

/// The hero (seat 0) against the boss (seat 1) with `board`, the boss's `emblems` and its `phases` (each beginning
/// at the end of round 1); both draw from libraries of Islands and the hero acts first in turn 1.
fn table(board: Board, emblems: Vec<Arc<CardDefinition>>, phases: &[&str]) -> Scenario {
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(HERO, board.heroes.into_iter().map(ready))
        .graveyard(HERO, board.hero_dead)
        .hand(HERO, board.hero_hand)
        .battlefield(BOSS, board.boss.into_iter().map(ready))
        .graveyard(BOSS, board.boss_dead)
        .library(HERO, (0..40).map(|_| island()))
        .library(BOSS, (0..40).map(|_| island()))
        .emblems(BOSS, emblems)
        .phases(BOSS, phases.iter().map(|id| phase(id)))
        .at(1, HERO, Step::Main1)
}

/// The battlefield permanents named `name` that `seat` controls.
fn named(driver: &Driver, seat: usize, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .filter(|id| controller_of(state, *id) == Some(pid(seat)))
        .collect()
}

fn count(driver: &Driver, seat: usize, name: &str) -> i32 {
    i32::try_from(named(driver, seat, name).len()).unwrap()
}

/// The greatest power among the permanents named `name` that `seat` controls, 0 with none.
fn power(driver: &Driver, seat: usize, name: &str) -> i32 {
    named(driver, seat, name)
        .iter()
        .filter_map(|id| characteristics(driver.state(), *id).and_then(|c| c.power))
        .max()
        .unwrap_or(0)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> i32 {
    let state = driver.state();
    i32::try_from(state.zone(Zone::Hand(pid(seat))).len()).unwrap()
}

fn colours(boss: &Boss) -> BTreeSet<char> {
    boss.colors.chars().collect()
}

#[test]
fn the_lieutenants_cover_every_colour_and_pair_and_two_final_bosses_close_it() {
    let (finals, lieutenants): (Vec<&Boss>, Vec<&Boss>) =
        gloamwood().bosses.iter().partition(|b| b.final_boss);
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
fn every_gloamwood_boss_holds_its_record() {
    let content = content().unwrap();
    let corpus = corpus::corpus().unwrap();
    let mut decks = BTreeSet::new();
    for boss in &gloamwood().bosses {
        assert_eq!(boss.since, GLOAMWOOD_FROM, "{}", boss.id);
        assert!(decks.insert(&boss.deck_file), "{} shares its deck", boss.id);
        let deck = content.deck(&boss.deck_file).unwrap();
        assert_eq!(deck.cards.iter().map(|(_, n)| n).sum::<u32>(), 100);
        let identity: BTreeSet<char> = deck.makeup.colors.keys().flat_map(|c| c.chars()).collect();
        assert!(identity.is_subset(&colours(boss)), "{}: {identity:?}", boss.id);
        for level in 1..=LEVELS {
            let card = content.card(&innate_id(&boss.innate, level)).unwrap();
            assert_eq!((card.kind, card.level), (CardKind::Innate, level));
            assert!(
                !card.text.is_empty() && !card.text.contains("{N}") && !card.text.contains('$'),
                "{}: {}",
                card.id,
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
fn only_a_run_of_its_generation_draws_the_gloamwood_at_every_heat() {
    let content = content().unwrap();
    for heat in 0..=mtg_campaign::scale::MAX_HEAT {
        let drawn = |generation| -> BTreeSet<String> {
            (0..300)
                .map(|seed| content.theme_at(seed, generation, heat).unwrap().id.clone())
                .collect()
        };
        assert!(drawn(GLOAMWOOD_FROM).contains("gloamwood"), "Heat {heat}");
        assert!(
            !drawn(GLOAMWOOD_FROM - 1).contains("gloamwood"),
            "an older run never meets it"
        );
    }
}

/// A run of the Gloamwood at Heat 1 with `heroes` heroes, started.
fn gloamwood_run(seed: u64, heroes: u8) -> Run {
    let mut events = vec![RunEvent::Content {
        version: GLOAMWOOD_FROM,
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
                colors: "G".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content().unwrap(), "gloamwood", seed, 1, &events).unwrap()
}

#[rstest]
fn a_gloamwood_ladder_is_three_lieutenants_then_a_final_boss(#[values(1, 2, 3, 99)] seed: u64) {
    let run = gloamwood_run(seed, 1);
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
    // Heat 1 starts the first boss at level I.
    let plan = run.fight_plan(content().unwrap()).unwrap().unwrap();
    assert_eq!(plan.boss.innate, innate_id(&bosses[0].innate, 1));
}

/// What a check counts at the hero's main phase of turn 3, after the boss's turn 2.
#[derive(Clone, Copy, Debug)]
enum Count {
    /// Permanents of this name the boss controls.
    Boss(&'static str),
    /// Permanents of this name the hero controls.
    Hero(&'static str),
    /// The greatest power among the boss's permanents of this name.
    BossPower(&'static str),
    /// Life the hero lost.
    HeroLost,
    /// Cards in the hero's hand.
    HeroHand,
}

/// The board of the named rows: three 2/2 Bears, a dead Ghoul and seven cards in hand for the hero; a 2/2 Brute and a
/// dead Wight (mana value 0) or Ancient (mana value 5) for the boss.
fn board(rows: &str) -> Board {
    let mut board = Board::default();
    for row in rows.split(',').filter(|r| !r.is_empty()) {
        match row {
            "hero_bears" => board
                .heroes
                .extend((0..3).map(|_| creature("Bear", 2, &[], 2, 2))),
            "hero_dead" => board.hero_dead.push(free_creature("Ghoul", 2, 2)),
            "hero_hand" => board.hero_hand.extend((0..7).map(|_| island())),
            "boss_brute" => board.boss.push(free_creature("Brute", 2, 2)),
            "boss_dead_small" => board.boss_dead.push(free_creature("Wight", 2, 2)),
            "boss_dead_big" => board
                .boss_dead
                .push(creature("Ancient", 5, &[], 5, 5)),
            other => panic!("{other}"),
        }
    }
    board
}

#[rstest]
#[case::wild_hunt_one_counter("bramblehorn_wild_hunt", 1, "", Count::BossPower("Wolf"), 2)]
#[case::wild_hunt_four_counters("bramblehorn_wild_hunt", 5, "", Count::BossPower("Wolf"), 5)]
#[case::dirge_one_spirit("pale_choir_dirge", 1, "", Count::Boss("Spirit"), 1)]
#[case::dirge_three_spirits("pale_choir_dirge", 5, "", Count::Boss("Spirit"), 3)]
#[case::stolen_names_unpaid("nightshade_stolen_names", 1, "", Count::Boss("Faerie Rogue"), 1)]
#[case::restless_dead_small_rises("hollis_restless_dead", 1, "boss_dead_small", Count::Boss("Wight"), 1)]
#[case::restless_dead_big_stays("hollis_restless_dead", 1, "boss_dead_big", Count::Boss("Ancient"), 0)]
#[case::restless_dead_big_rises("hollis_restless_dead", 5, "boss_dead_big", Count::Boss("Ancient"), 1)]
#[case::toads_gift_one_counter("mossbeard_toads_gift", 1, "boss_brute", Count::BossPower("Brute"), 3)]
#[case::toads_gift_three_counters("mossbeard_toads_gift", 5, "boss_brute", Count::BossPower("Brute"), 5)]
#[case::moon_hunger_one("gnarl_moon_hunger", 1, "boss_brute", Count::BossPower("Brute"), 3)]
#[case::moon_hunger_two("gnarl_moon_hunger", 5, "boss_brute", Count::BossPower("Brute"), 4)]
fn an_innate_acts_at_its_level(
    #[case] id: &str,
    #[case] level: u8,
    #[case] rows: &str,
    #[case] what: Count,
    #[case] expected: i32,
) -> Result<(), ScriptError> {
    check(table(board(rows), vec![leveled(id, level)], &[]), what, expected)
}

#[rstest]
#[case::fog_rolls_in("fennick_fog_rolls_in", "hero_bears", Count::Hero("Bear"), 0)]
#[case::barrows_open("hollis_barrows_open", "hero_dead", Count::Boss("Ghoul"), 1)]
#[case::moon_rises("gnarl_moon_rises", "hero_bears", Count::Boss("Wolf"), 3)]
#[case::penance("morrow_penance", "hero_bears", Count::Hero("Bear"), 1)]
#[case::lanterns_lit("lumen_lanterns_lit", "", Count::Boss("Angel"), 1)]
#[case::herd_answers("antlered_herd_answers", "", Count::Boss("Elk"), 1)]
#[case::marsh_fire("cinderwick_marsh_fire", "hero_bears", Count::Hero("Bear"), 0)]
#[case::pyres_lit("corvane_pyres_lit", "", Count::HeroLost, 2)]
#[case::court_of_whispers("nightshade_court_of_whispers", "hero_hand", Count::HeroHand, 4)]
fn a_phase_acts_as_it_begins(
    #[case] id: &str,
    #[case] rows: &str,
    #[case] what: Count,
    #[case] expected: i32,
) -> Result<(), ScriptError> {
    check(table(board(rows), Vec::new(), &[id]), what, expected)
}

#[rstest]
#[case::level_one(1, 1)]
#[case::level_five(5, 3)]
fn the_rut_burns_the_hero_when_the_herd_answers(
    #[case] level: u8,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = table(
        Board::default(),
        vec![leveled("antlered_rut", level)],
        &["antlered_herd_answers"],
    );
    check(scenario, Count::HeroLost, damage)
}

/// Plays the scenario to the hero's main phase of turn 3 and checks `what`.
fn check(scenario: Scenario, what: Count, expected: i32) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    let got = match what {
        Count::Boss(name) => count(&driver, BOSS, name),
        Count::Hero(name) => count(&driver, HERO, name),
        Count::BossPower(name) => power(&driver, BOSS, name),
        Count::HeroLost => 20 - life(&driver, HERO),
        Count::HeroHand => hand(&driver, HERO),
    };
    assert_eq!(got, expected, "{what:?}");
    Ok(())
}

/// Two heroes on fixture precons against the boss at innate level V with its precon, phases and satchel.
fn fight_setup(id: &str, seed: u64) -> GameSetup {
    let content = content().unwrap();
    let run = gloamwood_run(seed, 2);
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
fn a_real_gloamwood_fight_at_level_v_plays_on(
    #[values(
        "lumen_lantern",
        "fennick_mists",
        "hollis_barrow",
        "cinderwick_wisp",
        "bramblehorn_stag",
        "pale_choir",
        "nightshade_thief",
        "grizel_bonfire",
        "gnarl_howler",
        "hawthorn_hedge",
        "morrow_chapel",
        "orrin_owltower",
        "burdock_hollow",
        "mossbeard_sage",
        "corvane_witchfinder",
        "nettlewick_hag",
        "antlered_king"
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
