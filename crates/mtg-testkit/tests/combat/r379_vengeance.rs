//! Goal: "if that player attacked you during their last turn" reads the damaged player's latest finished turn at two to
//! four seats: O-Kagachi exiles a nonland permanent of the player it hits only when that player attacked its controller in
//! their last turn (attacking another player, or not attacking, is a near miss), and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 1 begins the game's first turn with a Raider; seat 0 controls O-Kagachi; seat 1 also controls a Relic (a
/// nonland permanent) and a Plains.
fn r379_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("O-Kagachi, Vengeful Kami").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Raider", 1, 1)),
                ready(free_creature("Relic", 0, 4)),
                ready(plains()),
            ],
        )
}

/// Answers passively until the deciding seat is to declare attackers in a turn of seat `seat`.
fn r379_to_attacks(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::DeclareAttackers(_))
            && driver.state().turn().active().index() == seat
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("no attack".to_owned()))
}

fn r379_alive(driver: &Driver, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

#[rstest]
#[case::attacked_you(Some(0), true)]
#[case::near_miss_no_attack(None, false)]
#[case::near_miss_attacked_another(Some(2), false)]
fn r379_o_kagachi_avenges_an_attack(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacked: Option<usize>,
    #[case] exiles: bool,
) -> Result<(), ScriptError> {
    // With two seats there is no other player to attack.
    if attacked == Some(2) && seats == 2 {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(r379_table(seats))?;
    r379_to_attacks(&mut driver, 1)?;
    match attacked {
        Some(at) => driver.attack(&[("Raider", at)])?,
        None => driver.attack(&[])?,
    };
    r379_to_attacks(&mut driver, 0)?;
    driver.attack(&[("O-Kagachi, Vengeful Kami", 1)])?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let relic = driver.target_named("Relic")?;
                driver.choose_targets(&[relic])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(!r379_alive(&driver, "Relic"), exiles, "the Relic");
    assert!(r379_alive(&driver, "Plains"), "a land is never a target");
    let setup = r379_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
