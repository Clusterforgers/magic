//! Goal: first strike, double strike, deathtouch, trample, lifelink and indestructible deal, divide, gain and destroy exactly as the rules say, over tables of attacker and blocker mixes with one blocker, several and none, with the lethal accounting of trample checked against damage already marked and deathtouch.

#![allow(clippy::expect_used)]

mod common;

use common::combat::{survivors, to_blockers};
use common::keywords::{Unit, attack_with_all, duel, keywords, ready_units};
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::decision::{Answer, DamageOptions, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::DeathCause;
use mtg_engine::turn::Step;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{free_creature, pinger};
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready};
use rstest::rstest;

/// The division a controller most often wants: lethal damage to each blocker in order, the rest to the player
/// attacked with trample and else on the last blocker. Written out here as the rule, apart from the engine.
fn lethal_first(options: &DamageOptions) -> Vec<(ObjectId, u32)> {
    let mut left = options.power;
    let mut shares = Vec::new();
    for (at, blocker) in options.blockers.iter().enumerate() {
        let last = at + 1 == options.blockers.len();
        let share = if last && options.excess.is_none() {
            left
        } else {
            left.min(options.lethal[at])
        };
        left -= share;
        shares.push((*blocker, share));
    }
    shares
}

/// Answers every damage decision with `divide` and every other with a pass, until the end of combat.
fn finish(
    driver: &mut Driver,
    divide: impl Fn(&DamageOptions) -> Vec<(ObjectId, u32)>,
) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let at_the_end =
            driver.state().turn().step() == Step::EndCombat && driver.engine().decision().is_some();
        if at_the_end {
            return Ok(());
        }
        let answer = match &driver.decision()?.kind {
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(divide(options)),
            _ => Answer::Priority(mtg_engine::decision::PriorityAction::Pass),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the end of combat".into()))
}

/// Player 0 attacks player 1 with every attacker, player 1 blocks as told, combat is played out.
fn fight(
    attackers: &[Unit],
    blockers: &[Unit],
    blocks: &[(&str, &str)],
) -> Result<Driver, ScriptError> {
    let mut driver = duel(attackers, blockers)?;
    attack_with_all(&mut driver, attackers)?;
    driver.block(blocks)?;
    finish(&mut driver, lethal_first)?;
    Ok(driver)
}

fn names(list: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = list.iter().map(|name| (*name).to_owned()).collect();
    names.sort();
    names
}

#[rstest]
// First strike: damage in its own step, with the deaths before the regular one.
#[case::first_strike_kills_before_the_blocker_strikes(("A", 2, 2, "first-strike"), ("B", 2, 2, ""), &["A"], 20, 20)]
#[case::first_strike_that_does_not_kill_is_hit_back(("A", 1, 1, "first-strike"), ("B", 2, 2, ""), &["B"], 20, 20)]
#[case::two_first_strikers_trade(("A", 2, 2, "first-strike"), ("B", 2, 2, "first-strike"), &[], 20, 20)]
#[case::a_first_striking_blocker_kills_the_attacker_first(("A", 2, 2, ""), ("B", 2, 2, "first-strike"), &["B"], 20, 20)]
#[case::first_strike_does_not_strike_twice(("A", 2, 4, "first-strike"), ("B", 2, 9, ""), &["A", "B"], 20, 20)]
// Double strike: twice, with the deaths between.
#[case::double_strike_kills_in_the_first_step(("A", 3, 3, "double-strike"), ("B", 1, 1, ""), &["A"], 20, 20)]
#[case::double_strike_finishes_in_the_second(("A", 2, 3, "double-strike"), ("B", 2, 4, ""), &["A"], 20, 20)]
#[case::double_strike_trades_when_the_blocker_survives_the_first(("A", 2, 2, "double-strike"), ("B", 2, 4, ""), &[], 20, 20)]
#[case::double_strike_against_double_strike(("A", 2, 4, "double-strike"), ("B", 2, 4, "double-strike"), &[], 20, 20)]
// Deathtouch: any damage is lethal.
#[case::deathtouch_kills_a_giant_and_dies_to_it(("A", 1, 1, "deathtouch"), ("B", 5, 5, ""), &[], 20, 20)]
#[case::a_deathtouch_blocker_kills_the_attacker(("A", 5, 5, ""), ("B", 1, 1, "deathtouch"), &[], 20, 20)]
#[case::deathtouch_with_first_strike_survives(("A", 1, 1, "first-strike deathtouch"), ("B", 5, 5, ""), &["A"], 20, 20)]
#[case::zero_power_deathtouch_kills_nothing(("A", 0, 3, "deathtouch"), ("B", 1, 5, ""), &["A", "B"], 20, 20)]
// Indestructible: lethal damage and deathtouch do not destroy.
#[case::indestructible_blocker_shrugs_off_damage(("A", 5, 5, ""), ("B", 2, 2, "indestructible"), &["A", "B"], 20, 20)]
#[case::indestructible_attacker_shrugs_off_damage(("A", 1, 1, "indestructible"), ("B", 3, 3, ""), &["A", "B"], 20, 20)]
#[case::indestructible_shrugs_off_deathtouch(("A", 1, 1, "deathtouch"), ("B", 3, 3, "indestructible"), &["B"], 20, 20)]
#[case::both_indestructible(("A", 9, 1, "indestructible"), ("B", 9, 1, "indestructible"), &["A", "B"], 20, 20)]
// Lifelink: the controller gains what the damage came to, overkill included.
#[case::lifelink_attacker_gains_all_it_dealt(("A", 3, 3, "lifelink"), ("B", 1, 1, ""), &["A"], 23, 20)]
#[case::lifelink_blocker_gains_what_it_dealt(("A", 2, 2, ""), ("B", 2, 2, "lifelink"), &[], 20, 22)]
#[case::lifelink_with_double_strike_gains_twice(("A", 2, 5, "lifelink double-strike"), ("B", 1, 9, ""), &["A", "B"], 24, 20)]
#[case::lifelink_gains_nothing_from_zero_power(("A", 0, 3, "lifelink"), ("B", 1, 9, ""), &["A", "B"], 20, 20)]
// Trample: the damage beyond lethal reaches the player.
#[case::trample_over_a_small_blocker(("A", 5, 5, "trample"), ("B", 2, 2, ""), &["A"], 20, 17)]
#[case::trample_that_cannot_get_past(("A", 2, 2, "trample"), ("B", 3, 3, ""), &["B"], 20, 20)]
#[case::trample_exactly_lethal_assigns_nothing_past(("A", 3, 3, "trample"), ("B", 1, 3, ""), &["A"], 20, 20)]
#[case::deathtouch_trample_needs_one_damage_per_blocker(("A", 3, 3, "trample deathtouch"), ("B", 0, 5, ""), &["A"], 20, 18)]
#[case::trample_with_lifelink_gains_the_whole_power(("A", 4, 4, "trample lifelink"), ("B", 1, 1, ""), &["A"], 24, 17)]
#[case::first_strike_trample(("A", 2, 2, "first-strike trample"), ("B", 1, 1, ""), &["A"], 20, 19)]
#[case::double_strike_trample_has_nothing_left_to_block_the_second_time(("A", 3, 3, "double-strike trample"), ("B", 1, 1, ""), &["A"], 20, 15)]
#[case::trample_against_indestructible_still_needs_lethal(("A", 5, 5, "trample"), ("B", 2, 2, "indestructible"), &["A", "B"], 20, 17)]
fn one_attacker_one_blocker(
    #[case] attacker: Unit,
    #[case] blocker: Unit,
    #[case] alive: &[&str],
    #[case] attacker_life: i32,
    #[case] defender_life: i32,
) -> Result<(), ScriptError> {
    let driver = fight(&[attacker], &[blocker], &[(blocker.0, attacker.0)])?;
    assert_eq!(survivors(&driver), names(alive));
    driver.expect_life(0, attacker_life)?;
    driver.expect_life(1, defender_life)
}

#[rstest]
#[case::plain(("A", 3, 3, ""), 17, 20)]
#[case::first_strike_deals_once(("A", 3, 3, "first-strike"), 17, 20)]
#[case::double_strike_deals_twice(("A", 3, 3, "double-strike"), 14, 20)]
#[case::double_strike_to_exactly_lethal(("A", 10, 3, "double-strike"), 0, 20)]
#[case::deathtouch_is_just_damage_to_a_player(("A", 3, 3, "deathtouch"), 17, 20)]
#[case::trample_with_nobody_in_the_way(("A", 3, 3, "trample"), 17, 20)]
#[case::lifelink_gains_when_it_hits_a_player(("A", 3, 3, "lifelink"), 17, 23)]
#[case::lifelink_double_strike(("A", 3, 3, "lifelink double-strike"), 14, 26)]
#[case::vigilance_changes_nothing_here(("A", 3, 3, "vigilance"), 17, 20)]
fn unblocked_attackers(
    #[case] attacker: Unit,
    #[case] defender_life: i32,
    #[case] attacker_life: i32,
) -> Result<(), ScriptError> {
    let mut driver = duel(&[attacker], &[("B", 1, 1, "")])?;
    attack_with_all(&mut driver, &[attacker])?;
    driver.block(&[])?;
    // The game may end at lethal damage; the life totals tell either way.
    finish(&mut driver, lethal_first).ok();
    driver.expect_life(1, defender_life)?;
    driver.expect_life(0, attacker_life)
}

#[test]
fn a_double_striker_that_is_lethal_in_the_first_step_never_deals_the_second()
-> Result<(), ScriptError> {
    let dealt_by = |power: i32| -> Result<Vec<u32>, ScriptError> {
        let attacker: Unit = ("A", power, 3, "double-strike");
        let mut driver = duel(&[attacker], &[("B", 1, 1, "")])?;
        attack_with_all(&mut driver, &[attacker])?;
        driver.block(&[])?;
        finish(&mut driver, lethal_first).ok();
        Ok(driver
            .engine()
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::DamageDealt { amount, .. } => Some(*amount),
                _ => None,
            })
            .collect())
    };
    // 10 and 10 take the player to 0 in the second step; 20 takes them there in the first, and the game is over.
    assert_eq!(dealt_by(10)?, [10, 10]);
    assert_eq!(dealt_by(20)?, [20]);
    Ok(())
}

#[test]
fn a_vanilla_combat_has_no_first_strike_step_and_any_first_striker_adds_one()
-> Result<(), ScriptError> {
    let steps_of = |attacker: Unit, blocker: Unit| -> Result<Vec<Step>, ScriptError> {
        let driver = fight(&[attacker], &[blocker], &[(blocker.0, attacker.0)])?;
        Ok(driver
            .engine()
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::StepBegan { step } => Some(*step),
                _ => None,
            })
            .filter(|step| matches!(step, Step::FirstStrikeDamage | Step::CombatDamage))
            .collect())
    };
    let both = vec![Step::FirstStrikeDamage, Step::CombatDamage];
    assert_eq!(
        steps_of(("A", 2, 2, ""), ("B", 2, 2, ""))?,
        [Step::CombatDamage]
    );
    assert_eq!(
        steps_of(("A", 2, 2, "first-strike"), ("B", 2, 2, ""))?,
        both
    );
    assert_eq!(
        steps_of(("A", 2, 2, ""), ("B", 0, 2, "first-strike"))?,
        both
    );
    assert_eq!(
        steps_of(("A", 2, 2, "double-strike"), ("B", 2, 2, ""))?,
        both
    );
    // Keywords that are not about striking leave combat as it was.
    assert_eq!(
        steps_of(
            ("A", 2, 2, "deathtouch trample lifelink"),
            ("B", 2, 2, "indestructible")
        )?,
        [Step::CombatDamage]
    );
    Ok(())
}

#[test]
fn the_first_strike_step_has_its_own_priority_round_with_the_deaths_before_the_regular_damage()
-> Result<(), ScriptError> {
    let mut driver = duel(&[("A", 2, 2, "first-strike")], &[("B", 2, 2, "")])?;
    attack_with_all(&mut driver, &[("A", 2, 2, "first-strike")])?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    driver.pass()?;
    // Damage is dealt and the blocker is gone while players hold priority in the first-strike step.
    driver.expect_step(Step::FirstStrikeDamage)?;
    assert_eq!(survivors(&driver), names(&["A"]));
    driver.expect_life(0, 20)?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_step(Step::CombatDamage)?;
    let log = driver.engine().events();
    let position = |wanted: &dyn Fn(&Event) -> bool| log.iter().position(wanted);
    let died = position(&|e| matches!(e, Event::CreatureDied { .. }));
    let regular = position(&|e| {
        matches!(
            e,
            Event::StepBegan {
                step: Step::CombatDamage
            }
        )
    });
    assert!(died.is_some() && died < regular, "{died:?} {regular:?}");
    let struck_by_b = log
        .iter()
        .any(|e| matches!(e, Event::DamageDealt { source, .. } if name_of(driver.state(), *source) == "B"));
    assert!(!struck_by_b, "the blocker died before it could strike");
    Ok(())
}

/// The options of the damage decision the attacker's controller faces, with the engine asked for nothing else.
fn damage_options(driver: &Driver) -> Result<DamageOptions, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::AssignDamage(options) => Ok(options.clone()),
        other => Err(ScriptError::Expectation(format!(
            "a damage decision was expected, not {}",
            other.name()
        ))),
    }
}

/// A combat that waits on player 0's division of damage; `blockers` all block the attacker "A".
fn at_the_division(
    attacker: Unit,
    blockers: Vec<mtg_engine::PresetPermanent>,
    blocks: &[(&str, &str)],
) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, ready_units(&[attacker]))
            .battlefield(1, blockers),
    )?;
    attack_with_all(&mut driver, &[attacker])?;
    driver.block(blocks)?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(0, "assign-damage")?;
    Ok(driver)
}

fn unit(name: &'static str, power: i32, toughness: i32) -> mtg_engine::PresetPermanent {
    ready(free_creature(name, power, toughness))
}

#[test]
fn the_options_say_what_is_lethal_to_each_blocker_and_who_trample_reaches()
-> Result<(), ScriptError> {
    let blockers = vec![
        mtg_testkit::damaged(free_creature("B1", 1, 4), 3),
        unit("B2", 1, 3),
    ];
    let shared = &[("B1", "A"), ("B2", "A")];
    // Without trample: damage already marked counts, and the player is not a recipient.
    let driver = at_the_division(("A", 6, 6, ""), blockers.clone(), shared)?;
    let options = damage_options(&driver)?;
    assert_eq!(
        (options.power, options.excess, options.lethal),
        (6, None, vec![1, 3])
    );
    // With trample the player attacked is the recipient of the rest.
    let driver = at_the_division(("A", 6, 6, "trample"), blockers.clone(), shared)?;
    let options = damage_options(&driver)?;
    assert_eq!(
        (options.excess, options.lethal),
        (Some(DamageTarget::Player(PlayerId(1))), vec![1, 3])
    );
    // Deathtouch makes one damage lethal where more is needed, and none where none is.
    let driver = at_the_division(("A", 6, 6, "trample deathtouch"), blockers, shared)?;
    assert_eq!(damage_options(&driver)?.lethal, [1, 1]);
    Ok(())
}

#[test]
fn a_blocker_already_holding_lethal_damage_needs_none() -> Result<(), ScriptError> {
    let mut indestructible = (*free_creature("B1", 1, 2)).clone();
    indestructible = indestructible.with_rules(Ability::Keyword(Keyword::Indestructible));
    let blockers = vec![
        damaged(std::sync::Arc::new(indestructible), 5),
        unit("B2", 1, 3),
    ];
    let driver = at_the_division(
        ("A", 4, 4, "trample deathtouch"),
        blockers,
        &[("B1", "A"), ("B2", "A")],
    )?;
    assert_eq!(damage_options(&driver)?.lethal, [0, 1]);
    Ok(())
}

#[rstest]
// 6 trample damage against two blockers that each need 2.
#[case::rest_to_the_player(&[("B1", 2), ("B2", 2)], Ok(2))]
#[case::some_past_one_blocker(&[("B1", 2), ("B2", 3)], Ok(1))]
#[case::everything_to_the_blockers(&[("B1", 3), ("B2", 3)], Ok(0))]
#[case::all_on_one_blocker(&[("B1", 6)], Ok(0))]
#[case::the_first_blocker_is_short(&[("B1", 1), ("B2", 3)], Err(IllegalAnswer::ExcessBeforeLethal))]
#[case::nothing_assigned_leaves_all_for_the_player(&[], Err(IllegalAnswer::ExcessBeforeLethal))]
#[case::more_than_the_power(&[("B1", 4), ("B2", 3)], Err(IllegalAnswer::DamageSplit { power: 6, assigned: 7 }))]
fn a_trampler_may_assign_past_its_blockers_only_after_lethal_damage_to_each(
    #[case] shares: &[(&str, u32)],
    #[case] result: Result<u32, IllegalAnswer>,
) -> Result<(), ScriptError> {
    let mut driver = at_the_division(
        ("A", 6, 6, "trample"),
        vec![unit("B1", 1, 2), unit("B2", 1, 2)],
        &[("B1", "A"), ("B2", "A")],
    )?;
    let before = driver.engine().state_hash();
    let outcome = driver.assign_damage(shares);
    match result {
        Ok(to_the_player) => {
            outcome?;
            finish(&mut driver, lethal_first)?;
            driver.expect_life(1, 20 - i32::try_from(to_the_player).expect("small"))?;
        }
        Err(illegal) => {
            assert_eq!(
                outcome.err(),
                Some(ScriptError::Engine(EngineError::Illegal(illegal)))
            );
            assert_eq!(driver.engine().state_hash(), before);
            driver.expect_decision(0, "assign-damage")?;
        }
    }
    Ok(())
}

#[test]
fn without_trample_all_the_damage_goes_to_the_blockers() -> Result<(), ScriptError> {
    let mut driver = at_the_division(
        ("A", 6, 6, ""),
        vec![unit("B1", 1, 2), unit("B2", 1, 2)],
        &[("B1", "A"), ("B2", "A")],
    )?;
    let refused = driver.assign_damage(&[("B1", 2), ("B2", 2)]);
    assert_eq!(
        refused.err(),
        Some(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::DamageSplit {
                power: 6,
                assigned: 4
            }
        )))
    );
    driver.assign_damage(&[("B1", 5), ("B2", 1)])?;
    finish(&mut driver, lethal_first)?;
    driver.expect_life(1, 20)
}

#[test]
fn a_trampler_with_one_blocker_is_asked_only_when_it_has_a_choice() -> Result<(), ScriptError> {
    // Power 2 against toughness 2: the only way to divide it is all to the blocker, so nobody is asked.
    let mut driver = duel(&[("A", 2, 2, "trample")], &[("B", 1, 2, "")])?;
    attack_with_all(&mut driver, &[("A", 2, 2, "trample")])?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    driver.pass()?;
    assert!(!matches!(
        driver.decision()?.kind,
        DecisionKind::AssignDamage(_)
    ));
    // Power 3 against toughness 2: one point may go past, so the controller divides.
    let mut driver = duel(&[("A", 3, 3, "trample")], &[("B", 1, 2, "")])?;
    attack_with_all(&mut driver, &[("A", 3, 3, "trample")])?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    driver.pass()?;
    let options = damage_options(&driver)?;
    assert_eq!((options.power, options.lethal.as_slice()), (3, &[2][..]));
    // Everything on the blocker is as legal as the rest past it.
    driver.assign_damage(&[("B", 3)])?;
    finish(&mut driver, lethal_first)?;
    driver.expect_life(1, 20)
}

#[test]
fn a_double_striker_divides_its_damage_in_each_step() -> Result<(), ScriptError> {
    let mut driver = at_the_division(
        ("A", 2, 2, "double-strike"),
        vec![unit("B1", 1, 9), unit("B2", 1, 9)],
        &[("B1", "A"), ("B2", "A")],
    )?;
    assert_eq!(damage_options(&driver)?.power, 2);
    driver.assign_damage(&[("B1", 1), ("B2", 1)])?;
    driver.expect_step(Step::FirstStrikeDamage)?;
    driver.pass()?;
    driver.pass()?;
    // The second step asks again; the first division is not reused.
    driver.expect_decision(0, "assign-damage")?;
    driver.expect_step(Step::CombatDamage)?;
    driver.assign_damage(&[("B1", 2), ("B2", 0)])?;
    let marked = |name: &str| -> Result<u32, ScriptError> {
        let id = driver.id_of(name)?;
        Ok(driver
            .state()
            .object(id)
            .and_then(|o| o.permanent())
            .map_or(0, mtg_engine::object::PermanentState::damage))
    };
    assert_eq!((marked("B1")?, marked("B2")?), (3, 1));
    Ok(())
}

#[test]
fn trample_reaches_a_planeswalker_when_that_is_what_is_attacked() -> Result<(), ScriptError> {
    let walker = std::sync::Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(5),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, ready_units(&[("A", 4, 4, "trample")]))
            .battlefield(1, [ready(walker), unit("B", 1, 1)]),
    )?;
    let (attacker, walker_id) = (driver.id_of("A")?, driver.id_of("Walker")?);
    driver.answer(Answer::DeclareAttackers(vec![(
        attacker,
        AttackTarget::Planeswalker(walker_id),
    )]))?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(
        damage_options(&driver)?.excess,
        Some(DamageTarget::Object(walker_id))
    );
    driver.assign_damage(&[("B", 1)])?;
    let walker_now = driver.state().object(walker_id).and_then(|o| o.permanent());
    assert_eq!(
        walker_now.map(|p| p.counters_of(&mtg_cards::value::CounterKind::Loyalty)),
        Some(2),
        "3 of the 4 damage went to the planeswalker, which lost that many loyalty counters"
    );
    assert_eq!(
        walker_now.map(mtg_engine::object::PermanentState::damage),
        Some(0)
    );
    driver.expect_life(1, 20)
}

fn cause_of(driver: &Driver, name: &str) -> Option<DeathCause> {
    driver.engine().events().iter().find_map(|e| match e {
        Event::CreatureDied { object, cause } if name_of(driver.state(), *object) == name => {
            Some(*cause)
        }
        _ => None,
    })
}

#[rstest]
#[case::lethal_damage(("A", 3, 3, ""), ("B", 1, 3, ""), "B", Some(DeathCause::LethalDamage))]
#[case::deathtouch_with_one_damage(("A", 1, 1, "deathtouch"), ("B", 1, 9, ""), "B", Some(DeathCause::Deathtouch))]
#[case::deathtouch_and_lethal_is_lethal_damage(("A", 9, 1, "deathtouch"), ("B", 1, 3, ""), "B", Some(DeathCause::LethalDamage))]
#[case::indestructible_does_not_die(("A", 9, 1, "deathtouch"), ("B", 1, 3, "indestructible"), "B", None)]
fn the_log_names_why_a_creature_died(
    #[case] attacker: Unit,
    #[case] blocker: Unit,
    #[case] victim: &str,
    #[case] cause: Option<DeathCause>,
) -> Result<(), ScriptError> {
    let driver = fight(&[attacker], &[blocker], &[(blocker.0, attacker.0)])?;
    assert_eq!(cause_of(&driver, victim), cause);
    Ok(())
}

#[test]
fn indestructible_still_dies_to_zero_toughness_and_a_deathtouch_mark_does_not_outlast_the_turn()
-> Result<(), ScriptError> {
    let mut driver = duel(
        &[("A", 1, 1, "deathtouch")],
        &[("B", 2, 2, "indestructible")],
    )?;
    attack_with_all(&mut driver, &[("A", 1, 1, "deathtouch")])?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let b = driver.id_of("B")?;
    let mark = |driver: &Driver| {
        driver
            .state()
            .object(b)
            .and_then(|o| o.permanent())
            .map(|p| (p.damage(), p.deathtouched()))
    };
    assert_eq!(mark(&driver), Some((1, true)), "it survived, marked");
    // Zero toughness is not destruction: the same creature dies to a -2/-2 effect.
    driver.add_effect(
        driver.on(b),
        Modification::ModifyPowerToughness {
            power: -2,
            toughness: -2,
        },
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(cause_of(&driver, "B"), Some(DeathCause::ZeroToughness));
    // The mark wears off with the damage at the end of the turn.
    let mut driver = duel(
        &[("A", 1, 1, "deathtouch")],
        &[("B", 2, 2, "indestructible")],
    )?;
    attack_with_all(&mut driver, &[("A", 1, 1, "deathtouch")])?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(3, Step::Upkeep)?;
    let b = driver.id_of("B")?;
    let after = driver
        .state()
        .object(b)
        .and_then(|o| o.permanent())
        .map(|p| (p.damage(), p.deathtouched()));
    assert_eq!(after, Some((0, false)));
    Ok(())
}

/// A 1/1 pinger (`{T}: 1 damage to any target`) with these keywords.
fn pinger_with(words: &str) -> std::sync::Arc<CardDefinition> {
    let mut card = (*pinger("Pinger", 0)).clone();
    for keyword in keywords(words) {
        card = card.with_rules(Ability::Keyword(keyword));
    }
    std::sync::Arc::new(card)
}

fn ping(driver: &mut Driver, target: &str) -> Result<(), ScriptError> {
    driver.begin_activation("Pinger")?;
    let at = driver.target_named(target)?;
    driver.choose_targets(&[at])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[test]
fn damage_outside_combat_counts_for_deathtouch_and_lifelink_too() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(pinger_with("deathtouch lifelink"))])
            .battlefield(1, [unit("Giant", 1, 9)])
            .life(0, 10),
    )?;
    ping(&mut driver, "Giant")?;
    assert_eq!(cause_of(&driver, "Giant"), Some(DeathCause::Deathtouch));
    driver.expect_life(0, 11)?;
    // The gain rides in the batch of the damage: nothing separates them.
    let log = driver.engine().events();
    let at = log
        .iter()
        .position(|e| matches!(e, Event::DamageDealt { .. }))
        .expect("the ping dealt damage");
    assert!(
        matches!(log[at + 1], Event::LifeGained { amount: 1, .. }),
        "{:?}",
        log[at + 1]
    );
    // A plain pinger neither kills the giant nor gains life.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(pinger_with(""))])
            .battlefield(1, [unit("Giant", 1, 9)]),
    )?;
    ping(&mut driver, "Giant")?;
    assert_eq!(cause_of(&driver, "Giant"), None);
    driver.expect_life(0, 20)
}

#[test]
fn lifelink_and_the_damage_it_follows_share_one_batch_in_combat() -> Result<(), ScriptError> {
    let mut driver = duel(&[("A", 3, 3, "lifelink")], &[("B", 1, 5, "")])?;
    attack_with_all(&mut driver, &[("A", 3, 3, "lifelink")])?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    let events = driver.pass()?;
    let kinds: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { .. } => Some("damage"),
            Event::LifeGained { .. } => Some("life"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["damage", "life", "damage"], "{events:?}");
    Ok(())
}
