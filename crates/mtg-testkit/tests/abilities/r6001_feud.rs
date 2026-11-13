//! Goal: Bitter Feud doubles damage only when a source controlled by one of its two chosen players would deal damage to
//! the other chosen player or a permanent that player controls, at two, three and four seats; damage to the source's own
//! controller, to a player not chosen or from a player not chosen is dealt as it is, and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const FEUD: &str = "Bitter Feud";

/// Passes until the stack is empty, choosing the seats in `players` in turn when a player is asked for.
fn r6001_settle(driver: &mut Driver, players: &[usize]) -> Result<(), ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let want = ChoiceOption::Player(pid(players[asked]));
                let at = options
                    .options
                    .iter()
                    .position(|o| *o == want)
                    .expect("the seat is offered");
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            _ => driver.answer_passively()?,
        }
    }
    Err(ScriptError::Stuck("the stack does not empty".to_owned()))
}

/// Where Lightning Bolt goes: a player's seat, or the 0/4 wall that seat controls.
#[derive(Clone, Copy, Debug)]
enum Aim {
    Player(usize),
    Wall(usize),
}

#[rstest]
#[case::the_other_chosen_player(2, [0, 1], Aim::Player(1), true)]
#[case::the_casters_own_life(2, [0, 1], Aim::Player(0), false)]
#[case::a_permanent_of_the_other(2, [0, 1], Aim::Wall(1), true)]
#[case::own_permanent(3, [0, 2], Aim::Wall(0), false)]
#[case::a_player_not_chosen(3, [0, 2], Aim::Player(1), false)]
#[case::caster_not_chosen(3, [1, 2], Aim::Player(2), false)]
#[case::four_seats_chosen_pair(4, [3, 0], Aim::Player(3), true)]
#[case::four_seats_wall_of_other(4, [0, 3], Aim::Wall(3), true)]
fn r6001_bitter_feud_doubles_damage_between_the_chosen_players(
    #[case] seats: usize,
    #[case] chosen: [usize; 2],
    #[case] aim: Aim,
    #[case] doubled: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut scenario = Scenario::new(seats)
            .at(1, 0, mtg_engine::turn::Step::Main1)
            .hand(0, [card(FEUD).unwrap(), card("Lightning Bolt").unwrap()]);
        for seat in 0..seats {
            let mut board = vec![ready(free_creature(&format!("Wall {seat}"), 0, 4))];
            if seat == 0 {
                board.extend((0..4).map(|_| ready(swamp())));
                board.extend((0..2).map(|_| ready(mountain())));
            }
            scenario = scenario.battlefield(seat, board);
        }
        scenario
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast(FEUD)?;
    r6001_settle(&mut driver, &chosen)?;
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.begin_cast("Lightning Bolt")?;
    let at = match aim {
        Aim::Player(seat) => driver.target_player(seat),
        Aim::Wall(seat) => driver.target_named(&format!("Wall {seat}"))?,
    };
    driver.choose_targets(&[at])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r6001_settle(&mut driver, &[])?;
    let dealt = if doubled { 6 } else { 3 };
    match aim {
        Aim::Player(seat) => {
            let mut expected = before;
            expected[seat] -= dealt;
            let after: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
            assert_eq!(after, expected);
        }
        Aim::Wall(seat) => {
            // 3 damage leaves the 0/4 wall alive; 6 destroys it.
            let wall = driver.id_of(&format!("Wall {seat}"))?;
            let zone = driver.state().object(wall).map(|o| o.zone());
            assert_eq!(zone != Some(Zone::Battlefield), doubled, "{zone:?}");
        }
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
