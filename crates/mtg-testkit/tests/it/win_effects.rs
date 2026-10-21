//! Goal: an effect that says a player wins the game makes every opponent lose, at tables of two, three and four seats: the game ends with the caster the only player left once the spell has finished resolving, in the caster's turn or another's, a seat that had left already is not made to lose again, the win can be answered by countering the spell, and Felidar Sovereign wins its controller's game at the beginning of their upkeep with 40 life or more and not with less.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameResult};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

fn victory(effects: &str) -> Arc<CardDefinition> {
    spell("Victory", CardType::Instant, "[]", effects)
}

fn hush() -> Arc<CardDefinition> {
    spell(
        "Hush",
        CardType::Instant,
        r#"[{"what":{"Object":["Spell"]}}]"#,
        r#"[{"Counter":{"what":{"Target":0}}}]"#,
    )
}

fn until_has_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// The seats that lost, in the order the log says, with why.
fn losses(driver: &Driver) -> Vec<(usize, LossReason)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::PlayerLost { player, reason } => Some((player.index(), reason.clone())),
            _ => None,
        })
        .collect()
}

fn position(driver: &Driver, wanted: fn(&Event) -> bool) -> Option<usize> {
    driver.engine().events().iter().position(wanted)
}

fn won() -> LossReason {
    LossReason::Format("an opponent won the game".into())
}

#[rstest]
fn the_caster_is_the_only_player_left_when_the_spell_is_done(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] own_turn: bool,
) -> Result<(), ScriptError> {
    let active = usize::from(!own_turn);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, active, Step::Main1)
            .hand(0, [victory(r#"[{"WinGame":{}}]"#)]),
    )?;
    until_has_priority(&mut driver, 0)?;
    driver.cast("Victory")?;
    assert!(
        driver.state().result().is_none(),
        "casting the spell wins nothing"
    );
    resolve_all(&mut driver)?;
    assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(0))));
    assert!(driver.engine().decision().is_none(), "the game is over");
    // Every opponent lost, once, in turn order from the active player, and the caster did not.
    let order: Vec<usize> = (0..seats)
        .map(|n| (active + n) % seats)
        .filter(|s| *s != 0)
        .collect();
    assert_eq!(
        losses(&driver),
        order.iter().map(|seat| (*seat, won())).collect::<Vec<_>>()
    );
    for seat in 0..seats {
        assert_eq!(driver.state().is_alive(pid(seat)), seat == 0, "seat {seat}");
    }
    // The losses come after the spell did what it says and before the game ends, which it does once.
    let ended = |e: &Event| matches!(e, Event::GameEnded { .. });
    let count = driver.engine().events().iter().filter(|e| ended(e)).count();
    assert_eq!(count, 1);
    let first_loss = position(&driver, |e| matches!(e, Event::PlayerLost { .. })).unwrap();
    assert!(first_loss < position(&driver, ended).unwrap());
    driver.expect_zone(Zone::Graveyard(pid(0)), &["Victory"])
}

#[rstest]
fn seats_that_left_already_are_not_made_to_lose_again(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, [victory(r#"[{"WinGame":{}}]"#)]))?;
    driver.concede(pid(1))?;
    assert_eq!(driver.state().result(), None);
    driver.cast("Victory")?;
    resolve_all(&mut driver)?;
    assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(0))));
    let lost = losses(&driver);
    assert_eq!(lost[0], (1, LossReason::Conceded));
    assert_eq!(
        lost[1..],
        (2..seats).map(|seat| (seat, won())).collect::<Vec<_>>()
    );
    Ok(())
}

#[rstest]
fn the_rest_of_the_spell_is_done_before_the_game_ends(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(
        0,
        [victory(r#"[{"WinGame":{}},{"GainLife":{"amount":3}}]"#)],
    ))?;
    driver.cast("Victory")?;
    resolve_all(&mut driver)?;
    driver.expect_life(0, 23)?;
    let finished = position(&driver, |e| {
        matches!(
            e,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            }
        )
    })
    .unwrap();
    let ended = position(&driver, |e| matches!(e, Event::GameEnded { .. })).unwrap();
    assert!(finished < ended);
    assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(0))));
    Ok(())
}

#[rstest]
fn the_win_can_be_answered_by_countering_the_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [victory(r#"[{"WinGame":{}}]"#)])
            .hand(1, [hush()]),
    )?;
    driver.cast("Victory")?;
    until_has_priority(&mut driver, 1)?;
    let spell = spell_named(&driver, "Victory");
    driver.cast_at("Hush", &[spell])?;
    resolve_all(&mut driver)?;
    assert_eq!(driver.state().result(), None);
    assert_eq!(losses(&driver), []);
    for seat in 0..seats {
        assert!(driver.state().is_alive(pid(seat)));
    }
    driver.expect_zone(Zone::Graveyard(pid(0)), &["Victory"])?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Hush"])
}

#[rstest]
#[case::enough(40, true)]
#[case::more_than_enough(41, true)]
#[case::one_short(39, false)]
#[case::the_starting_life(20, false)]
fn felidar_sovereign_wins_at_the_upkeep_with_forty_life_or_more(
    #[case] life: i32,
    #[case] wins: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Untap)
            .battlefield_named(0, ["Felidar Sovereign"])
            .life(0, life),
    )?;
    for _ in 0..40 {
        if driver.state().result().is_some() || driver.state().turn().step() == Step::Draw {
            break;
        }
        driver.answer_passively()?;
    }
    if wins {
        assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(0))));
        assert_eq!(losses(&driver).len(), seats - 1);
        assert_eq!(driver.state().turn().step(), Step::Upkeep);
    } else {
        assert_eq!(driver.state().result(), None);
        assert_eq!(losses(&driver), []);
        assert_eq!(driver.state().turn().step(), Step::Draw);
    }
    Ok(())
}
