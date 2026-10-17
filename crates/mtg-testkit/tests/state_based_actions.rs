//! Goal: state-based actions fire for every loss and death condition and are rechecked until nothing more changes, in the order the rules require.

mod common;

use common::{names, pid};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::events::DeathCause;
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameResult};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready};
use rstest::rstest;

fn deaths(driver: &Driver) -> Vec<(String, DeathCause)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::CreatureDied { object, cause } => {
                Some((common::name_of(driver.state(), *object), *cause))
            }
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::untouched(2, 2, 0, true)]
#[case::scratched(2, 2, 1, true)]
#[case::exactly_lethal(2, 2, 2, false)]
#[case::overkill(2, 2, 7, false)]
#[case::one_one_dies(1, 1, 1, false)]
#[case::big_survives(0, 9, 8, true)]
#[case::zero_toughness(3, 0, 0, false)]
fn creatures_die_when_damage_reaches_toughness(
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] damage: u32,
    #[case] survives: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [damaged(free_creature("Subject", power, toughness), damage)],
    ))?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield) == ["Subject"],
        survives
    );
    if !survives {
        driver.expect_zone(Zone::Graveyard(pid(0)), &["Subject"])?;
        let cause = if toughness <= 0 {
            DeathCause::ZeroToughness
        } else {
            DeathCause::LethalDamage
        };
        assert_eq!(deaths(&driver), [("Subject".to_owned(), cause)]);
    }
    Ok(())
}

#[rstest]
#[case::minus_one_on_one_one(1, 1, -1, false)]
#[case::minus_one_on_two_two(2, 2, -1, true)]
#[case::minus_two_on_two_two(2, 2, -2, false)]
#[case::plus_never_kills(1, 1, 5, true)]
#[case::overkill_debuff(4, 4, -9, false)]
fn debuffs_that_zero_toughness_kill_without_damage(
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] delta: i32,
    #[case] survives: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(free_creature("Subject", power, toughness))]),
    )?;
    let subject = driver.id_of("Subject")?;
    driver.add_effect(
        Selector::Object(subject),
        Modification::ModifyPowerToughness {
            power: delta,
            toughness: delta,
        },
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield) == ["Subject"],
        survives
    );
    if !survives {
        assert_eq!(
            deaths(&driver),
            [("Subject".to_owned(), DeathCause::ZeroToughness)]
        );
    }
    Ok(())
}

#[test]
fn a_death_can_cause_another_death_in_the_next_pass() -> Result<(), ScriptError> {
    // T is 2/2 but "Source" gives it +0/+2 for as long as Source lives.
    // Combat deals T 3 damage (survivable with toughness 4) and kills Source at the same time.
    // Once Source is gone the buff lapses and the SBA fixpoint must also kill T.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("Source", 1, 1)),
                    ready(free_creature("T", 2, 2)),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Kills Source", 2, 2)),
                    ready(free_creature("Hits T", 3, 3)),
                ],
            ),
    )?;
    let (source, t) = (driver.id_of("Source")?, driver.id_of("T")?);
    driver.add_effect(
        Selector::Object(t),
        Modification::ModifyPowerToughness {
            power: 0,
            toughness: 2,
        },
        Duration::WhileSourceOnBattlefield(source),
    )?;
    driver.attack(&[("Source", 1), ("T", 1)])?;
    driver.pass()?;
    driver.pass()?;
    driver.block(&[("Kills Source", "Source"), ("Hits T", "T")])?;
    driver.pass()?;
    let events = driver.pass()?;
    let story: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::CreatureDied { object, .. } => {
                Some(format!("{} died", common::name_of(driver.state(), *object)))
            }
            Event::EffectExpired { .. } => Some("buff lapsed".to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(
        story,
        ["Source died", "T died", "buff lapsed"],
        "T dies in the second pass; the lapsed effect is only pruned afterwards"
    );
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        ["Kills Source", "Hits T"]
    );
    assert!(
        driver.state().effects().is_empty(),
        "the lapsed effect was cleaned up"
    );
    Ok(())
}

#[rstest]
#[case::zero(0)]
#[case::negative(-4)]
fn a_life_total_of_zero_or_less_loses(#[case] life: i32) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).life(1, life))?;
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(0))));
    driver.expect_event("player 1 lost to zero life", |e| {
        matches!(e, Event::PlayerLost { player, reason: LossReason::ZeroLife } if *player == pid(1))
    })
}

#[test]
fn positive_life_never_loses() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(3).life(0, 1).life(1, 1).life(2, 1))?;
    assert_eq!(driver.engine().result(), None);
    driver.expect_no_event("any loss", |e| matches!(e, Event::PlayerLost { .. }))
}

#[rstest]
#[case::empty_library_on_first_draw(0, true)]
#[case::one_card_left(1, false)]
fn drawing_from_an_empty_library_loses(
    #[case] library: usize,
    #[case] loses: bool,
) -> Result<(), ScriptError> {
    // Turn 2 is player 1's, so they draw in the draw step.
    let scenario = Scenario::new(2)
        .at(2, 1, Step::Untap)
        .library(1, (0..library).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Draw).ok();
    assert_eq!(driver.engine().result().is_some(), loses);
    if loses {
        assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(0))));
        driver.expect_event("empty-library loss", |e| {
            matches!(e, Event::PlayerLost { player, reason: LossReason::EmptyLibrary } if *player == pid(1))
        })?;
    }
    Ok(())
}

#[test]
fn the_empty_library_loss_comes_one_draw_late() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Untap)
            .library(1, [forest()]),
    )?;
    assert_eq!(
        driver.engine().result(),
        None,
        "the last card was drawn without losing"
    );
    driver.idle_until(4, Step::Main1).ok();
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(0))));
    Ok(())
}

#[test]
fn simultaneous_losses_are_resolved_in_one_pass() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(4)
            .life(1, 0)
            .life(2, -1)
            .battlefield(0, [damaged(free_creature("Doomed", 1, 1), 1)]),
    )?;
    let first_pass: Vec<&Event> = driver
        .engine()
        .events()
        .iter()
        .take_while(|e| !matches!(e, Event::ObjectLeftGame { .. }))
        .filter(|e| matches!(e, Event::PlayerLost { .. } | Event::CreatureDied { .. }))
        .collect();
    assert_eq!(
        first_pass.len(),
        3,
        "both players and the creature are handled together: {first_pass:?}"
    );
    assert_eq!(driver.engine().result(), None, "players 0 and 3 remain");
    Ok(())
}
