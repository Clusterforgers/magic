//! Goal: the heuristic levels use activated abilities in their windows (loyalty in the main phase, tap and mana-sink
//! abilities at an opponent's end step, a loot aimed at the seat itself, crews and animations before combat), and a
//! judgement the budget cuts short still finds the play instead of passing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_testkit::brain::suite::{Case, cases, run};
use mtg_testkit::brain::{HeuristicBot, Level, Settings, brain};
use rstest::rstest;

fn case(name: &str) -> Case {
    cases()
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

#[rstest]
fn each_level_uses_the_ability(
    #[values(Level::Normal, Level::Hard)] level: Level,
    #[values(
        "garruk_uses_its_loyalty",
        "jace_uses_its_loyalty",
        "sorin_uses_its_loyalty",
        "ajani_uses_its_loyalty",
        "cast_and_use_the_walker",
        "ping_the_x1_at_their_end_step",
        "make_a_snake_at_their_end_step",
        "loot_at_their_end_step",
        "equip_before_combat",
        "crew_the_copter_and_attack"
    )]
    name: &str,
) {
    let mut bot = brain(level, 7, PlayerId::from_index(0), false);
    let verdict = run(&case(name), bot.as_mut()).unwrap();
    assert!(verdict.passed, "{level:?} {name}");
}

#[rstest]
#[case::normal(Settings::NORMAL)]
#[case::hard(Settings::HARD)]
fn a_judgement_cut_short_still_uses_the_ability(
    #[case] settings: Settings,
    #[values(
        "garruk_uses_its_loyalty",
        "jace_uses_its_loyalty",
        "make_a_snake_at_their_end_step",
        "animate_the_manland_and_attack"
    )]
    name: &str,
) {
    // One simulation: the pass takes it and no play is judged in time.
    let starved = Settings {
        sims: 1,
        deep: None,
        endgame: false,
        ..settings.untimed()
    };
    let mut bot = HeuristicBot::new(7, PlayerId::from_index(0), starved);
    let verdict = run(&case(name), &mut bot).unwrap();
    assert!(verdict.passed, "{name}");
}
