//! Goal: the turn structure runs every step in order for any table size, with the right draw, untap, mana-emptying and hand-size rules.

mod common;

use common::{names, pid, turn_logs};
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

const QUIET_TURN: [Step; 10] = [
    Step::Untap,
    Step::Upkeep,
    Step::Draw,
    Step::Main1,
    Step::BeginCombat,
    Step::DeclareAttackers,
    Step::EndCombat,
    Step::Main2,
    Step::End,
    Step::Cleanup,
];

#[rstest]
fn quiet_turns_visit_every_step_and_rotate_through_all_seats(
    #[values(2, 3, 4, 5, 6, 7, 8)] players: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(players).at(1, 0, Step::Untap))?;
    let wanted_turn = u32::try_from(players).unwrap_or(0) + 1;
    driver.idle_until(wanted_turn, Step::Upkeep)?;
    let turns = turn_logs(driver.engine().events(), pid(0), Step::Untap);
    assert_eq!(turns.len(), players + 1);
    for (i, turn) in turns.iter().take(players).enumerate() {
        assert_eq!(turn.active, pid(i), "turn {} belongs to seat {i}", i + 1);
        assert_eq!(turn.steps, QUIET_TURN, "steps of turn {}", i + 1);
    }
    assert_eq!(turns[players].active, pid(0), "turn order wraps around");
    Ok(())
}

#[rstest]
fn combat_steps_are_skipped_but_reported_when_nobody_can_attack(
    #[values(2, 5)] players: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(players).at(2, 1, Step::Main1))?;
    driver.idle_until(2, Step::Main2)?;
    driver.expect_event("NothingToDo(Attack) for the active player", |e| {
        matches!(e, Event::NothingToDo { player, kind: mtg_engine::events::NothingToDo::Attack } if *player == pid(1))
    })?;
    driver.expect_no_event("any declare-blockers step", |e| {
        matches!(
            e,
            Event::StepBegan {
                step: Step::DeclareBlockers | Step::CombatDamage | Step::FirstStrikeDamage
            }
        )
    })
}

#[rstest]
#[case(2, 0)]
#[case(3, 1)]
#[case(4, 1)]
#[case(8, 1)]
fn only_two_player_games_skip_the_first_players_first_draw(
    #[case] players: usize,
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(players).at(1, 0, Step::Untap))?;
    driver.idle_until(1, Step::Main1)?;
    let drawn = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::CardDrawn { player, .. } if *player == pid(0)))
        .count();
    assert_eq!(drawn, draws);
    Ok(())
}

#[test]
fn later_turns_always_draw() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).at(2, 1, Step::Untap))?;
    driver.idle_until(2, Step::Main1)?;
    let before = driver.state().zone(Zone::Hand(pid(1))).len();
    assert_eq!(before, 1);
    Ok(())
}

#[rstest]
#[case(0, 0)]
#[case(7, 0)]
#[case(8, 1)]
#[case(9, 2)]
#[case(12, 5)]
fn cleanup_discards_down_to_seven(
    #[case] hand: usize,
    #[case] discards: usize,
) -> Result<(), ScriptError> {
    let cards = (0..hand).map(|_| forest());
    let mut driver =
        Driver::from_scenario(Scenario::new(2).at(1, 0, Step::Cleanup).hand(0, cards))?;
    if discards == 0 {
        driver.expect_step(Step::Upkeep)?;
    } else {
        driver.expect_decision(0, "discard")?;
        let first_cards: Vec<_> = driver
            .state()
            .zone(Zone::Hand(pid(0)))
            .iter()
            .copied()
            .take(discards)
            .collect();
        driver.answer(mtg_engine::Answer::Discard(first_cards))?;
    }
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand.min(7));
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), discards);
    assert_eq!(driver.state().turn().number(), 2, "the next turn began");
    Ok(())
}

#[rstest]
#[case(Step::Upkeep, Step::Draw)]
#[case(Step::Draw, Step::Main1)]
#[case(Step::Main1, Step::BeginCombat)]
#[case(Step::BeginCombat, Step::EndCombat)]
#[case(Step::EndCombat, Step::Main2)]
#[case(Step::Main2, Step::End)]
#[case(Step::End, Step::Cleanup)]
fn unspent_mana_vanishes_when_the_step_ends(
    #[case] start: Step,
    #[case] next: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, start)
            .battlefield(0, [ready(forest())]),
    )?;
    driver.tap_for_mana("Forest")?;
    assert_eq!(
        driver.state().player(pid(0)).map(|p| p.mana().total()),
        Some(1)
    );
    driver.pass_until(next)?;
    assert_eq!(
        driver.state().player(pid(0)).map(|p| p.mana().total()),
        Some(0)
    );
    driver.expect_event(
        "ManaPoolEmptied",
        |e| matches!(e, Event::ManaPoolEmptied { player } if *player == pid(0)),
    )
}

#[test]
fn untap_step_untaps_only_the_active_players_permanents() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(3)
            .at(3, 1, Step::Untap)
            .battlefield(0, [tapped(forest()), tapped(bear())])
            .battlefield(1, [tapped(forest()), tapped(bear()), ready(bear())])
            .battlefield(2, [tapped(forest())]),
    )?;
    let state = driver.state();
    let tapped_by_owner = |seat: usize| {
        state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id))
            .filter(|o| o.controller() == pid(seat))
            .filter(|o| {
                o.permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
            })
            .count()
    };
    assert_eq!(
        (tapped_by_owner(0), tapped_by_owner(1), tapped_by_owner(2)),
        (2, 0, 1)
    );
    assert_eq!(names(state, Zone::Battlefield).len(), 6);
    Ok(())
}
