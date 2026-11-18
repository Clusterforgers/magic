//! Goal: every new run plays the one boss system: at every Heat and party size from one to six, each theme's first
//! fight deals the boss its 100-card precon with its satchel set apart, the opening-hand land rule, its innate,
//! modifier and two phases by round, life by party size only, and a fight plan whose JSON names no part of the
//! removed systems (schemes, minions, commander card, life phases); the Warrens' final is Grimgut or the Unmaker.

use std::sync::Arc;

use mtg_campaign::run::CONTENT;
use mtg_campaign::scale::{MAX_HEAT, PHASE_ROUNDS};
use mtg_campaign::{Brain, HeroDeck, Run, RunEvent, Scale, content};
use mtg_engine::GameSetup;
use mtg_engine::format::Commander;
use mtg_engine::opening_lands::OpeningLands;
use rstest::rstest;

/// A new run of `heroes` heroes at `heat` drawn by `seed`, started.
fn started(seed: u64, heroes: usize, heat: u8) -> Run {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content { version: CONTENT }];
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
                colors: "G".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    let theme = content.theme_for(seed, CONTENT).unwrap();
    Run::replay(content, &theme.id, seed, heat, &events).unwrap()
}

/// Keys of the removed boss systems that no plan may carry.
const GONE: [&str; 8] = [
    "minions",
    "schemes",
    "scheme_every",
    "commander",
    "rules",
    "\"phases\"",
    "enrage",
    "rework",
];

#[rstest]
fn a_new_run_builds_the_reworked_fight(
    #[values(1, 2, 3, 4, 5, 6)] heroes: usize,
    #[values(0, 1, 2)] heat: u8,
    #[values(3, 8)] seed: u64,
) {
    assert!(heat <= MAX_HEAT);
    let content = content().unwrap();
    let run = started(seed, heroes, heat);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let boss = &plan.boss;
    assert_eq!(plan.seats(), heroes + 1);
    assert_eq!(
        (boss.life, boss.brain),
        (Scale::boss_life(heroes), Brain::Hard)
    );
    let spec = content.theme(&run.theme).unwrap().boss(&boss.boss).unwrap();
    let precon = content.deck(&spec.deck_file).unwrap();
    assert_eq!(boss.deck, precon.cards);
    assert_eq!(boss.deck.iter().map(|(_, n)| n).sum::<u32>(), 100);
    assert_eq!(boss.satchel.len(), 4);
    assert!(boss.modifier.is_some());
    let rounds: Vec<u32> = boss.round_phases.iter().map(|(_, r)| *r).collect();
    assert_eq!(rounds, PHASE_ROUNDS);
    let json = serde_json::to_string(&plan).unwrap();
    for key in GONE {
        assert!(!json.contains(key), "{key} in {json}");
    }
    let land = || mtg_testkit::cards::land(mtg_core::color::Color::Green);
    let mut decks: Vec<Vec<_>> = (0..heroes)
        .map(|_| (0..40).map(|_| land()).collect())
        .collect();
    decks.push(crate::common::boss_deck(&plan));
    let setup = plan
        .apply(content, GameSetup::new(Arc::new(Commander), seed, decks))
        .unwrap();
    let seat = &setup.players[heroes];
    assert_eq!(seat.library.len(), 100);
    let reusable = boss.satchel.iter().filter(|s| s.reusable).count();
    assert_eq!(seat.satchel_reusable.len(), reusable);
    assert_eq!(seat.satchel.len(), 4 - reusable);
    assert_eq!(seat.opening_lands, Some(OpeningLands::BOSS));
    assert!(seat.held.is_empty());
    assert!(
        setup.players[..heroes]
            .iter()
            .all(|p| p.opening_lands.is_none())
    );
}

#[test]
fn the_warrens_draw_either_final_boss() {
    let content = content().unwrap();
    let warrens = content.theme("warrens").unwrap();
    let finals: Vec<&str> = warrens
        .bosses
        .iter()
        .filter(|b| b.final_boss)
        .map(|b| b.id.as_str())
        .collect();
    assert_eq!(finals, ["grimgut_court", "unmaker_void"]);
}
