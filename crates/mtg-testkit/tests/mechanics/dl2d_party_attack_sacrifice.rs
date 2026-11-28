//! Goal: in a combined attack of a team sharing its turns (Delves and Two-Headed Giant, CR 805, CR 810, CR 508.1g-h)
//! each hero gives up the permanents an attack tax takes for their own creatures (Reclamation, Flooded Woodlands, beside
//! Windborn Muse's mana): each hero is asked for their own lands, teammates in turn order before the leader, only their
//! own lands are offered and sacrificed, a hero without a land is refused their own choice while the others still
//! attack, a teammate who backs out returns the party to the declaration with nothing given up, and every log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::CardsChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, replay};
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The color each sacrifice tax names.
fn color_of(tax: &str) -> Color {
    if tax == "Flooded Woodlands" {
        Color::Green
    } else {
        Color::Black
    }
}

/// `lands[seat]` Plains and a ready creature of `color` for each hero, then a foe holding `taxes`, at the party's
/// declare-attackers step; Two-Headed Giant seats two foes against two heroes, else the party shares its turns.
fn party(
    mode: TeamMode,
    taxes: &[&str],
    color: Color,
    lands: &[usize],
) -> Result<Driver, ScriptError> {
    let heroes = lands.len();
    let foes = if mode == TeamMode::TwoHeadedGiant {
        2
    } else {
        1
    };
    let mut seats = vec![0; heroes];
    seats.extend(vec![1; foes]);
    let mut scenario = Scenario::new(heroes + foes)
        .house_rules(HouseRules {
            teams: Some(Teams { mode, seats }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1);
    for (seat, count) in lands.iter().enumerate() {
        let mut board = vec![ready(creature(&format!("Bear {seat}"), 0, &[color], 2, 2))];
        board.extend((0..*count).map(|_| ready(plains())));
        scenario = scenario.battlefield(seat, board);
    }
    let held: Vec<_> = taxes.iter().map(|tax| ready(card(tax).unwrap())).collect();
    scenario = scenario.battlefield(heroes, held);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    Ok(driver)
}

/// The Plains each hero still controls.
fn lands_left(driver: &Driver, heroes: usize) -> Vec<usize> {
    let state = driver.state();
    (0..heroes)
        .map(|seat| {
            state
                .zone(Zone::Battlefield)
                .iter()
                .filter_map(|id| state.object(*id))
                .filter(|object| object.controller() == pid(seat) && object.card().name == "Plains")
                .count()
        })
        .collect()
}

/// `seat` chooses to attack the first foe with their Bear, or with nothing.
fn pick(driver: &mut Driver, seat: usize, foe: usize, attack: bool) -> Result<(), ScriptError> {
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(seat), "hero {seat} chooses");
    assert!(matches!(decision.kind, DecisionKind::DeclareAttackers(_)));
    let chosen: Vec<(ObjectId, AttackTarget)> = if attack {
        vec![(
            driver.id_of(&format!("Bear {seat}"))?,
            AttackTarget::Player(pid(foe)),
        )]
    } else {
        Vec::new()
    };
    driver.answer(Answer::DeclareAttackers(chosen))?;
    Ok(())
}

/// Answers each pick and payment the party is asked for with the first lands offered and the automatic payment,
/// recording who was asked for lands, until none is left.
fn give_up_all(driver: &mut Driver) -> Result<Vec<usize>, ScriptError> {
    let mut pickers = Vec::new();
    while let Ok(decision) = driver.decision() {
        let player = decision.player;
        match decision.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                assert!(
                    options
                        .from
                        .iter()
                        .all(|id| state.object(*id).unwrap().controller() == player),
                    "hero {} is offered only their own lands",
                    player.index()
                );
                pickers.push(player.index());
                let taken = options.from.iter().copied().take(options.min as usize);
                driver.answer(Answer::ChooseCards(CardsChoice(taken.collect())))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    Ok(pickers)
}

/// The log replays to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn each_hero_sacrifices_their_own_land_for_their_own_creature(
    #[values(
        &["Reclamation"][..],
        &["Flooded Woodlands"][..],
        &["Reclamation", "Windborn Muse"][..]
    )]
    taxes: &[&str],
    #[values(2, 3, 4)] heroes: usize,
) -> Result<(), ScriptError> {
    // With the Muse each hero sacrifices one land and taps the other two for its {2}.
    let each = if taxes.contains(&"Windborn Muse") {
        3
    } else {
        2
    };
    let lands = vec![each; heroes];
    let mut driver = party(TeamMode::SharedTurns, taxes, color_of(taxes[0]), &lands)?;
    for seat in 0..heroes {
        pick(&mut driver, seat, heroes, true)?;
    }
    let pickers = give_up_all(&mut driver)?;
    let mut expected: Vec<usize> = (1..heroes).collect();
    expected.push(0);
    assert_eq!(pickers, expected, "the teammates pick, then the leader");
    assert_eq!(driver.state().combat().attackers().len(), heroes);
    assert_eq!(
        lands_left(&driver, heroes),
        vec![each - 1; heroes],
        "each hero sacrificed one land of their own"
    );
    replays(&driver);
    Ok(())
}

/// `lands` per hero; `attacks` whether each hero attacks once a hero without a land chose nothing instead.
#[rstest]
#[case::the_teammate_has_none(&[2, 0], &[true, false])]
#[case::the_leader_has_none(&[0, 2], &[false, true])]
#[case::one_of_three_has_none(&[2, 0, 2], &[true, false, true])]
#[case::none_has_any(&[0, 0, 0, 0], &[false, false, false, false])]
fn a_hero_without_a_land_is_refused_their_own_choice(
    #[case] lands: &[usize],
    #[case] attacks: &[bool],
) -> Result<(), ScriptError> {
    let heroes = lands.len();
    let mut driver = party(TeamMode::SharedTurns, &["Reclamation"], Color::Black, lands)?;
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
                "hero {seat}'s Bear is refused: {refused:?}"
            );
        }
        pick(&mut driver, seat, heroes, *attack)?;
    }
    give_up_all(&mut driver)?;
    let attacking = attacks.iter().filter(|a| **a).count();
    assert_eq!(driver.state().combat().attackers().len(), attacking);
    let left: Vec<usize> = lands
        .iter()
        .zip(attacks)
        .map(|(count, attack)| count - usize::from(*attack))
        .collect();
    assert_eq!(lands_left(&driver, heroes), left);
    replays(&driver);
    Ok(())
}

#[test]
fn a_teammate_who_backs_out_returns_the_party_to_the_declaration() -> Result<(), ScriptError> {
    let mut driver = party(
        TeamMode::SharedTurns,
        &["Reclamation"],
        Color::Black,
        &[2, 2],
    )?;
    pick(&mut driver, 0, 2, true)?;
    pick(&mut driver, 1, 2, true)?;
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(1), "the teammate picks first");
    assert!(matches!(decision.kind, DecisionKind::ChooseCards(_)));
    driver.answer(Answer::Undo)?;
    assert_eq!(lands_left(&driver, 2), [2, 2], "nothing is given up");
    assert_eq!(driver.state().combat().attackers().len(), 0);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::DeclareAttackers(_)
    ));
    Ok(())
}

#[rstest]
fn two_headed_giant_heads_each_give_up_their_own_land(
    #[values("Reclamation", "Flooded Woodlands")] tax: &str,
    #[values(&[2, 2][..], &[1, 3][..])] lands: &[usize],
) -> Result<(), ScriptError> {
    let mut driver = party(TeamMode::TwoHeadedGiant, &[tax], color_of(tax), lands)?;
    pick(&mut driver, 0, 2, true)?;
    pick(&mut driver, 1, 2, true)?;
    let pickers = give_up_all(&mut driver)?;
    let asked: Vec<usize> = [1, 0].into_iter().filter(|seat| lands[*seat] > 1).collect();
    assert_eq!(pickers, asked, "a head with one land gives it up unasked");
    assert_eq!(driver.state().combat().attackers().len(), 2);
    let left: Vec<usize> = lands.iter().map(|count| count - 1).collect();
    assert_eq!(lands_left(&driver, 2), left);
    replays(&driver);
    Ok(())
}
