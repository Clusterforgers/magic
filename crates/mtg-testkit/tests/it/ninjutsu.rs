//! Goal: ninjutsu is an activated ability from the hand that returns an unblocked attacker the player controls to its owner's hand and puts the card onto the battlefield tapped and attacking the same player, without declaring it as an attacker: it is offered only after blockers are declared and only with an unblocked attacker and the mana to pay, never for a blocked attacker or one that left combat; the attacker returns as a cost, the ninja deals damage as an attacker, a copy of it is no attacker, and the ability does nothing for a card that left the hand; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{name_of, on_battlefield, pid};
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{forest, island, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority until the stack is empty or a decision other than priority is pending.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The cards called `name` in `zone`.
fn count(state: &GameState, zone: Zone, name: &str) -> usize {
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds `ninja` and has Grizzly Bears, `lands` untapped; seat 1 has a Black Knight to block with.
fn scenario(seats: usize, ninja: &str, lands: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand_named(0, [ninja, "Brainstorm"])
            .battlefield(
                0,
                lands
                    .into_iter()
                    .chain([ready(mtg_testkit::corpus::card("Grizzly Bears").unwrap())]),
            )
            .battlefield_named(1, ["Black Knight"]),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

/// Seat 0 attacks seat `defender` with Grizzly Bears and the game stops in the declare blockers step, seat `defender`
/// having blocked with `blockers` (each pair is blocker, attacker).
fn to_blockers(
    driver: &mut Driver,
    defender: usize,
    blockers: &[(&str, &str)],
) -> Result<(), ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", defender)])?;
    loop {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            if driver.decision()?.player == pid(defender) {
                if blockers.is_empty() {
                    driver.answer(Answer::DeclareBlockers(Vec::new()))?;
                } else {
                    driver.block(blockers)?;
                }
            } else {
                driver.answer(Answer::DeclareBlockers(Vec::new()))?;
            }
        } else if driver.state().turn().step() == Step::DeclareBlockers
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        } else {
            driver.answer_passively()?;
        }
    }
}

/// Seat 0 activates the ninjutsu of `name`, paying automatically.
fn ninjutsu(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
            unreachable!()
        };
        let pick = options.from.clone();
        driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

#[rstest]
#[case::black("Ninja of the New Moon", vec![swamp(), swamp(), swamp(), swamp()], 6)]
#[case::black_lifelink("Mukotai Ambusher", vec![swamp(), swamp()], 3)]
#[case::blue_trigger("Ninja of the Deep Hours", vec![island(), island()], 2)]
#[case::black_vanilla("Dokuchi Shadow-Walker", vec![swamp(), swamp(), swamp(), swamp()], 5)]
fn the_ninja_replaces_the_unblocked_attacker_tapped_and_attacking_the_same_player(
    #[case] ninja: &str,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] power: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let defender = seats - 1;
    let mut driver = Driver::from_scenario(scenario(
        seats,
        ninja,
        lands.into_iter().map(ready).collect(),
    ))?;
    to_blockers(&mut driver, defender, &[])?;
    ninjutsu(&mut driver, ninja)?;
    let id = on_battlefield(driver.state(), ninja).expect("the ninja entered");
    assert!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .map(|a| (a.object(), a.target()))
            .collect::<Vec<_>>(),
        [(id, AttackTarget::Player(pid(defender)))],
        "the ninja attacks the player the attacker attacked, and the attacker is gone"
    );
    assert_eq!(
        count(driver.state(), Zone::Hand(pid(0)), "Grizzly Bears"),
        1
    );
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), ninja), 0);
    assert!(
        !driver.engine().events().iter().any(|event| matches!(event, Event::AttackersDeclared { attackers, .. } if attackers.iter().any(|(o, _)| *o == id))),
        "the ninja was never declared as an attacker"
    );
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    let life = driver.state().player(pid(defender)).unwrap().life();
    assert_eq!(life, 20 - power, "the ninja dealt its combat damage");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::before_attackers(Step::Main1, false)]
#[case::declare_blockers(Step::DeclareBlockers, true)]
#[case::combat_damage(Step::CombatDamage, true)]
#[case::end_of_combat(Step::EndCombat, true)]
fn it_is_offered_in_combat_after_blockers_are_declared_with_an_unblocked_attacker(
    #[case] step: Step,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(
        2,
        "Dokuchi Shadow-Walker",
        vec![
            ready(swamp()),
            ready(swamp()),
            ready(swamp()),
            ready(swamp()),
        ],
    ))?;
    if step == Step::Main1 {
        driver.idle_until(1, Step::Main1)?;
    } else {
        to_blockers(&mut driver, 1, &[])?;
        if step != Step::DeclareBlockers {
            driver.pass_until(step)?;
        }
    }
    assert_eq!(
        !driver
            .offered_abilities("Dokuchi Shadow-Walker")?
            .is_empty(),
        offered
    );
    Ok(())
}

#[rstest]
#[case::blocked("Dokuchi Shadow-Walker", true, 4, false)]
#[case::unblocked_without_the_mana("Dokuchi Shadow-Walker", false, 3, false)]
#[case::unblocked_with_the_mana("Dokuchi Shadow-Walker", false, 4, true)]
fn a_blocked_attacker_or_a_cost_that_cannot_be_paid_offers_nothing(
    #[case] ninja: &str,
    #[case] block: bool,
    #[case] swamps: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let lands = (0..swamps).map(|_| ready(swamp())).collect();
    let mut driver = Driver::from_scenario(scenario(2, ninja, lands))?;
    let blockers: &[(&str, &str)] = if block {
        &[("Black Knight", "Grizzly Bears")]
    } else {
        &[]
    };
    to_blockers(&mut driver, 1, blockers)?;
    assert_eq!(!driver.offered_abilities(ninja)?.is_empty(), offered);
    Ok(())
}

#[test]
fn an_attacker_that_left_combat_cannot_be_returned() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        scenario(
            2,
            "Dokuchi Shadow-Walker",
            (0..4).map(|_| ready(swamp())).collect(),
        )
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, [ready(mountain())]),
    )?;
    to_blockers(&mut driver, 1, &[])?;
    driver.pass()?;
    let id = on_battlefield(driver.state(), "Grizzly Bears").unwrap();
    let at = Target::Object(driver.state().object(id).unwrap().reference());
    driver.cast_at("Lightning Bolt", &[at])?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Grizzly Bears").is_none());
    assert_eq!(driver.state().combat().attackers().len(), 0);
    assert_eq!(driver.offered_abilities("Dokuchi Shadow-Walker")?.len(), 0);
    Ok(())
}

#[test]
fn a_copy_of_the_ninja_is_no_attacker_and_stays_out_of_combat() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        scenario(
            2,
            "Mukotai Ambusher",
            vec![
                ready(swamp()),
                ready(swamp()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        )
        .hand_named(0, ["Mukotai Ambusher", "Cackling Counterpart"]),
    )?;
    to_blockers(&mut driver, 1, &[])?;
    ninjutsu(&mut driver, "Mukotai Ambusher")?;
    let at = driver.target_named("Mukotai Ambusher")?;
    driver.cast_at("Cackling Counterpart", &[at])?;
    settle(&mut driver)?;
    assert_eq!(
        count(driver.state(), Zone::Battlefield, "Mukotai Ambusher"),
        2
    );
    assert_eq!(
        driver.state().combat().attackers().len(),
        1,
        "only the ninja attacks"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_ninja_that_left_the_hand_in_response_does_nothing_and_the_attacker_stays_returned(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ninja = "Ninja of the Deep Hours";
    let lands = vec![ready(island()), ready(island()), ready(island())];
    let mut driver = Driver::from_scenario(scenario(seats, ninja, lands))?;
    to_blockers(&mut driver, seats - 1, &[])?;
    driver.begin_activation(ninja)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(driver.state().stack().len(), 1, "the ability waits");
    let ninja_id = *driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .find(|id| name_of(driver.state(), **id) == ninja)
        .unwrap();
    // In response Brainstorm draws three cards and puts the ninja and a forest back on the library.
    driver.cast("Brainstorm")?;
    for _ in 0..seats {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            break;
        }
        driver.pass()?;
    }
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the cards to put back are asked");
    };
    let forest_id = *options.from.iter().find(|id| **id != ninja_id).unwrap();
    driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
        ninja_id, forest_id,
    ])))?;
    // The two cards go back in an order, then the ability resolves.
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        driver.answer_passively()?;
    }
    assert_eq!(count(driver.state(), Zone::Battlefield, ninja), 0);
    assert_eq!(count(driver.state(), Zone::Library(pid(0)), ninja), 1);
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), ninja), 0);
    assert_eq!(
        count(driver.state(), Zone::Hand(pid(0)), "Grizzly Bears"),
        1,
        "the attacker returned as a cost"
    );
    assert_eq!(driver.state().combat().attackers().len(), 0);
    assert_eq!(driver.state().stack(), []);
    assert_replays(&driver);
    Ok(())
}
