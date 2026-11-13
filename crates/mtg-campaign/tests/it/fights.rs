//! Goal: a fight plan becomes a playable game: the boss's life grows with the party and the Heat, its phases sit at
//! their share of that life, every seat starts with its life, hand and emblems, and bots can play the game.
use std::sync::Arc;

use mtg_campaign::{Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_engine::format::Commander;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use rstest::rstest;

fn boss_deck(deck: &[(String, u32)]) -> Vec<Arc<CardDefinition>> {
    deck.iter()
        .flat_map(|(name, n)| (0..*n).map(move |_| corpus::card(name).unwrap()))
        .collect()
}

fn run_at_first_boss(heroes: usize, heat: u8) -> Run {
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
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "G".into(),
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content, "warrens", 21, heat, &events).unwrap()
}

#[rstest]
#[case(1, 0)]
#[case(2, 0)]
#[case(4, 0)]
#[case(2, 1)]
fn the_boss_scales_with_the_party_and_the_heat(#[case] heroes: usize, #[case] heat: u8) {
    let content = content().unwrap();
    let run = run_at_first_boss(heroes, heat);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let boss = content.themes[0].boss(&plan.boss.boss).unwrap();
    let mut life = boss.life + boss.life_per_hero * i32::try_from(heroes - 1).unwrap();
    if heat >= 1 {
        life += life / 5;
    }
    assert_eq!(plan.boss.life, life);
    for ((_, at), (_, percent)) in plan.boss.phases.iter().zip(&boss.phases) {
        assert_eq!(*at, life * i32::try_from(*percent).unwrap() / 100);
    }
    let rules = plan.house_rules(&mtg_core::house::HouseRules::default());
    assert_eq!(rules.handicaps.len(), heroes + 1);
    assert_eq!(
        rules
            .teams
            .unwrap()
            .seats
            .iter()
            .filter(|t| **t == 1)
            .count(),
        1
    );
}

#[test]
fn a_planned_fight_starts_as_a_game_with_its_emblems() {
    let content = content().unwrap();
    let run = run_at_first_boss(2, 0);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let hero = fixture_deck("precon_tramplesaurus_rex").unwrap();
    let mut decks: Vec<Vec<Arc<CardDefinition>>> = (0..2).map(|_| hero.cards.clone()).collect();
    decks.push(boss_deck(&plan.boss.deck));
    let mut commanders: Vec<Vec<Arc<CardDefinition>>> =
        (0..2).map(|_| hero.commanders.clone()).collect();
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), 4, decks).with_commanders(commanders);
    let setup = plan.apply(content, setup).unwrap();
    let engine = Engine::new(setup).unwrap();
    let state = engine.state();
    let boss = plan.boss_seat();
    let boss_emblems: Vec<String> = state
        .zone(Zone::Command(boss))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind() == ObjectKind::Emblem)
        .map(|o| o.card().name.clone())
        .collect();
    let expected: Vec<String> = plan
        .boss
        .rules
        .iter()
        .map(|id| content.card(id).unwrap().name.clone())
        .collect();
    assert_eq!(boss_emblems, expected);
    assert_eq!(state.player(boss).unwrap().life(), plan.boss.life);
    assert_eq!(
        state.player(boss).unwrap().phases().0,
        plan.boss.phases.len()
    );
}
