//! Goal: in a party's combined attack (CR 805, CR 508.1h) each hero pays the attack tax of their own creatures
//! (Windborn Muse, Ghostly Prison, Propaganda, Archangel of Tithes) from their own mana, so a hero's tax never stalls the
//! party: the teammates are asked to pay in turn and the leader's payment makes the declaration, each hero's lands pay
//! for their own creatures only, a hero who cannot pay for their own choice is refused it while the others still attack,
//! and a teammate who backs out of their payment returns the party to the declaration with nothing paid.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// `lands[seat]` Plains and a ready Bear for each hero, then a boss holding `tax`, under shared team turns, at the
/// party's declare-attackers step.
fn party(tax: &str, lands: &[usize]) -> Result<Driver, ScriptError> {
    let heroes = lands.len();
    let mut seats = vec![0; heroes];
    seats.push(1);
    let mut scenario = Scenario::new(heroes + 1)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats,
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1)
        .life(heroes, 100);
    for (seat, count) in lands.iter().enumerate() {
        let mut board = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        board.extend((0..*count).map(|_| ready(plains())));
        scenario = scenario.battlefield(seat, board);
    }
    scenario = scenario.battlefield(heroes, vec![ready(card(tax).unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    Ok(driver)
}

/// The tapped Plains of each hero.
fn tapped_lands(driver: &Driver, heroes: usize) -> Vec<usize> {
    let state = driver.state();
    (0..heroes)
        .map(|seat| {
            state
                .zone(Zone::Battlefield)
                .iter()
                .filter_map(|id| state.object(*id))
                .filter(|object| {
                    object.controller() == pid(seat)
                        && object.card().name == "Plains"
                        && object
                            .permanent()
                            .is_some_and(mtg_engine::object::PermanentState::tapped)
                })
                .count()
        })
        .collect()
}

/// `seat` chooses to attack the boss with their Bear, or with nothing.
fn pick(
    driver: &mut Driver,
    seat: usize,
    boss: usize,
    attack: bool,
) -> Result<Vec<Event>, ScriptError> {
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(seat), "hero {seat} chooses");
    assert!(matches!(decision.kind, DecisionKind::DeclareAttackers(_)));
    let chosen: Vec<(ObjectId, AttackTarget)> = if attack {
        vec![(
            driver.id_of(&format!("Bear {seat}"))?,
            AttackTarget::Player(pid(boss)),
        )]
    } else {
        Vec::new()
    };
    driver.answer(Answer::DeclareAttackers(chosen))
}

/// Answers each payment the party is asked for, recording who paid, until none is left.
fn pay_all(driver: &mut Driver) -> Result<Vec<usize>, ScriptError> {
    let mut payers = Vec::new();
    while let Ok(decision) = driver.decision()
        && matches!(decision.kind, DecisionKind::PayMana(_))
    {
        payers.push(decision.player.index());
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    Ok(payers)
}

/// Each hero's tax per creature for `tax`.
fn per_creature(tax: &str) -> usize {
    if tax == "Archangel of Tithes" { 1 } else { 2 }
}

#[rstest]
fn each_hero_pays_for_their_own_creatures(
    #[values("Windborn Muse", "Ghostly Prison", "Propaganda", "Archangel of Tithes")] tax: &str,
    #[values(2, 3)] heroes: usize,
) -> Result<(), ScriptError> {
    let each = per_creature(tax);
    let lands = vec![each; heroes];
    let mut driver = party(tax, &lands)?;
    for seat in 0..heroes {
        let options = match &driver.decision().unwrap().kind {
            DecisionKind::DeclareAttackers(options) => options.clone(),
            _ => panic!("not attackers"),
        };
        assert_eq!(
            options.mana as usize, each,
            "hero {seat}'s budget is their own mana"
        );
        pick(&mut driver, seat, heroes, true)?;
    }
    let payers = pay_all(&mut driver)?;
    let mut expected: Vec<usize> = (1..heroes).collect();
    expected.push(0);
    assert_eq!(payers, expected, "the teammates pay, then the leader");
    assert_eq!(
        driver.state().combat().attackers().len(),
        heroes,
        "every Bear attacks"
    );
    assert_eq!(
        tapped_lands(&driver, heroes),
        lands,
        "each hero tapped their own lands"
    );
    Ok(())
}

/// `lands` per hero; `attacks` whether each hero attacks once a hero who cannot pay chose nothing instead.
#[rstest]
#[case::the_teammate_cannot_pay(&[2, 0], &[true, false])]
#[case::the_leader_cannot_pay(&[0, 2], &[false, true])]
#[case::neither_can_pay(&[1, 1], &[false, false])]
fn a_hero_who_cannot_pay_is_refused_their_own_choice(
    #[case] lands: &[usize],
    #[case] attacks: &[bool],
) -> Result<(), ScriptError> {
    let heroes = lands.len();
    let mut driver = party("Windborn Muse", lands)?;
    for (seat, attack) in attacks.iter().enumerate() {
        if !attack {
            let refused = pick(&mut driver, seat, heroes, true);
            assert!(
                matches!(
                    refused,
                    Err(ScriptError::Engine(EngineError::Illegal(
                        IllegalAnswer::AttackUnaffordable(_)
                    )))
                ),
                "hero {seat}'s unpaid Bear is refused: {refused:?}"
            );
        }
        pick(&mut driver, seat, heroes, *attack)?;
    }
    pay_all(&mut driver)?;
    let attacking = driver.state().combat().attackers().len();
    assert_eq!(attacking, attacks.iter().filter(|a| **a).count());
    let paid: Vec<usize> = attacks.iter().map(|a| if *a { 2 } else { 0 }).collect();
    assert_eq!(tapped_lands(&driver, heroes), paid);
    Ok(())
}

#[test]
fn a_teammate_who_backs_out_returns_the_party_to_the_declaration() -> Result<(), ScriptError> {
    let mut driver = party("Windborn Muse", &[2, 2])?;
    pick(&mut driver, 0, 2, true)?;
    pick(&mut driver, 1, 2, true)?;
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(1), "the teammate pays first");
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(tapped_lands(&driver, 2), [0, 0], "nothing is paid");
    assert_eq!(
        driver.state().combat().attackers().len(),
        0,
        "nothing attacks"
    );
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::DeclareAttackers(_)
    ));
    Ok(())
}
