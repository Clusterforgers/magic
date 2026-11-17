//! Goal: the bracket generation of bosses holds together: every boss deck loads at every bracket with its size, at
//! most four copies of a card, cards of its colours that play automatically and the bracket's mana value cap; the
//! bracket table never makes a boss easier; no boss deck holds an infinite combo, and the boss's side stops any loop
//! after five iterations; the commander card waits in the command zone and is cast as the last phase begins in real
//! fights of two to four heroes; an upgraded boss can stand as the final boss; runs of an earlier generation keep their
//! ladder and decks.
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_campaign::content::{Boss, UPGRADED};
use mtg_campaign::map::{LIEUTENANTS, generate, generate_for};
use mtg_campaign::run::CONTENT;
use mtg_campaign::scale::{BRACKETS, BRACKETS_FROM, DECK_SIZE, MAX_BRACKET, MAX_HEAT, RANKS};
use mtg_campaign::{FightPlan, Run, RunEvent, Scale, content};
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::Answer;
use mtg_engine::events::Event;
use mtg_engine::format::Commander;
use mtg_engine::loops::{LoopAnswer, LoopLife, LoopOptions, LoopStep};
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::loop_answer::{BOSS_LOOP_CAP, loop_answer_in};
use mtg_testkit::{Driver, RandomBot};
use rstest::rstest;

/// The basic lands, which the engine knows without a rules file.
const BASICS: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
/// The highest mana value a deck card may have, by bracket from 1; the commander card is apart.
const MANA_VALUE_CAP: [u32; MAX_BRACKET as usize] = [5, 6, 7, 20, 20];
/// Known two-card infinite combos: a boss deck never holds both cards of one.
const COMBOS: [(&str, &str); 8] = [
    ("Kiki-Jiki, Mirror Breaker", "Pestermite"),
    ("Kiki-Jiki, Mirror Breaker", "Zealous Conscripts"),
    ("Sanguine Bond", "Exquisite Blood"),
    ("Gravecrawler", "Phyrexian Altar"),
    ("Nim Deathmantle", "Ashnod's Altar"),
    ("Mikaeus, the Unhallowed", "Triskelion"),
    ("Heliod, Sun-Crowned", "Walking Ballista"),
    ("Dualcaster Mage", "Twinflame"),
];

/// A deck card by name: a basic land from the testkit, anything else from the corpus.
fn card(name: &str) -> Arc<CardDefinition> {
    use mtg_core::color::Color;
    let basic = [
        ("Plains", Color::White),
        ("Island", Color::Blue),
        ("Swamp", Color::Black),
        ("Mountain", Color::Red),
        ("Forest", Color::Green),
    ];
    match basic.iter().find(|(basic, _)| *basic == name) {
        Some((_, color)) => mtg_testkit::cards::land(*color),
        None => corpus::card(name).unwrap(),
    }
}

/// Every boss of the theme that has bracket decks.
fn bracket_bosses() -> Vec<&'static Boss> {
    content().unwrap().themes[0]
        .bosses
        .iter()
        .filter(|b| b.brackets.is_some())
        .collect()
}

/// Asserts `name` plays automatically (Verified or Read) and its colour identity lies within `colors`.
fn automatic_in_colors(name: &str, colors: &str, boss: &str) {
    let support = corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap_or_else(|e| panic!("{boss}: {name}: {e}"));
    assert!(
        matches!(support.support, Support::Verified | Support::Read),
        "{boss}: {name} does not play automatically"
    );
    let identity = card(name).color_identity();
    for color in identity.iter() {
        use mtg_core::color::Color;
        let letter = match color {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        };
        assert!(
            colors.contains(letter),
            "{boss}: {name} is outside its colours {colors}"
        );
    }
}

#[test]
fn every_boss_deck_loads_at_every_bracket_and_keeps_its_rules() {
    let bosses = bracket_bosses();
    assert!(bosses.len() >= 6, "the four bosses and the new ones");
    for boss in bosses {
        assert!(!boss.commander.is_empty(), "{}: a commander card", boss.id);
        automatic_in_colors(&boss.commander, &boss.colors, &boss.id);
        for bracket in 1..=MAX_BRACKET {
            let deck = boss.deck_at(bracket);
            let at = format!("{} at bracket {bracket}", boss.id);
            let size: u32 = deck.iter().map(|(_, n)| n).sum();
            assert_eq!(size, DECK_SIZE[usize::from(bracket - 1)], "{at}");
            let lands: u32 = deck
                .iter()
                .filter(|(name, _)| BASICS.contains(&name.as_str()))
                .map(|(_, n)| n)
                .sum();
            assert!(
                lands * 100 >= size * 40 && lands * 100 <= size * 45,
                "{at}: {lands} lands"
            );
            let names: BTreeSet<&str> = deck.iter().map(|(name, _)| name.as_str()).collect();
            assert_eq!(names.len(), deck.len(), "{at}: a card named twice");
            assert!(
                !names.contains(boss.commander.as_str()),
                "{at}: commander in the deck"
            );
            for (name, copies) in deck.iter().filter(|(n, _)| !BASICS.contains(&n.as_str())) {
                assert!(*copies <= 4, "{at}: {copies} {name}");
                automatic_in_colors(name, &boss.colors, &at);
                let value = card(name).mana_value();
                assert!(
                    value <= MANA_VALUE_CAP[usize::from(bracket - 1)],
                    "{at}: {name} costs {value}"
                );
            }
            for (a, b) in COMBOS {
                assert!(
                    !(names.contains(a) && names.contains(b)),
                    "{at}: {a} and {b} loop"
                );
            }
        }
    }
}

#[test]
fn the_roster_covers_each_colour_once_on_its_own() {
    let mono: Vec<&str> = bracket_bosses()
        .iter()
        .filter(|b| b.colors.len() == 1)
        .map(|b| b.colors.as_str())
        .collect();
    let distinct: BTreeSet<&str> = mono.iter().copied().collect();
    assert_eq!(distinct.len(), mono.len(), "{mono:?}");
    assert_eq!(distinct, BTreeSet::from(["W", "U", "B", "R", "G"]));
}

#[rstest]
fn the_bracket_table_never_makes_a_boss_easier(
    #[values(1, 2, 3, 4, 5, 6)] heroes: usize,
    #[values(0, 1, 2)] heat: u8,
) {
    let boss = bracket_bosses()[0];
    let strength = |scale: Scale| {
        let tier = scale.tier();
        [
            i64::from(scale.bracket()),
            i64::from(scale.life(boss)),
            i64::from(scale.hand(boss)),
            scale.brain(boss.brain) as i64,
            i64::try_from(scale.minion_count()).unwrap(),
            i64::from(tier.relic),
            i64::from(tier.modifier),
            -i64::from(scale.enrage_turn()),
        ]
    };
    for rank in 0..=RANKS {
        let here = strength(Scale::of_content(heroes, heat, rank, CONTENT));
        let later = strength(Scale::of_content(heroes, heat, rank + 1, CONTENT));
        assert!(later.iter().zip(&here).all(|(a, b)| a >= b), "rank {rank}");
        if heat < MAX_HEAT {
            let hotter = strength(Scale::of_content(heroes, heat + 1, rank, CONTENT));
            assert!(hotter.iter().zip(&here).all(|(a, b)| a >= b), "rank {rank}");
        }
    }
    // The bracket carries the step: each Heat starts one bracket up and the final boss is two above the first.
    let row = BRACKETS[usize::from(heat)];
    assert_eq!(row, [heat + 1, heat + 2, heat + 2, heat + 3]);
    assert_eq!(
        Scale::of_content(heroes, heat, 0, CONTENT).bracket(),
        heat + 1
    );
    assert_eq!(Scale::new(heroes, heat, 0).bracket(), 0, "earlier runs");
}

/// A run of `heroes` at `heat` and content generation `version`, started, with its seed.
fn run(heroes: usize, heat: u8, seed: u64, version: u32) -> Run {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content { version }];
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(at).unwrap(),
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "G".into(),
                commanders: Vec::new(),
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content, "warrens", seed, heat, &events).unwrap()
}

/// The game of `plan`, the heroes on a fixture Commander deck.
fn engine(plan: &FightPlan, seed: u64) -> Engine {
    let hero = fixture_deck("precon_tramplesaurus_rex").unwrap();
    let heroes = plan.heroes.len();
    let mut decks: Vec<Vec<Arc<CardDefinition>>> =
        (0..heroes).map(|_| hero.cards.clone()).collect();
    let mut commanders: Vec<Vec<Arc<CardDefinition>>> =
        (0..heroes).map(|_| hero.commanders.clone()).collect();
    for deck in plan.boss_decks() {
        decks.push(
            deck.iter()
                .flat_map(|(name, n)| (0..*n).map(move |_| card(name)))
                .collect(),
        );
        commanders.push(Vec::new());
    }
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    Engine::new(plan.apply(content().unwrap(), setup).unwrap()).unwrap()
}

#[rstest]
fn a_new_run_plays_bracket_decks_and_an_old_one_its_decks_as_written(#[values(0, 1, 2)] heat: u8) {
    let content = content().unwrap();
    let theme = &content.themes[0];
    let new = run(2, heat, 31, CONTENT);
    let old = run(2, heat, 31, 1);
    assert_eq!(old.map, generate(theme, 31), "an old run keeps its ladder");
    assert_eq!(new.map, generate_for(theme, 31, CONTENT));
    for (rank, node) in new
        .map
        .nodes
        .iter()
        .filter(|n| n.boss.is_some())
        .enumerate()
    {
        let mut at = new.clone();
        at.at = Some(node.id);
        at.status = mtg_campaign::Status::Fight { game: None };
        let plan = at.fight_plan(content).unwrap().unwrap();
        let boss = theme.boss(node.boss.as_deref().unwrap()).unwrap();
        let bracket = BRACKETS[usize::from(heat)][rank.min(RANKS - 1)];
        assert_eq!(plan.boss.deck, boss.deck_at(bracket), "{}", boss.id);
        assert_eq!(
            plan.boss.commander.as_deref(),
            Some(boss.commander.as_str())
        );
    }
    for node in old.map.nodes.iter().filter(|n| n.boss.is_some()) {
        let mut at = old.clone();
        at.at = Some(node.id);
        at.status = mtg_campaign::Status::Fight { game: None };
        let plan = at.fight_plan(content).unwrap().unwrap();
        let boss = theme.boss(node.boss.as_deref().unwrap()).unwrap();
        assert_eq!(boss.since, 0, "an old run meets only the old bosses");
        assert_eq!(plan.boss.deck, boss.deck);
        assert_eq!(plan.boss.commander, None);
    }
}

#[rstest]
fn the_commander_card_waits_and_is_cast_as_the_last_phase_begins(#[values(2, 3, 4)] heroes: usize) {
    let content = content().unwrap();
    let run = run(heroes, 1, 40 + heroes as u64, CONTENT);
    let mut plan = run.fight_plan_at(content, 2).unwrap().unwrap();
    let commander = plan.boss.commander.clone().unwrap();
    let boss = plan.boss_seat();
    // The boss stands at its last phase's threshold, so the phases begin at the first check.
    let last = plan
        .boss
        .phases
        .iter()
        .rfind(|(id, _)| id != mtg_campaign::scale::BULWARK)
        .unwrap()
        .1;
    plan.boss.life = last;
    let engine = engine(&plan, 7);
    let held: Vec<_> = engine
        .state()
        .zone(Zone::Command(boss))
        .iter()
        .filter_map(|id| engine.state().object(*id))
        .filter(|o| o.card().name == commander)
        .map(mtg_engine::object::GameObject::kind)
        .collect();
    assert_eq!(held, vec![ObjectKind::Card], "{commander} waits as a card");
    let library = engine.state().zone(Zone::Library(boss)).len()
        + engine.state().zone(Zone::Hand(boss)).len();
    assert_eq!(
        library,
        plan.boss
            .deck
            .iter()
            .map(|(_, n)| *n as usize)
            .sum::<usize>()
    );
    let mut driver = Driver::new(engine).unwrap();
    let mut bots: Vec<RandomBot> = (0..plan.seats())
        .map(|at| RandomBot::new(7, at as u64))
        .collect();
    let mut cast = false;
    for _ in 0..3000 {
        let Ok(decision) = driver.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose_in(&decision, driver.state());
        let events = driver.answer(answer).unwrap();
        cast = events.iter().any(|event| {
            matches!(event, Event::SpellCast { player, object, .. }
                if *player == boss
                    && driver.state().object(*object).is_some_and(|o| o.card().name == commander))
        });
        if cast {
            break;
        }
    }
    assert!(cast, "{} casts {commander}", plan.boss.name);
    assert!(
        !driver
            .state()
            .zone(Zone::Command(boss))
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .any(|o| o.card().name == commander)
    );
}

#[test]
fn the_boss_side_stops_any_loop_after_five_iterations() {
    let content = content().unwrap();
    let run = run(2, 1, 5, CONTENT);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let engine = engine(&plan, 3);
    let state = engine.state();
    let boss = plan.boss_seat();
    let hero = PlayerId::from_index(0);
    let options = |seat: PlayerId, can_draw: bool, bounded: bool| LoopOptions {
        cards: vec!["A loop".into()],
        per_iteration: LoopStep {
            tokens: Vec::new(),
            life: vec![LoopLife {
                player: seat,
                change: 1,
            }],
        },
        mandatory: can_draw,
        bounded,
        max: 10_000,
        asked: 0,
        can_draw,
    };
    let cap = Answer::Loop(LoopAnswer::Repeat(BOSS_LOOP_CAP));
    assert_eq!(BOSS_LOOP_CAP, 5);
    assert_eq!(
        loop_answer_in(state, boss, &options(boss, false, false)),
        cap
    );
    assert_eq!(
        loop_answer_in(state, boss, &options(boss, false, true)),
        cap
    );
    // A mandatory loop that never ends is no draw for the boss: it runs five times.
    assert_eq!(
        loop_answer_in(state, boss, &options(boss, true, false)),
        cap
    );
    // A loop worth nothing to the boss is stopped.
    assert_eq!(
        loop_answer_in(state, boss, &options(hero, false, false)),
        Answer::Loop(LoopAnswer::Stop)
    );
    // A hero's bot is not capped.
    assert_eq!(
        loop_answer_in(state, hero, &options(hero, false, false)),
        Answer::Loop(LoopAnswer::Repeat(mtg_testkit::loop_answer::GROWING))
    );
    for at in 0..plan.minions.len() {
        let minion = plan.minion_seat(at);
        assert_eq!(
            loop_answer_in(state, minion, &options(minion, false, false)),
            cap
        );
    }
}

#[rstest]
fn no_boss_deck_loops_in_a_simulated_fight(#[values(1, 2, 3, 4, 5)] bracket: u8) {
    let content = content().unwrap();
    let base = run(2, 0, 11, CONTENT);
    let template = base.fight_plan(content).unwrap().unwrap();
    for boss in bracket_bosses() {
        let mut plan = template.clone();
        plan.boss.deck = boss.deck_at(bracket);
        plan.boss.commander = Some(boss.commander.clone());
        let mut driver = Driver::new(engine(&plan, u64::from(bracket))).unwrap();
        let mut bots: Vec<RandomBot> = (0..plan.seats())
            .map(|at| RandomBot::new(u64::from(bracket), at as u64))
            .collect();
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
                    "{} at bracket {bracket} drew by a loop",
                    boss.id
                );
                if let Event::LoopRepeated(summary) = event
                    && summary
                        .player
                        .is_some_and(|p| p.index() >= plan.heroes.len())
                {
                    assert!(summary.iterations <= BOSS_LOOP_CAP, "{}", boss.id);
                }
            }
        }
    }
}

#[test]
fn an_upgraded_boss_stands_as_the_final_boss_of_a_ladder_it_is_not_on() {
    let content = content().unwrap();
    let theme = &content.themes[0];
    assert_ne!(theme.upgraded.len(), 0);
    for upgraded in &theme.upgraded {
        let base = theme.boss(upgraded.identity()).unwrap();
        assert_eq!(upgraded.id, format!("{}{UPGRADED}", base.id));
        assert!(upgraded.final_boss && upgraded.since >= BRACKETS_FROM);
        assert_eq!(upgraded.rules[..base.rules.len()], base.rules[..]);
        assert!(upgraded.rules.len() > base.rules.len(), "an extra effect");
        assert_eq!(upgraded.brackets, base.brackets);
        assert_eq!(upgraded.commander, base.commander);
    }
    let mut finals = BTreeSet::new();
    for seed in 0..200u64 {
        let map = generate_for(theme, seed, CONTENT);
        let bosses: Vec<&str> = map.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
        assert_eq!(bosses.len(), LIEUTENANTS + 1, "seed {seed}");
        let last = theme.boss(bosses[LIEUTENANTS]).unwrap();
        assert!(last.final_boss);
        let identities: BTreeSet<&str> = bosses
            .iter()
            .map(|id| theme.boss(id).unwrap().identity())
            .collect();
        assert_eq!(identities.len(), bosses.len(), "seed {seed}: a boss twice");
        finals.insert(last.id.clone());
        if last.upgrade_of.is_some() {
            // A fight against it plays its own name, the extra card and the base boss's deck at the final bracket.
            let mut run = run(3, 2, seed, CONTENT);
            run.at = map
                .nodes
                .iter()
                .find(|n| n.boss.as_deref() == Some(&last.id))
                .map(|n| n.id);
            run.status = mtg_campaign::Status::Fight { game: None };
            let plan = run.fight_plan(content).unwrap().unwrap();
            assert_eq!(plan.boss.name, last.name);
            assert!(
                plan.boss
                    .rules
                    .iter()
                    .any(|id| last.rules[1..].contains(id))
            );
            let base = theme.boss(last.identity()).unwrap();
            assert_eq!(plan.boss.deck, base.deck_at(BRACKETS[2][RANKS - 1]));
            assert!(plan.minions.iter().all(|m| {
                theme
                    .minions
                    .iter()
                    .find(|x| x.id == m.minion)
                    .unwrap()
                    .deck_of
                    != base.id
            }));
        }
    }
    assert!(
        finals.iter().any(|id| id.ends_with(UPGRADED)) && finals.contains("grimgut_court"),
        "{finals:?}"
    );
}
