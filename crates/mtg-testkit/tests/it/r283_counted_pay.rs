//! Goal: "counter target spell unless its controller discards their hand" (Perplex) and "... exiles all cards from
//! their graveyard" (Grip of Amnesia) ask the controller of the target, who pays with the whole of their own hand or
//! graveyard counted as the spell resolves, or lets the spell be countered, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pass_until_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

/// How many cards lie in the zone.
fn count(driver: &Driver, zone: Zone) -> usize {
    driver.state().zone(zone).len()
}

#[rstest]
#[case::perplex_paid("Perplex", true)]
#[case::perplex_declined("Perplex", false)]
#[case::grip_paid("Grip of Amnesia", true)]
#[case::grip_declined("Grip of Amnesia", false)]
fn r283_the_controller_of_the_target_pays_with_all_of_their_own_cards_or_loses_the_spell(
    #[case] counter: &str,
    #[case] pays: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let caster = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(
                0,
                [
                    spell(
                        "Victim",
                        CardType::Instant,
                        "[]",
                        r#"[{"GainLife":{"amount":5}}]"#,
                    ),
                    free_creature("Bear", 2, 2),
                    free_creature("Bear", 2, 2),
                ],
            )
            .graveyard(0, vec![free_creature("Bear", 2, 2); 3])
            .library(0, vec![island(); 4])
            .library(caster, vec![island(); 4])
            .hand_named(caster, [counter])
            .graveyard(caster, [free_creature("Bear", 2, 2)])
            .battlefield(caster, [ready(island()), ready(island()), ready(swamp())]),
    )?;
    driver.cast("Victim")?;
    pass_until_priority(&mut driver, caster)?;
    let target = spell_named(&driver, "Victim");
    driver.cast_at(counter, &[target])?;
    let mut asked = false;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        let decision = driver.decision()?;
        if matches!(decision.kind, DecisionKind::YesNo(_)) {
            assert_eq!(
                decision.player,
                pid(0),
                "the controller of the target is asked"
            );
            asked = true;
            driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert!(asked);
    let victim_resolved = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .any(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Victim")
        });
    assert!(
        victim_resolved,
        "Victim ends in its owner's graveyard either way"
    );
    let life = driver
        .state()
        .player(pid(0))
        .map(mtg_engine::player::Player::life);
    assert_eq!(
        life,
        Some(if pays { 25 } else { 20 }),
        "Victim resolves only when paid"
    );
    let (hand, graveyard, exile) = (
        count(&driver, Zone::Hand(pid(0))),
        count(&driver, Zone::Graveyard(pid(0))),
        count(&driver, Zone::Exile),
    );
    match (counter, pays) {
        // The two Bears of the hand are discarded beside Victim.
        ("Perplex", true) => assert_eq!((hand, graveyard, exile), (0, 6, 0)),
        // The three Bears of the graveyard are exiled; the caster's own graveyard stays.
        ("Grip of Amnesia", true) => assert_eq!((hand, graveyard, exile), (2, 1, 3)),
        _ => assert_eq!((hand, graveyard, exile), (2, 4, 0)),
    }
    // The caster's own cards are never paid.
    assert!(count(&driver, Zone::Graveyard(pid(caster))) >= 1);
    Ok(())
}
