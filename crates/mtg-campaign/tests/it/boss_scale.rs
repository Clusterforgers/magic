//! Goal: from generation `BOSS_SCALE_FROM` a boss brings relics from the first fight, more with depth and Heat, and one
//! reinforcement for each hero after the first (a solo hero meets the same boss as before); runs of older generations
//! keep the relic count and the boss they were played with; from `THEME_RELICS_FROM` a theme may add relics to its
//! bosses.

use mtg_campaign::boss_relics::{self, MOST_BOSS_RELICS, RELIC_OFFSET};
use mtg_campaign::content::FOUNDRY_FROM;
use mtg_campaign::run::{BOSS_SCALE_FROM, CONTENT, THEME_RELICS_FROM};
use mtg_campaign::scale::REINFORCEMENT;
use mtg_campaign::{CardKind, HeroDeck, Run, RunEvent, Scale, content};
use proptest::prelude::*;
use rstest::rstest;

/// The relic count of the newest generation at depths 0 to 4, one row per Heat.
const EARLY: [[usize; 5]; 4] = [
    [0, 1, 1, 2, 2],
    [1, 1, 2, 2, 3],
    [1, 2, 2, 3, 3],
    [2, 2, 3, 3, 4],
];

#[rstest]
fn a_boss_brings_relics_at_low_depth_by_heat(#[values(0, 1, 2, 3)] heat: u8) {
    for depth in 0..5u32 {
        assert_eq!(
            boss_relics::count(depth, heat, CONTENT),
            EARLY[usize::from(heat)][depth as usize],
            "Heat {heat}, depth {depth}"
        );
    }
}

proptest! {
    #[test]
    fn older_generations_keep_their_relic_count(depth in 0u32..40, heat in 0u8..4, generation in 0u32..BOSS_SCALE_FROM) {
        let old = usize::try_from(depth.saturating_sub(RELIC_OFFSET[usize::from(heat)])).unwrap().min(MOST_BOSS_RELICS);
        prop_assert_eq!(boss_relics::count(depth, heat, generation), old);
    }

    #[test]
    fn the_newest_count_never_falls_below_the_old_or_with_depth_and_heat(depth in 0u32..40, heat in 0u8..4) {
        let now = boss_relics::count(depth, heat, CONTENT);
        prop_assert!(now >= boss_relics::count(depth, heat, FOUNDRY_FROM));
        prop_assert!(boss_relics::count(depth + 1, heat, CONTENT) >= now);
        prop_assert!(boss_relics::count(depth + 1, heat, CONTENT) <= now + 1);
        if heat < 3 {
            prop_assert!(boss_relics::count(depth, heat + 1, CONTENT) >= now);
        }
        prop_assert!(now <= MOST_BOSS_RELICS);
    }
}

#[rstest]
#[case::solo(1, 0)]
#[case::pair(2, 1)]
#[case::three(3, 2)]
#[case::six(6, 5)]
#[case::beyond_the_most(9, 5)]
fn one_reinforcement_for_each_hero_after_the_first(#[case] heroes: usize, #[case] extra: usize) {
    assert_eq!(
        Scale::reinforcements(heroes, CONTENT),
        vec![REINFORCEMENT.to_owned(); extra]
    );
    assert_eq!(
        Scale::reinforcements(heroes, FOUNDRY_FROM),
        Vec::<String>::new()
    );
    let card = content().unwrap().card(REINFORCEMENT).unwrap();
    assert_eq!(card.kind, CardKind::Reinforcement);
}

/// A started run of content `version` with `heroes` heroes at Heat 1.
fn run(heroes: usize, version: u32) -> Run {
    let mut events = vec![RunEvent::Content { version }];
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
    Run::replay(content().unwrap(), "warrens", 21, 1, &events).unwrap()
}

#[rstest]
fn the_fight_plan_carries_the_reinforcements_by_party_size(
    #[values(1, 2, 3)] heroes: usize,
    #[values(FOUNDRY_FROM, CONTENT)] version: u32,
) {
    let content = content().unwrap();
    let boss = run(heroes, version)
        .fight_plan(content)
        .unwrap()
        .unwrap()
        .boss;
    let extra = if version >= BOSS_SCALE_FROM {
        heroes - 1
    } else {
        0
    };
    assert_eq!(boss.reinforcements.len(), extra);
    assert_eq!(boss.relics.len(), boss_relics::count(0, 1, version));
    assert_eq!(boss.life, Scale::boss_life(heroes));
    // Each reinforcement is an emblem from the start and named on the sheet beside the relics.
    let emblems = boss.emblems();
    assert_eq!(
        emblems.iter().filter(|id| *id == REINFORCEMENT).count(),
        extra
    );
    assert_eq!(
        boss.sheet(content).unwrap().relics.len(),
        boss.relics.len() + extra
    );
}

#[test]
fn a_solo_hero_meets_the_same_boss_apart_from_the_early_relics() {
    let content = content().unwrap();
    let old = run(1, FOUNDRY_FROM)
        .fight_plan(content)
        .unwrap()
        .unwrap()
        .boss;
    let mut now = run(1, CONTENT).fight_plan(content).unwrap().unwrap().boss;
    assert_eq!(now.reinforcements, Vec::<String>::new());
    now.relics.clone_from(&old.relics);
    assert_eq!(now, old);
}

proptest! {
    #[test]
    fn a_theme_adds_its_relics_only_from_its_generation(
        depth in 0u32..40,
        heat in 0u8..4,
        generation in 0u32..20,
        extra in 0u32..4,
    ) {
        let base = boss_relics::count(depth, heat, generation);
        let now = boss_relics::theme_count(depth, heat, generation, extra);
        if generation < THEME_RELICS_FROM {
            prop_assert_eq!(now, base);
        } else {
            prop_assert_eq!(now, (base + extra as usize).min(MOST_BOSS_RELICS));
        }
    }
}

/// A started run of the `theme` with two heroes at `heat` in content `version`, its first boss's plan.
fn first_boss(theme: &str, heat: u8, version: u32) -> mtg_campaign::BossSeat {
    let mut events = vec![RunEvent::Content { version }];
    for at in 0..2u8 {
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
    Run::replay(content, theme, 21, heat, &events)
        .unwrap()
        .fight_plan(content)
        .unwrap()
        .unwrap()
        .boss
}

#[rstest]
#[case::warrens("warrens", 1, 0)]
#[case::spire("spire", 1, 0)]
#[case::foundry("foundry", 3, 1)]
fn a_themes_bosses_bring_its_extra_relics_from_its_generation(
    #[case] theme: &str,
    #[case] heat: u8,
    #[case] extra: usize,
) {
    assert_eq!(
        content().unwrap().theme(theme).unwrap().boss_relics as usize,
        extra
    );
    let old = first_boss(theme, heat, BOSS_SCALE_FROM);
    let mut now = first_boss(theme, heat, CONTENT);
    assert_eq!(
        old.relics.len(),
        boss_relics::count(0, heat, BOSS_SCALE_FROM)
    );
    assert_eq!(now.relics.len(), old.relics.len() + extra);
    // Apart from the relics, the boss is the one of generation 12.
    now.relics.clone_from(&old.relics);
    assert_eq!(now, old);
}
