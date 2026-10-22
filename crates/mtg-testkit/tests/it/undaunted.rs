//! Goal: undaunted plays as the rules say at two to four seats. A spell with it costs {1} less for each opponent still in the game, never less than no generic mana and never a coloured symbol, and it stands beside the other cost modifiers (an increase first, then the reductions); the spell is cast with exactly the reduced mana and resolves as it reads.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::choose::Target;
use mtg_engine::object::PermanentState;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::cost_cards::modifier;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds `spell` and `lands` lands of its colour, two Bears and the modifier permanents `mine` in play; seat 1
/// holds the modifiers `theirs` and every other seat one Bear; the seats in `gone` have conceded.
fn table(
    seats: usize,
    spell: &str,
    lands: usize,
    mine: &[&str],
    theirs: &[&str],
    gone: &[usize],
) -> Driver {
    let land = || match spell {
        "Coastal Breach" => ready(island()),
        "Curtains' Call" => ready(swamp()),
        _ => ready(plains()),
    };
    let bear = || ready(creature("Bear", 1, &[Color::Green], 2, 2));
    let mut scenario = Scenario::new(seats).hand(0, [card(spell).unwrap()]).battlefield(
        0,
        (0..lands)
            .map(|_| land())
            .chain(mine.iter().map(|name| ready(modifier(name).unwrap())))
            .chain([bear(), bear()])
            .collect::<Vec<_>>(),
    );
    for seat in 1..seats {
        let own: &[&str] = if seat == 1 { theirs } else { &[] };
        scenario = scenario.battlefield(
            seat,
            own.iter()
                .map(|name| ready(modifier(name).unwrap()))
                .chain([bear()])
                .collect::<Vec<_>>(),
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for seat in gone {
        driver.concede(pid(*seat)).unwrap();
    }
    driver
}

/// The cost the payment asks for when `spell` is cast.
fn announced(driver: &mut Driver, spell: &str) -> String {
    driver.begin_cast(spell).unwrap();
    driver.pay_options().unwrap().cost.to_string()
}

fn bears(driver: &Driver) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == "Bear"))
        .collect()
}

#[rstest]
#[case::one_opponent("Sublime Exhalation", 2, &[], &[], &[], "{5}{W}")]
#[case::two_opponents("Sublime Exhalation", 3, &[], &[], &[], "{4}{W}")]
#[case::three_opponents("Sublime Exhalation", 4, &[], &[], &[], "{3}{W}")]
#[case::another_undaunted_spell("Coastal Breach", 4, &[], &[], &[], "{3}{U}")]
#[case::an_opponent_who_conceded_is_not_counted("Sublime Exhalation", 4, &[], &[], &[3], "{4}{W}")]
#[case::two_opponents_gone("Sublime Exhalation", 4, &[], &[], &[2, 3], "{5}{W}")]
#[case::an_increase_of_mine_comes_first("Sublime Exhalation", 4, &["Sphere"], &[], &[], "{4}{W}")]
#[case::an_increase_of_theirs_applies_too("Sublime Exhalation", 4, &[], &["Sphere"], &[], "{4}{W}")]
#[case::another_reduction_adds_up("Sublime Exhalation", 4, &["Helm"], &[], &[], "{2}{W}")]
#[case::an_increase_then_reductions("Sublime Exhalation", 3, &["Sphere", "Helm"], &[], &[], "{4}{W}")]
#[case::never_below_zero_generic("Coastal Breach", 4, &["Helm"; 4], &[], &[], "{U}")]
#[case::the_colour_stays("Curtains' Call", 4, &["Helm"; 6], &[], &[], "{B}")]
fn the_cost_is_one_less_for_each_opponent_still_in_the_game(
    #[case] spell: &str,
    #[case] seats: usize,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] gone: &[usize],
    #[case] expected: &str,
) {
    let mut driver = table(seats, spell, 12, mine, theirs, gone);
    if spell == "Curtains' Call" {
        driver.begin_cast(spell).unwrap();
        let targets: Vec<Target> = bears(&driver)
            .iter()
            .take(2)
            .map(|id| Target::Object(driver.state().object(*id).unwrap().reference()))
            .collect();
        driver.choose_targets(&targets).unwrap();
        assert_eq!(driver.pay_options().unwrap().cost.to_string(), expected);
    } else {
        assert_eq!(announced(&mut driver, spell), expected);
    }
}

#[rstest]
fn the_spell_is_castable_with_exactly_the_reduced_mana(#[values(2, 3, 4)] seats: usize) {
    // {6}{W} less one for each opponent.
    let enough = 7 - (seats - 1);
    let castable = |lands: usize| {
        let driver = table(seats, "Sublime Exhalation", lands, &[], &[], &[]);
        match &driver.decision().unwrap().kind {
            DecisionKind::Priority(options) => !options.castable.is_empty(),
            other => panic!("priority was expected, not {}", other.name()),
        }
    };
    assert!(castable(enough));
    assert!(!castable(enough - 1));
}

#[rstest]
fn the_spell_resolves_as_it_reads_after_paying_the_reduced_cost(#[values(2, 3, 4)] seats: usize) {
    let lands = 7 - (seats - 1);
    let mut driver = table(seats, "Sublime Exhalation", lands, &[], &[], &[]);
    driver.begin_cast("Sublime Exhalation").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let state = driver.state();
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name != "Bear")
        .filter(|object| object.permanent().is_some_and(PermanentState::tapped))
        .count();
    assert_eq!(tapped, lands, "every land pays for the {{W}} and the rest");
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    assert!(driver.state().stack().is_empty());
    assert!(bears(&driver).is_empty(), "all creatures are destroyed");
}

#[rstest]
fn a_spell_with_targets_pays_the_reduced_cost_and_resolves(#[values(2, 3, 4)] seats: usize) {
    let lands = 6 - (seats - 1);
    let mut driver = table(seats, "Curtains' Call", lands, &[], &[], &[]);
    let before = bears(&driver);
    assert_eq!(before.len(), seats + 1);
    driver.begin_cast("Curtains' Call").unwrap();
    // The two Bears of seat 1 and seat 0 or another seat: any two creatures.
    let targets: Vec<Target> = before
        .iter()
        .take(2)
        .map(|id| Target::Object(driver.state().object(*id).unwrap().reference()))
        .collect();
    driver.choose_targets(&targets).unwrap();
    assert_eq!(
        driver.pay_options().unwrap().cost.to_string(),
        format!("{{{}}}{{B}}", 5 - (seats - 1))
    );
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    assert_eq!(bears(&driver).len(), seats - 1, "two creatures were destroyed");
}
