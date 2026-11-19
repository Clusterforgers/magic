//! Goal: the reworked Skrix, Mossjaw, Vex, Grimgut, Seraphine, Ysolde, Hollowmere and Morwenna load whole (precon deck
//! file, colours of its commander, innate in five levels, two round phases, four satchel cards that play
//! automatically); each innate does its thing at levels I and V against two heroes; each phase begins at its round and
//! its "when it begins" effect fires once; a reusable satchel card casts and returns with its tax; and a real fight at
//! level V never loops.
use std::path::Path;
use std::sync::Arc;

use mtg_campaign::content::Boss;
use mtg_campaign::innate::innate_id;
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_campaign::{Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::support::Support;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::characteristics::characteristics;
use mtg_engine::events::Event;
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::cards::{burn, free_creature, land, mountain};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::loop_answer::BOSS_LOOP_CAP;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

/// The bosses of this block.
const BOSSES: [&str; 8] = [
    "skrix_nest",
    "mossjaw_hall",
    "vex_crypt",
    "grimgut_court",
    "seraphine_spire",
    "ysolde_reef",
    "hollowmere_belfry",
    "morwenna_court",
];
/// The boss's seat at the two-hero table.
const BOSS: usize = 2;
/// The creatures each hero starts with: name, power and toughness.
const HERO_BOARD: [(&str, i32, i32); 4] = [
    ("Bear", 2, 2),
    ("Wolf", 2, 2),
    ("Ogre", 3, 3),
    ("Giant", 4, 4),
];

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn boss(id: &str) -> &'static Boss {
    content()
        .unwrap()
        .theme("warrens")
        .unwrap()
        .boss(id)
        .unwrap()
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The boss's two round phases, beginning at their rounds.
fn phases(boss: &Boss) -> Vec<BossPhase> {
    boss.round_phases
        .iter()
        .zip(PHASE_ROUNDS)
        .map(|(id, round)| BossPhase {
            begins: PhaseStart::AtRound(round),
            emblem: custom(id),
        })
        .collect()
}

/// Two heroes (seats 0 and 1: the [`HERO_BOARD`], a Spirit emblem, two creature cards and a Mountain in the graveyard,
/// five cards in hand) against the boss (seat 2: a ready 3/3 Brute and `board`, 40 life, `emblems` and its round
/// phases), all on libraries of Mountains; the heroes are one team.
fn table(
    boss: &Boss,
    emblems: Vec<Arc<CardDefinition>>,
    board: impl IntoIterator<Item = PresetPermanent>,
) -> Scenario {
    let mut own = vec![ready(free_creature("Brute", 3, 3))];
    own.extend(board);
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 0, 1],
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(3).house_rules(house);
    for hero in 0..BOSS {
        let board = HERO_BOARD.map(|(name, p, t)| ready(free_creature(name, p, t)));
        scenario = scenario
            .life(hero, 20)
            .spirit(hero, Arc::new(emblem_definition("Haunt", &[])))
            .battlefield(hero, board)
            .graveyard(
                hero,
                [
                    free_creature("Fallen", 1, 1),
                    free_creature("Lost", 1, 1),
                    mountain(),
                ],
            )
            .hand(hero, (0..5).map(|_| mountain()));
    }
    for seat in 0..=BOSS {
        scenario = scenario.library(seat, (0..60).map(|_| mountain()));
    }
    scenario
        .life(BOSS, 40)
        .battlefield(BOSS, own)
        .emblems(BOSS, emblems)
        .phases(BOSS, phases(boss))
}

/// The permanents named `name` that `seat` controls.
fn named(driver: &Driver, seat: usize, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.controller() == pid(seat) && o.card().name == name)
        })
        .collect()
}

fn count(driver: &Driver, seat: usize, name: &str) -> i32 {
    i32::try_from(named(driver, seat, name).len()).unwrap()
}

/// The power of the first permanent named `name` that `seat` controls, 0 with none.
fn power(driver: &Driver, seat: usize, name: &str) -> i32 {
    named(driver, seat, name)
        .first()
        .and_then(|id| characteristics(driver.state(), *id))
        .and_then(|c| c.power)
        .unwrap_or(0)
}

fn keyword(driver: &Driver, seat: usize, name: &str, keyword: &Keyword) -> i32 {
    named(driver, seat, name)
        .first()
        .is_some_and(|id| has_keyword(driver.state(), *id, keyword))
        .into()
}

/// The heroes' creatures on the battlefield, and how many of them are tapped.
fn hero_creatures(driver: &Driver) -> (i32, i32) {
    let state = driver.state();
    let mut all = 0;
    let mut tapped = 0;
    for id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(*id) else {
            continue;
        };
        if object.controller().index() < BOSS
            && HERO_BOARD.iter().any(|(n, ..)| object.card().name == *n)
        {
            all += 1;
            tapped += i32::from(
                object
                    .permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped),
            );
        }
    }
    (all, tapped)
}

fn hand(driver: &Driver, seat: usize) -> i32 {
    i32::try_from(driver.state().zone(Zone::Hand(pid(seat))).len()).unwrap()
}

/// The boss casts each of `bolts` (3 damage) at its target named on its first turn, one after another.
fn bolt(driver: &mut Driver, victims: &[&str]) -> Result<(), ScriptError> {
    for victim in victims {
        let target = driver.target_named(victim)?;
        driver.cast_at("Bolt", &[target])?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
    }
    Ok(())
}

/// What the innate of `id` at `level` has done, in two numbers the case names.
fn innate_effect(id: &str, level: u8) -> Result<(i32, i32), ScriptError> {
    let boss = boss(id);
    let innate = custom(&innate_id(boss.innate.as_str(), level));
    let mut emblems = vec![innate];
    if id == "seraphine_spire" {
        // A source of life gain each upkeep, so the end step finds that she gained life.
        let spring = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
        emblems.push(Arc::new(emblem_definition(
            "Spring",
            &[serde_json::from_str(spring).unwrap()],
        )));
    }
    let bolts = matches!(id, "vex_crypt" | "hollowmere_belfry");
    let board = match id {
        "seraphine_spire" => vec![ready(free_creature("Cleric", 1, 1))],
        _ if bolts => vec![ready(mountain()), ready(mountain())],
        _ => Vec::new(),
    };
    let mut scenario = table(boss, emblems, board);
    if bolts {
        scenario = scenario
            .at(3, BOSS, Step::Main1)
            .hand(BOSS, [burn("Bolt", 3), burn("Bolt", 3)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    Ok(match id {
        "skrix_nest" | "grimgut_court" => {
            driver.idle_until(3, Step::Main1)?;
            let token = if id == "skrix_nest" { "Rat" } else { "Goblin" };
            (count(&driver, BOSS, token), power(&driver, BOSS, token))
        }
        "mossjaw_hall" => (
            power(&driver, BOSS, "Brute"),
            keyword(&driver, BOSS, "Brute", &Keyword::Trample),
        ),
        "vex_crypt" => {
            bolt(&mut driver, &["Bear", "Wolf"])?;
            (
                count(&driver, BOSS, "Zombie"),
                power(&driver, BOSS, "Zombie"),
            )
        }
        "hollowmere_belfry" => {
            bolt(&mut driver, &["Brute"])?;
            (
                count(&driver, BOSS, "Spirit"),
                power(&driver, BOSS, "Spirit"),
            )
        }
        "seraphine_spire" => {
            driver.idle_until(4, Step::Main1)?;
            (
                power(&driver, BOSS, "Brute"),
                power(&driver, BOSS, "Cleric"),
            )
        }
        "ysolde_reef" => {
            driver.idle_until(3, Step::Main1)?;
            let (_, now) = hero_creatures(&driver);
            driver.idle_until(5, Step::Main1)?;
            (now, hero_creatures(&driver).1)
        }
        "morwenna_court" => {
            driver.idle_until(3, Step::Upkeep)?;
            let before = hand(&driver, 0) + hand(&driver, 1);
            driver.idle_until(3, Step::Main1)?;
            (before - hand(&driver, 0) - hand(&driver, 1), 0)
        }
        other => panic!("no innate check for {other}"),
    })
}

#[rstest]
#[case::skrix_one_rat("skrix_nest", 1, (1, 1))]
#[case::skrix_five_rats("skrix_nest", 5, (5, 1))]
#[case::mossjaw_plus_one_no_trample("mossjaw_hall", 1, (4, 0))]
#[case::mossjaw_plus_four_and_trample("mossjaw_hall", 5, (7, 1))]
#[case::vex_first_death_only("vex_crypt", 1, (1, 2))]
#[case::vex_two_each_with_a_counter("vex_crypt", 5, (4, 3))]
#[case::grimgut_one_goblin("grimgut_court", 1, (1, 1))]
#[case::grimgut_five_goblins("grimgut_court", 5, (5, 1))]
#[case::seraphine_lifelink_only("seraphine_spire", 1, (3, 1))]
#[case::seraphine_two_on_the_greatest("seraphine_spire", 3, (5, 1))]
#[case::seraphine_one_on_each("seraphine_spire", 5, (4, 2))]
#[case::ysolde_one_tapped_that_untaps("ysolde_reef", 1, (1, 0))]
#[case::ysolde_four_held_down("ysolde_reef", 5, (4, 4))]
#[case::hollowmere_one_spirit("hollowmere_belfry", 1, (1, 1))]
#[case::hollowmere_five_spirits("hollowmere_belfry", 5, (5, 1))]
#[case::morwenna_the_eyed_hero_discards("morwenna_court", 1, (1, 0))]
#[case::morwenna_each_hero_discards("morwenna_court", 5, (2, 0))]
fn each_innate_does_its_thing_at_its_level(
    #[case] id: &str,
    #[case] level: u8,
    #[case] want: (i32, i32),
) -> Result<(), ScriptError> {
    assert_eq!(innate_effect(id, level)?, want, "{id} at level {level}");
    Ok(())
}

/// What the phases of `id` have done, in two numbers the case names.
fn phase_effect(id: &str, driver: &Driver) -> (i32, i32) {
    match id {
        "skrix_nest" => (count(driver, BOSS, "Rat"), power(driver, BOSS, "Rat")),
        "mossjaw_hall" => (
            driver.state().player(pid(BOSS)).unwrap().life(),
            keyword(driver, BOSS, "Brute", &Keyword::Haste),
        ),
        "vex_crypt" => (count(driver, BOSS, "Zombie"), power(driver, BOSS, "Zombie")),
        "grimgut_court" => (count(driver, BOSS, "Goblin"), power(driver, BOSS, "Goblin")),
        "seraphine_spire" => (count(driver, BOSS, "Angel"), hero_creatures(driver).0),
        "ysolde_reef" => (hero_creatures(driver).0, power(driver, BOSS, "Brute")),
        "hollowmere_belfry" => (count(driver, BOSS, "Spirit"), power(driver, BOSS, "Brute")),
        "morwenna_court" => (hand(driver, 1), power(driver, BOSS, "Brute")),
        other => panic!("no phase check for {other}"),
    }
}

/// Before Phase 1 (round 8), after it (round 9), before Phase 2 (round 14) and after it (round 15).
const PHASE_POINTS: [u32; 4] = [24, 25, 40, 43];

#[rstest]
#[case::skrix("skrix_nest", [(0, 0), (8, 1), (8, 1), (16, 3)])]
#[case::mossjaw("mossjaw_hall", [(40, 0), (60, 0), (60, 0), (60, 1)])]
#[case::vex("vex_crypt", [(0, 0), (9, 2), (9, 2), (9, 4)])]
#[case::grimgut("grimgut_court", [(0, 0), (4, 2), (4, 2), (4, 2)])]
#[case::seraphine("seraphine_spire", [(0, 8), (0, 8), (5, 8), (6, 0)])]
#[case::ysolde("ysolde_reef", [(8, 3), (0, 3), (0, 3), (0, 4)])]
#[case::hollowmere("hollowmere_belfry", [(0, 3), (4, 3), (4, 3), (4, 4)])]
#[case::morwenna("morwenna_court", [(5, 3), (3, 3), (7, 3), (7, 4)])]
fn each_phase_begins_at_its_round_and_fires_once(
    #[case] id: &str,
    #[case] want: [(i32, i32); 4],
) -> Result<(), ScriptError> {
    let scenario = table(boss(id), Vec::new(), []).at(PHASE_POINTS[0], BOSS, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let mut seen = Vec::new();
    for (at, turn) in PHASE_POINTS.into_iter().enumerate() {
        let step = if at == 0 { Step::Main2 } else { Step::Main1 };
        driver.idle_until(turn, step)?;
        let (_, begun) = driver.state().player(pid(BOSS)).unwrap().phases();
        seen.push((begun, phase_effect(id, &driver)));
    }
    let begun: Vec<usize> = seen.iter().map(|(b, _)| *b).collect();
    assert_eq!(begun, [0, 1, 1, 2], "{id}");
    let effects: Vec<(i32, i32)> = seen.iter().map(|(_, e)| *e).collect();
    assert_eq!(effects, want, "{id}");
    Ok(())
}

/// For each boss: a reusable satchel card and the name of what it targets.
#[rstest]
#[case::skrix("skrix_nest", "Feed the Swarm", "Bear")]
#[case::mossjaw("mossjaw_hall", "Beast Within", "Bear")]
#[case::vex("vex_crypt", "Doom Blade", "Bear")]
#[case::grimgut("grimgut_court", "Lightning Bolt", "Bear")]
#[case::seraphine("seraphine_spire", "Swords to Plowshares", "Bear")]
#[case::ysolde("ysolde_reef", "River Heralds' Boon", "Brute")]
#[case::hollowmere("hollowmere_belfry", "Momentary Blink", "Brute")]
#[case::morwenna("morwenna_court", "Peppersmoke", "Bear")]
fn a_reusable_satchel_card_returns_with_its_tax(
    #[case] id: &str,
    #[case] name: &str,
    #[case] victim: &str,
) -> Result<(), ScriptError> {
    let boss = boss(id);
    let card = |name: &str| corpus::card(name).unwrap();
    let reusable = boss
        .satchel
        .iter()
        .filter(|s| s.reusable)
        .map(|s| card(&s.card));
    let once = boss
        .satchel
        .iter()
        .filter(|s| !s.reusable)
        .map(|s| card(&s.card));
    let lands = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ]
    .into_iter()
    .flat_map(|color| (0..3).map(move |_| ready(land(color))));
    let scenario = table(boss, Vec::new(), lands)
        .at(3, BOSS, Step::Main1)
        .satchel_reusable(BOSS, reusable.collect::<Vec<_>>())
        .satchel(BOSS, once.collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario)?;
    let state = driver.state();
    let spell = *state
        .satchel(pid(BOSS))
        .iter()
        .find(|s| state.object(**s).is_some_and(|o| o.card().name == name))
        .unwrap();
    assert_eq!(state.satchel(pid(BOSS)).len(), 4);
    let target = driver.target_named(victim)?;
    driver.cast_at(name, &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let state = driver.state();
    assert!(state.satchel(pid(BOSS)).contains(&spell), "{name} is back");
    assert_eq!(state.satchel_tax(spell), 2, "{name}");
    Ok(())
}

/// An innate whose shape changes with the level: a cost of {N}, a step only from level III, a text for each level.
const SHAPED: &str = r#"[{"id":"toll","name":"Toll","kind":"Innate","type_line":"Boss — Innate","levels":[1,1,2,3,4],
  "text":"unused","texts":["one {N}","two {N}","three {N}","four {N}","five {N}"],
  "abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},
    "if":{"Compare":{"left":"$L","op":"Ge","right":3}},
    "effects":[{"Pay":{"who":"Spotlit","cost":"{N}","unpaid":[{"Discard":{"who":"Spotlit"}}]}}]}}]}]"#;

#[rstest]
#[case::one(1, "one 1", "{1}", 1)]
#[case::three(3, "three 2", "{2}", 3)]
#[case::five(5, "five 4", "{4}", 5)]
fn an_innate_takes_its_level_its_cost_and_its_text(
    #[case] level: u8,
    #[case] text: &str,
    #[case] cost: &str,
    #[case] at: u8,
) {
    let content = mtg_campaign::content::Content::parse(&[SHAPED], &[], "[]").unwrap();
    let card = content.card(&innate_id("toll", level)).unwrap();
    assert_eq!(card.text, text);
    let rules = serde_json::to_string(&card.abilities).unwrap();
    assert!(rules.contains(&format!("\"cost\":\"{cost}\"")), "{rules}");
    assert!(rules.contains(&format!("\"left\":{at}")), "{rules}");
}

/// The face commander's colour identity of a deck file, as WUBRG letters, read from its Scryfall fixture (a commander
/// that plays from a draft is no corpus card).
fn deck_colors(file: &str) -> String {
    let deck = deck_file(file);
    let mut letters = String::new();
    for face in deck["commander"].as_array().unwrap() {
        let slug = mtg_cards::load::slug(face.as_str().unwrap());
        let drafts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cli/drafts/fixtures");
        let text = [corpus::fixture_dir(), drafts]
            .iter()
            .find_map(|dir| std::fs::read_to_string(dir.join(format!("{slug}.json"))).ok())
            .unwrap();
        let card: serde_json::Value = serde_json::from_str(&text).unwrap();
        for letter in card["color_identity"].as_array().unwrap() {
            letters.push_str(letter.as_str().unwrap());
        }
    }
    letters
}

fn deck_file(file: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("content/decks")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn sorted(letters: &str) -> Vec<char> {
    let mut letters: Vec<char> = letters.chars().collect();
    letters.sort_unstable();
    letters
}

#[rstest]
fn each_reworked_boss_loads_whole(#[values(0, 1, 2, 3, 4, 5, 6, 7)] at: usize) {
    let content = content().unwrap();
    let boss = boss(BOSSES[at]);
    let file = boss.deck_file.as_str();
    // The boss takes its colours from its deck.
    assert_eq!(
        sorted(&boss.colors),
        sorted(&deck_colors(file)),
        "{}",
        boss.id
    );
    let innate = boss.innate.as_str();
    for level in 1..=5 {
        let card = content.card(&innate_id(innate, level)).unwrap();
        assert_eq!(card.level, level);
        assert!(
            !card.text.contains("{N}") && !card.text.contains('$'),
            "{}",
            card.text
        );
        let rules = serde_json::to_string(&card.abilities).unwrap();
        assert!(!rules.contains("$N") && !rules.contains("$L"), "{rules}");
    }
    assert_eq!(boss.round_phases.len(), 2);
    for id in &boss.round_phases {
        assert!(content.card(id).unwrap().name.starts_with("Phase"), "{id}");
    }
    assert_eq!(boss.satchel.len(), 4);
    for spell in &boss.satchel {
        let record = corpus::corpus().unwrap().support(&spell.card).unwrap();
        assert!(
            matches!(record.support, Support::Verified | Support::Read),
            "{} plays automatically",
            spell.card
        );
    }
}

/// The game of a level-V fight against `boss` with two heroes on a fixture deck; deck cards the corpus lacks (read-tier
/// drafts) stand in as basic lands of the boss's first colour.
fn fight(boss: &Boss, seed: u64) -> Engine {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content {
        version: mtg_campaign::run::CONTENT,
    }];
    for at in 0..2u8 {
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
    let run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    let mut plan = run.fight_plan(content).unwrap().unwrap();
    plan.boss = run.boss_seat(content, boss, 3, (2, 12)).unwrap();
    let stand_in = match boss.colors.chars().next() {
        Some('W') => Color::White,
        Some('U') => Color::Blue,
        Some('B') => Color::Black,
        Some('R') => Color::Red,
        _ => Color::Green,
    };
    let boss_deck: Vec<Arc<CardDefinition>> = crate::common::boss_deck(&plan)
        .into_iter()
        .map(|card| {
            if card.name == "Wastes" {
                land(stand_in)
            } else {
                card
            }
        })
        .collect();
    let hero = fixture_deck("precon_tramplesaurus_rex").unwrap();
    let decks = vec![hero.cards.clone(), hero.cards.clone(), boss_deck];
    let commanders = vec![hero.commanders.clone(), hero.commanders.clone(), Vec::new()];
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    let setup = plan.apply(content, setup).unwrap();
    assert_eq!(
        setup.players[2].satchel.len() + setup.players[2].satchel_reusable.len(),
        4
    );
    Engine::new(setup).unwrap()
}

#[rstest]
fn a_level_five_fight_never_loops(#[values(0, 1, 2, 3, 4, 5, 6, 7)] at: usize) {
    let boss = boss(BOSSES[at]);
    let seed = 5;
    let mut driver = Driver::new(fight(boss, seed)).unwrap();
    let mut bots: Vec<RandomBot> = (0..3).map(|at| RandomBot::new(seed, at)).collect();
    for _ in 0..1500 {
        let Ok(decision) = driver.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose_in(&decision, driver.state());
        let Ok(events) = driver.answer(answer) else {
            break;
        };
        for event in &events {
            assert!(
                !matches!(event, Event::LoopDrawn { .. }),
                "{} drew by a loop",
                boss.id
            );
            if let Event::LoopRepeated(summary) = event
                && summary.player.is_some_and(|p| p.index() == BOSS)
            {
                assert!(summary.iterations <= BOSS_LOOP_CAP, "{}", boss.id);
            }
        }
    }
    // The fight got going: the boss took turns, and its innate is in its command zone.
    let state = driver.state();
    assert!(
        state.turn().number() >= 6,
        "{}: turn {}",
        boss.id,
        state.turn().number()
    );
    assert!(
        state.zone(Zone::Command(pid(BOSS))).iter().any(|id| state
            .object(*id)
            .is_some_and(|o| o.card().name.ends_with(" V"))),
        "{}",
        boss.id
    );
}
