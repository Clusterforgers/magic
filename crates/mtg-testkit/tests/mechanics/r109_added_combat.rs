//! Goal: an effect that lasts "during that combat phase" applies in the combat phase its resolution added and in no other
//! (CR 500.8), at two to four seats: after Bumi's trigger only land creatures can attack in the added combat, and after
//! Illusionist's Gambit the attackers removed from combat attack again if able but not its caster; Portal Manipulator
//! entering in the declare attackers step sends the attackers of the target player's opponents at that player; each log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// The creatures offered as attackers, each with what it may attack.
type Offered = Vec<(ObjectId, Vec<AttackTarget>)>;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// A 1/1 land creature.
fn arbor() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Arbor")
            .with_type(CardType::Land)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// Answers passively up to the next declaration of attackers this turn; `None` when
/// the turn moves on first.
fn to_attackers(driver: &mut Driver) -> Result<Option<Offered>, ScriptError> {
    let turn = driver.state().turn().number();
    for _ in 0..200 {
        if driver.state().turn().number() != turn {
            return Ok(None);
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::DeclareAttackers(options) = &decision.kind {
            return Ok(Some(
                options
                    .attackers
                    .iter()
                    .map(|a| (a.object, a.defenders.clone()))
                    .collect(),
            ));
        }
        driver.answer_passively()?;
    }
    Ok(None)
}

#[rstest]
fn r109_only_land_creatures_attack_in_bumis_added_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(card("Bumi, Unleashed").unwrap()),
                ready(free_creature("Bear", 2, 2)),
                ready(arbor()),
                tapped(island()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Bumi, Unleashed", 1), ("Bear", 1)])?;
    let offered = to_attackers(&mut driver)?.expect("an added combat");
    let names: Vec<String> = offered
        .iter()
        .map(|(id, _)| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert_eq!(names, ["Arbor"], "only the land creature may attack");
    assert!(
        !driver
            .state()
            .object(driver.id_of("Island")?)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    let bear = driver.id_of("Bear")?;
    let refused = driver.answer(Answer::DeclareAttackers(vec![(
        bear,
        AttackTarget::Player(seat(1)),
    )]));
    assert!(
        refused.is_err(),
        "a creature that is not a land can't attack in that combat"
    );
    driver.attack(&[("Arbor", 1)])?;
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
fn r109_illusionists_gambit_sends_the_attackers_elsewhere(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        )
        .battlefield(1, [ready(free_creature("Raider", 3, 3))])
        .hand(0, [card("Illusionist's Gambit").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Raider", 0)])?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if decision.player == seat(0)
            && driver.state().turn().step() == Step::DeclareBlockers
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.answer_passively()?;
    }
    driver.cast("Illusionist's Gambit")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let raider = driver.id_of("Raider")?;
    assert!(
        !driver
            .state()
            .object(raider)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped(),
        "untapped"
    );
    let Some(offered) = to_attackers(&mut driver)? else {
        // With only the caster to attack, nothing can attack in the added combat.
        assert_eq!(seats, 2, "an added combat with a player to attack");
        replays(&scenario, &driver);
        return Ok(());
    };
    let defenders = offered
        .iter()
        .find(|(id, _)| *id == raider)
        .map(|(_, d)| d.clone())
        .unwrap_or_default();
    assert!(
        !defenders.contains(&AttackTarget::Player(seat(0))),
        "it can't attack the caster"
    );
    // With another player to attack it must attack; with none left it can't attack at all.
    let skipped = driver.answer(Answer::DeclareAttackers(Vec::new()));
    assert_eq!(skipped.is_err(), seats > 2, "{defenders:?}");
    if seats > 2 {
        let at_caster = driver.answer(Answer::DeclareAttackers(vec![(
            raider,
            AttackTarget::Player(seat(0)),
        )]));
        assert!(at_caster.is_err(), "it can't attack the caster that combat");
        driver.attack(&[("Raider", 2)])?;
    }
    replays(&scenario, &driver);
    Ok(())
}

/// Passes until seat 0 has priority in the current step.
fn to_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if decision.player == seat(0) && matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("seat 0 priority".to_owned()))
}

#[rstest]
fn r109_portal_manipulator_sends_attackers_at_the_target_player(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Raider", 3, 3)),
                ready(free_creature("Scout", 1, 1)),
            ],
        )
        .hand(0, [card("Portal Manipulator").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Raider", 0), ("Scout", 0)])?;
    to_seat_zero(&mut driver)?;
    assert_eq!(driver.state().turn().step(), Step::DeclareAttackers);
    driver.cast("Portal Manipulator")?;
    let raider = driver.target_named("Raider")?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseTargets(options) = &decision.kind {
            if options.spec == 0 {
                // The player whose opponents' attackers turn on them: seat 1's own attackers are not offered then.
                let at = if seats > 2 { 2 } else { 1 };
                driver.choose_targets(&[driver.target_player(at)])?;
                continue;
            }
            if seats == 2 {
                assert!(
                    !options.legal.contains(&raider),
                    "seat 1's attacker is no opponent's of seat 1"
                );
                driver.choose_targets(&[])?;
            } else {
                driver.choose_targets(std::slice::from_ref(&raider))?;
            }
            continue;
        }
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let target_of = |driver: &Driver, name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .find(|a| a.object() == id)
            .map(mtg_engine::combat::Attacker::target)
    };
    let expected = if seats > 2 { seat(2) } else { seat(0) };
    assert_eq!(
        target_of(&driver, "Raider"),
        Some(AttackTarget::Player(expected))
    );
    assert_eq!(
        target_of(&driver, "Scout"),
        Some(AttackTarget::Player(seat(0))),
        "an untargeted attacker stays"
    );
    replays(&scenario, &driver);
    Ok(())
}
