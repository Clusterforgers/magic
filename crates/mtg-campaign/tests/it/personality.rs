//! Goal: every boss of every theme carries a personality from the content, each theme holding all three, and from
//! `PERSONALITY_FROM` its fight plan and its game seat carry it whatever the party's size; a run of an older generation
//! and a plan stored before personalities meet the Watchful boss they were played with.

use std::sync::Arc;

use mtg_campaign::run::{CONTENT, PARTY_SCALE_FROM, PERSONALITY_FROM};
use mtg_campaign::{FightPlan, HeroDeck, Run, RunEvent, content};
use mtg_core::personality::Personality;
use mtg_engine::GameSetup;
use mtg_engine::format::Commander;
use rstest::rstest;

#[rstest]
fn every_boss_has_a_personality_and_each_theme_all_three(
    #[values("warrens", "spire", "foundry", "gloamwood")] theme: &str,
) {
    let theme = content().unwrap().theme(theme).unwrap();
    for boss in &theme.bosses {
        assert!(!boss.personality.is_watchful(), "{} has none", boss.id);
    }
    for personality in &Personality::ALL[1..] {
        assert!(
            theme.bosses.iter().any(|b| b.personality == *personality),
            "{} has no {personality:?} boss",
            theme.id
        );
    }
}

/// The first fight of a run of the `theme` at `heat` in content `version` with `heroes` heroes, seeded by `seed`.
fn first_fight(theme: &str, heat: u8, version: u32, heroes: u8, seed: u64) -> FightPlan {
    let mut events = vec![RunEvent::Content { version }];
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: at,
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
    let content = content().unwrap();
    Run::replay(content, theme, seed, heat, &events)
        .unwrap()
        .fight_plan(content)
        .unwrap()
        .unwrap()
}

#[rstest]
fn the_plan_and_the_seat_carry_the_bosss_personality(
    #[values(("warrens", 1), ("spire", 1), ("foundry", 3), ("gloamwood", 1))] theme: (&str, u8),
    #[values(1, 2, 4, 6)] heroes: u8,
    #[values(7, 21)] seed: u64,
) {
    let (theme, heat) = theme;
    let content = content().unwrap();
    let plan = first_fight(theme, heat, CONTENT, heroes, seed);
    let boss = content
        .themes
        .iter()
        .flat_map(|t| &t.bosses)
        .find(|b| b.id == plan.boss.boss)
        .unwrap();
    assert_eq!(plan.boss.personality, boss.personality);
    let heroes = usize::from(heroes);
    let land = || mtg_testkit::cards::land(mtg_core::color::Color::Green);
    let mut decks: Vec<Vec<_>> = (0..heroes)
        .map(|_| (0..40).map(|_| land()).collect())
        .collect();
    decks.push(crate::common::boss_deck(&plan));
    let setup = plan
        .apply(content, GameSetup::new(Arc::new(Commander), 4, decks))
        .unwrap();
    assert_eq!(setup.players[heroes].personality, boss.personality);
    assert!(
        setup.players[..heroes]
            .iter()
            .all(|p| p.personality.is_watchful())
    );
}

#[rstest]
fn an_older_generation_meets_the_watchful_boss(#[values(1, 3, 6)] heroes: u8) {
    assert_eq!(PERSONALITY_FROM, PARTY_SCALE_FROM + 1);
    let plan = first_fight("warrens", 1, PARTY_SCALE_FROM, heroes, 21);
    assert_eq!(plan.boss.personality, Personality::Watchful);
}

#[test]
fn a_stored_plan_without_a_personality_reads_as_watchful() {
    let plan = first_fight("warrens", 1, CONTENT, 2, 21);
    let mut json = serde_json::to_value(&plan.boss).unwrap();
    let stored = json.as_object_mut().unwrap();
    assert!(stored.contains_key("personality"));
    stored.remove("personality");
    let read: mtg_campaign::BossSeat = serde_json::from_value(json).unwrap();
    assert_eq!(read.personality, Personality::Watchful);
}
