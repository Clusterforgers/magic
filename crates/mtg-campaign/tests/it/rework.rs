//! Goal: the reworked boss follows the design's tables: the innate level and the relic count by depth and Heat, the
//! life by party size, the rare and mythic odds, all never falling as depth or Heat grow; innates load as five level
//! cards; an encounter modifier is never drawn for a boss it must not meet and a retried fight keeps it; a boss never
//! brings a relic that names teammates; a fight plan of the rework generation holds exactly the reworked parts; and the
//! boss seat carries the sheet that names them, its innate at its level.

use mtg_campaign::boss_relics::{self, MOST_BOSS_RELICS};
use mtg_campaign::content::{Content, FOUNDRY_FROM, Rarity};
use mtg_campaign::run::CONTENT;
use mtg_campaign::modifiers::{self, EncounterModifier};
use mtg_campaign::relics::DeckMakeup;
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_campaign::{Brain, Run, RunEvent, Scale, content};
use proptest::prelude::*;
use rstest::rstest;

/// The design's innate level table: one row per Heat over depths 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 20.
const LEVELS: [[u8; 14]; 3] = [
    [1, 1, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5],
    [1, 2, 2, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5],
    [2, 2, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5],
];
/// The relic count table of generations before `BOSS_SCALE_FROM`, same columns.
const RELICS: [[usize; 14]; 3] = [
    [0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 6, 6, 6],
    [0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 6, 6, 6, 6],
    [0, 0, 0, 1, 2, 3, 4, 5, 6, 6, 6, 6, 6, 6],
];
const DEPTHS: [u32; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 20];

#[rstest]
fn innate_level_and_relic_count_follow_the_tables(#[values(0, 1, 2)] heat: u8) {
    for (at, depth) in DEPTHS.into_iter().enumerate() {
        let row = usize::from(heat);
        assert_eq!(
            Scale::innate_level(depth, heat),
            LEVELS[row][at],
            "depth {depth}"
        );
        assert_eq!(
            boss_relics::count(depth, heat, FOUNDRY_FROM),
            RELICS[row][at],
            "depth {depth}"
        );
    }
}

#[rstest]
#[case(1, 40)]
#[case(2, 55)]
#[case(4, 85)]
#[case(6, 115)]
fn life_grows_with_the_party_only(#[case] heroes: usize, #[case] life: i32) {
    assert_eq!(Scale::boss_life(heroes), life);
}

#[rstest]
#[case::one_heat_0(1, 0, (10, 0))]
#[case::six_heat_0(6, 0, (60, 0))]
#[case::three_heat_1(3, 1, (35, 0))]
#[case::four_heat_1(4, 1, (45, 5))]
#[case::six_heat_1(6, 1, (60, 15))]
#[case::five_heat_2(5, 2, (60, 10))]
fn rarity_odds_follow_the_table(#[case] count: usize, #[case] heat: u8, #[case] odds: (u32, u32)) {
    assert_eq!(boss_relics::odds(count, heat), odds);
}

proptest! {
    #[test]
    fn deeper_or_hotter_never_weakens_the_boss(depth in 0u32..40, heat in 0u8..3) {
        let deeper = depth + 1;
        prop_assert!(Scale::innate_level(deeper, heat) >= Scale::innate_level(depth, heat));
        prop_assert!(boss_relics::count(deeper, heat, CONTENT) >= boss_relics::count(depth, heat, CONTENT));
        prop_assert!(boss_relics::count(depth, heat, CONTENT) <= MOST_BOSS_RELICS);
        if heat < 2 {
            prop_assert!(Scale::innate_level(depth, heat + 1) >= Scale::innate_level(depth, heat));
            prop_assert!(boss_relics::count(depth, heat + 1, CONTENT) >= boss_relics::count(depth, heat, CONTENT));
        }
        let n = boss_relics::count(depth, heat, CONTENT).max(1);
        let (rare, mythic) = boss_relics::odds(n, heat);
        let (rare_more, mythic_more) = boss_relics::odds((n + 1).min(MOST_BOSS_RELICS), heat);
        prop_assert!(rare_more >= rare && mythic_more >= mythic);
    }

    #[test]
    fn a_modifier_never_meets_a_boss_it_must_not(seed in any::<u64>(), floor in 1u8..5, room in 0u8..12) {
        let pool = content().unwrap().modifiers.clone();
        for boss in ["vex_crypt", "skrix_nest"] {
            let drawn = modifiers::draw(&pool, boss, seed, floor, room).unwrap();
            prop_assert!(drawn.fits(boss));
            // A retried fight keeps its modifier.
            prop_assert_eq!(modifiers::draw(&pool, boss, seed, floor, room), Some(drawn));
        }
    }
}

#[test]
fn no_modifier_when_none_fits() {
    let pool = vec![EncounterModifier {
        id: "x".into(),
        not_with: vec!["vex_crypt".into()],
    }];
    assert!(modifiers::draw(&pool, "vex_crypt", 1, 1, 1).is_none());
    assert!(modifiers::draw(&pool, "skrix_nest", 1, 1, 1).is_some());
}

/// An innate written with its five numbers.
fn innate(levels: &str) -> String {
    format!(
        r#"[{{"id":"brood","name":"Brood","kind":"Innate","type_line":"Boss — Innate","levels":{levels},
        "text":"At the beginning of your upkeep, you gain {{N}} life.",
        "abilities":[{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{{"GainLife":{{"amount":"$N"}}}}]}}}}]}}]"#
    )
}

#[test]
fn an_innate_loads_as_five_level_cards() {
    let content = Content::parse(&[&innate("[1, 2, 4, 6, 9]")], &[], "[]").unwrap();
    let cards: Vec<(String, String, u8)> = content
        .cards
        .values()
        .map(|c| (c.id.clone(), c.text.clone(), c.level))
        .collect();
    let want = [
        ("i", 1, 1),
        ("ii", 2, 2),
        ("iii", 4, 3),
        ("iv", 6, 4),
        ("v", 9, 5),
    ];
    for (id, n, level) in want {
        let text = format!("At the beginning of your upkeep, you gain {n} life.");
        assert!(
            cards.contains(&(format!("brood_{id}"), text, level)),
            "{cards:?}"
        );
        let card = content.card(&format!("brood_{id}")).unwrap();
        let json = serde_json::to_string(&card.abilities).unwrap();
        assert!(json.contains(&format!("\"amount\":{n}")), "{json}");
    }
}

#[rstest]
#[case::falling("[1, 2, 1, 4, 5]")]
#[case::four("[1, 2, 3, 4]")]
fn an_innate_must_have_five_levels_that_never_fall(#[case] levels: &str) {
    assert!(Content::parse(&[&innate(levels)], &[], "[]").is_err());
}

#[rstest]
fn a_boss_never_brings_a_relic_naming_teammates(
    #[values(0, 1, 2)] heat: u8,
    #[values(5, 8, 12)] depth: u32,
    #[values(1, 2, 3)] seed: u64,
) {
    let content = content().unwrap();
    let relics = boss_relics::draw(
        content,
        &DeckMakeup::default(),
        CONTENT,
        (depth, heat),
        (seed, 2, 3),
    );
    assert_eq!(relics.len(), boss_relics::count(depth, heat, CONTENT));
    for id in &relics {
        let relic = content.card(id).unwrap();
        assert!(!relic.team, "{id}");
        assert!(heat > 0 || relic.rarity != Rarity::Mythic, "{id}");
    }
    let again = boss_relics::draw(
        content,
        &DeckMakeup::default(),
        CONTENT,
        (depth, heat),
        (seed, 2, 3),
    );
    assert_eq!(again, relics);
}

fn reworked_run(heroes: usize, heat: u8) -> Run {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content {
        version: CONTENT,
    }];
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
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content, "warrens", 21, heat, &events).unwrap()
}

#[rstest]
fn a_reworked_fight_plan_holds_only_the_reworked_parts(
    #[values(1, 2, 4)] heroes: usize,
    #[values(0, 1, 2)] heat: u8,
) {
    let content = content().unwrap();
    let run = reworked_run(heroes, heat);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let boss = &plan.boss;
    let rework = boss;
    assert_eq!(boss.life, Scale::boss_life(heroes));
    assert_eq!(boss.brain, Brain::Hard);
    assert!(rework.modifier.is_some());
    assert_eq!(rework.relics.len(), boss_relics::count(0, heat, CONTENT));
    let rounds: Vec<u32> = rework.round_phases.iter().map(|(_, r)| *r).collect();
    assert!(rounds.is_empty() || rounds == PHASE_ROUNDS);
    // The plan's emblems: the modifier and any relics, copies kept.
    assert_eq!(
        boss.emblems().len(),
        2 + rework.relics.len() + rework.reinforcements.len()
    );
}

#[rstest]
fn a_reworked_boss_seat_takes_the_opening_land_rule_and_its_emblems(#[values(1, 2)] heroes: usize) {
    use mtg_engine::GameSetup;
    use mtg_engine::format::Commander;
    use mtg_engine::opening_lands::OpeningLands;
    use std::sync::Arc;
    let content = content().unwrap();
    let plan = reworked_run(heroes, 2)
        .fight_plan(content)
        .unwrap()
        .unwrap();
    let land = || mtg_testkit::cards::land(mtg_core::color::Color::Green);
    let mut decks: Vec<Vec<_>> = (0..heroes)
        .map(|_| (0..40).map(|_| land()).collect())
        .collect();
    decks.push(crate::common::boss_deck(&plan));
    let setup = plan
        .apply(content, GameSetup::new(Arc::new(Commander), 4, decks))
        .unwrap();
    let boss = &setup.players[heroes];
    assert_eq!(boss.opening_lands, Some(OpeningLands::BOSS));
    assert!(
        setup.players[..heroes]
            .iter()
            .all(|p| p.opening_lands.is_none())
    );
    assert_eq!(boss.emblems.len(), plan.boss.emblems().len());
}

/// A boss seat whose innate is `innate` and whose relics are `relics`.
fn seat(innate: &str, relics: Vec<String>) -> mtg_campaign::BossSeat {
    mtg_campaign::BossSeat {
        boss: "b".into(),
        name: "B".into(),
        life: 40,
        hand: 7,
        brain: Brain::Hard,
        deck: Vec::new(),
        innate: innate.into(),
        modifier: None,
        relics,
        reinforcements: Vec::new(),
        round_phases: Vec::new(),
        satchel: Vec::new(),
    }
}

#[rstest]
#[case::level_one("brood_i", 1, "Brood I", 1)]
#[case::level_three("brood_iii", 3, "Brood III", 4)]
#[case::level_five("brood_v", 5, "Brood V", 9)]
fn the_sheet_names_the_innate_at_its_level(
    #[case] id: &str,
    #[case] level: u8,
    #[case] name: &str,
    #[case] n: u32,
) {
    let content = Content::parse(&[&innate("[1, 2, 4, 6, 9]")], &[], "[]").unwrap();
    let rework = seat(id, Vec::new());
    let sheet = rework.sheet(&content).unwrap();
    assert_eq!(sheet.level, level);
    let innate = sheet.innate.unwrap();
    assert_eq!(innate.name, name);
    assert_eq!(
        innate.text,
        format!("At the beginning of your upkeep, you gain {n} life.")
    );
    assert!(sheet.modifier.is_none() && sheet.relics.is_empty() && sheet.phases.is_empty());
    let unknown = seat(id, vec!["nowhere".into()]);
    assert!(unknown.sheet(&content).is_err());
}

#[rstest]
fn a_reworked_boss_seat_carries_its_sheet(
    #[values(1, 2, 4)] heroes: usize,
    #[values(0, 2)] heat: u8,
) {
    use mtg_engine::GameSetup;
    use mtg_engine::format::Commander;
    use std::sync::Arc;
    let content = content().unwrap();
    let plan = reworked_run(heroes, heat)
        .fight_plan(content)
        .unwrap()
        .unwrap();
    let rework = &plan.boss;
    let land = || mtg_testkit::cards::land(mtg_core::color::Color::Green);
    let mut decks: Vec<Vec<_>> = (0..heroes)
        .map(|_| (0..40).map(|_| land()).collect())
        .collect();
    decks.push(crate::common::boss_deck(&plan));
    let setup = plan
        .apply(content, GameSetup::new(Arc::new(Commander), 4, decks))
        .unwrap();
    let sheet = setup.players[heroes].boss_sheet.as_deref().unwrap();
    assert_eq!(*sheet, rework.sheet(content).unwrap());
    let modifier = content.card(rework.modifier.as_deref().unwrap()).unwrap();
    assert_eq!(sheet.modifier.as_ref().unwrap().name, modifier.name);
    assert_eq!(
        sheet.relics.len(),
        rework.relics.len() + rework.reinforcements.len()
    );
    assert_eq!(sheet.phases.len(), rework.round_phases.len());
    assert!(
        setup.players[..heroes]
            .iter()
            .all(|p| p.boss_sheet.is_none())
    );
}
