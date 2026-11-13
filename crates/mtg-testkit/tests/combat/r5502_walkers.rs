//! Goal: "Whenever one or more creatures attack one of your opponents or a planeswalker they control, those creatures gain
//! menace until end of turn." plays as printed at tables of two, three and four seats: Frontier Warmonger answers an
//! attack on one of its controller's opponents and on a planeswalker that opponent controls, gives menace to those
//! attackers only, ignores an attack on its own controller, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::Answer;
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat `holder` controls Frontier Warmonger; seat 0 attacks seat 1 with the Wolf and seat 1's planeswalker with the Ox
/// (each when asked); the Bear stays home. Says which of them have menace once the trigger is done.
#[rstest]
#[case::its_controller_attacks_the_player(0, true, false, [true, false, false])]
#[case::its_controller_attacks_the_planeswalker(0, false, true, [false, true, false])]
#[case::its_controller_attacks_both(0, true, true, [true, true, false])]
#[case::an_attack_on_its_controller(1, true, true, [false, false, false])]
#[case::another_seats_opponent(2, true, true, [true, true, false])]
fn r5502_frontier_warmonger_gives_menace_to_those_attackers(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: usize,
    #[case] at_player: bool,
    #[case] at_walker: bool,
    #[case] menace: [bool; 3],
) -> Result<(), ScriptError> {
    if holder >= seats {
        return Ok(());
    }
    let mut attackers = vec![
        ready(free_creature("Wolf", 2, 2)),
        ready(free_creature("Ox", 2, 2)),
        ready(free_creature("Bear", 2, 2)),
    ];
    let warmonger = ready(card("Frontier Warmonger").unwrap());
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    let mut walkers = vec![ready(walker("Walker", 3))];
    match holder {
        0 => attackers.push(warmonger),
        1 => walkers.push(warmonger),
        _ => scenario = scenario.battlefield(holder, [warmonger]),
    }
    let scenario = scenario.battlefield(0, attackers).battlefield(1, walkers);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut declared = Vec::new();
    if at_player {
        declared.push((driver.id_of("Wolf")?, AttackTarget::Player(PlayerId(1))));
    }
    if at_walker {
        let walker = driver.id_of("Walker")?;
        declared.push((driver.id_of("Ox")?, AttackTarget::Planeswalker(walker)));
    }
    driver.answer(Answer::DeclareAttackers(declared))?;
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    let state = driver.state();
    for (name, wanted) in ["Wolf", "Ox", "Bear"].into_iter().zip(menace) {
        let id = driver.id_of(name)?;
        assert_eq!(has_keyword(state, id, &Keyword::Menace), wanted, "{name}");
    }
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
