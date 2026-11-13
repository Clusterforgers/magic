//! Goal: "whenever a creature you control fights" (Foe-Razer Regent) triggers once for each creature of a fight that its
//! controller controls, whatever the damage came to, and its delayed counters land at the next end step at two to four
//! seats; the creature of another player that fought gets nothing. "Whenever one or more creatures you control fight or
//! become blocked" (Neyith of the Dire Hunt) draws once for the batch of a fight or of a block, and not without a block.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;

use crate::fight::{MINE, THEIRS, creature, replays, settle, table};

/// The +1/+1 counters on the permanent.
fn r5513_counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::two_seats(2, 1, 3, 6)]
#[case::three_seats(3, 2, 4, 5)]
#[case::four_seats(4, 3, 3, 4)]
#[case::no_damage_dealt(3, 1, 0, 2)]
fn r5513_a_creature_you_control_that_fights_gets_two_counters_at_the_next_end_step(
    #[case] seats: usize,
    #[case] foe: usize,
    #[case] power: i32,
    #[case] toughness: i32,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            vec![card("Prey Upon").unwrap()],
            creature(MINE, power, 6, &[]),
            creature(THEIRS, 1, toughness, &[]),
        )
        .battlefield(0, [ready(card("Foe-Razer Regent").unwrap())])
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    driver.begin_cast("Prey Upon")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let fought = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Fought { .. }))
        .count();
    assert_eq!(fought, 2, "each creature of the fight fought");
    assert_eq!(
        r5513_counters(&driver, mine),
        0,
        "nothing before the end step"
    );
    driver.pass_until(Step::End)?;
    settle(&mut driver)?;
    assert_eq!(r5513_counters(&driver, mine), 2);
    assert_eq!(r5513_counters(&driver, theirs), 0, "theirs is not yours");
    replays(&driver, &setup);
    Ok(())
}

/// The cards in seat 0's hand.
fn r5513_hand(driver: &Driver) -> usize {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(crate::common::pid(0)))
        .len()
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 1)]
fn r5513_neyith_draws_once_when_creatures_you_control_fight(
    #[case] seats: usize,
    #[case] foe: usize,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            vec![card("Prey Upon").unwrap()],
            creature(MINE, 2, 6, &[]),
            creature(THEIRS, 1, 6, &[]),
        )
        .battlefield(0, [ready(card("Neyith of the Dire Hunt").unwrap())])
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let targets = [driver.target_named(MINE)?, driver.target_named(THEIRS)?];
    let before = r5513_hand(&driver);
    driver.begin_cast("Prey Upon")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    // The spell left the hand as it was cast; one draw for the batch of the fight, though two creatures fought.
    assert_eq!(r5513_hand(&driver), before);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1, true)]
#[case::three_seats(3, 2, true)]
#[case::four_seats_unblocked(4, 3, false)]
fn r5513_neyith_draws_when_a_creature_you_control_becomes_blocked(
    #[case] seats: usize,
    #[case] foe: usize,
    #[case] blocked: bool,
) -> Result<(), ScriptError> {
    let game = || {
        table(
            seats,
            foe,
            Vec::new(),
            creature(MINE, 2, 6, &[]),
            creature(THEIRS, 1, 6, &[]),
        )
        .battlefield(0, [ready(card("Neyith of the Dire Hunt").unwrap())])
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let before = r5513_hand(&driver);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(MINE, foe)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        match decision.kind {
            mtg_engine::decision::DecisionKind::DeclareBlockers(_) => {
                let blocks: &[(&str, &str)] =
                    if blocked && decision.player == crate::common::pid(foe) {
                        &[(THEIRS, MINE)]
                    } else {
                        &[]
                    };
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(r5513_hand(&driver), before + usize::from(blocked));
    replays(&driver, &setup);
    Ok(())
}
