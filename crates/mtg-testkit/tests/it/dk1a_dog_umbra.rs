//! Goal: the compiled Dog Umbra plays as the rules say: on a creature of another player it can't attack or block and the Aura has no umbra armor; on a creature of its own controller the creature fights freely and the Aura has umbra armor, which saves the creature from destruction at the price of the Aura; a change of control flips both.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Modification;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// A free instant that destroys target creature.
fn doom() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Doom")
            .with_type(mtg_core::card::CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// Seat 0 controls the Bear and Dog Umbra; seat 1 the Hound and the Wolf, and a Doom. The Aura is on the Bear
/// (`own`) or on the Hound.
fn table(own: bool, turn: u32, active: usize, step: Step) -> Result<Driver, ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    let host = if own { (0, 0) } else { (1, 0) };
    let aura = attached_to(ready(card("Dog Umbra").unwrap()), host.0, host.1);
    Driver::from_scenario(
        Scenario::new(2)
            .at(turn, active, step)
            .library(0, filler())
            .library(1, filler())
            .battlefield(0, [ready(free_creature("Bear", 2, 2)), aura])
            .battlefield(
                1,
                [
                    ready(free_creature("Hound", 2, 2)),
                    ready(free_creature("Wolf", 2, 2)),
                ],
            )
            .hand(1, [doom()]),
    )
}

fn offers(driver: &Driver) -> Vec<String> {
    let options = match &driver.decision().unwrap().kind {
        DecisionKind::DeclareAttackers(options) => options
            .attackers
            .iter()
            .map(|option| option.object)
            .collect::<Vec<_>>(),
        DecisionKind::DeclareBlockers(options) => options
            .blockers
            .iter()
            .map(|option| option.object)
            .collect(),
        other => panic!("a declaration is pending, not {other:?}"),
    };
    let mut names: Vec<String> = options
        .into_iter()
        .map(|object| name_of(driver.state(), object))
        .collect();
    names.sort();
    names
}

fn umbra(driver: &Driver) -> bool {
    has_keyword(
        driver.state(),
        driver.id_of("Dog Umbra").unwrap(),
        &Keyword::UmbraArmor,
    )
}

#[rstest]
#[case::bear_on_its_own_turn(true, 2, 0, &["Bear"])]
#[case::wolf_and_hound_without_the_aura_on_them(true, 3, 1, &["Hound", "Wolf"])]
#[case::hound_wearing_it_on_its_own_turn(false, 3, 1, &["Wolf"])]
#[case::bear_unaffected_by_the_aura_on_the_hound(false, 2, 0, &["Bear"])]
fn only_a_creature_of_another_player_is_barred_from_attacking(
    #[case] own: bool,
    #[case] turn: u32,
    #[case] active: usize,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    let driver = table(own, turn, active, Step::DeclareAttackers)?;
    assert_eq!(offers(&driver), offered);
    Ok(())
}

#[rstest]
#[case::the_hound_wears_it(false, &["Wolf"])]
#[case::the_bear_wears_it(true, &["Hound", "Wolf"])]
fn only_a_creature_of_another_player_is_barred_from_blocking(
    #[case] own: bool,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = table(own, 2, 0, Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    for _ in 0..3 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(offers(&driver), offered);
    Ok(())
}

#[rstest]
#[case::its_own_creature(true, true)]
#[case::a_creature_of_another_player(false, false)]
fn umbra_armor_is_only_on_the_aura_of_the_controllers_creature(
    #[case] own: bool,
    #[case] armored: bool,
) -> Result<(), ScriptError> {
    let driver = table(own, 2, 0, Step::DeclareAttackers)?;
    assert_eq!(umbra(&driver), armored);
    Ok(())
}

#[rstest]
#[case::on_the_controllers_creature(true, "Bear", true)]
#[case::on_a_creature_of_another_player(false, "Hound", false)]
fn destruction_is_replaced_only_by_umbra_armor(
    #[case] own: bool,
    #[case] victim: &str,
    #[case] survives: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(own, 3, 1, Step::Main1)?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Doom", &[target])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let field = names(driver.state(), Zone::Battlefield);
    assert_eq!(field.contains(&victim.to_owned()), survives, "{field:?}");
    // The Aura goes with the creature either way, to the graveyard of its owner.
    assert!(!field.contains(&"Dog Umbra".to_owned()), "{field:?}");
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Dog Umbra".to_owned()));
    Ok(())
}

#[test]
fn a_change_of_control_turns_the_restriction_on_and_the_armor_off() -> Result<(), ScriptError> {
    let mut driver = table(true, 2, 0, Step::DeclareAttackers)?;
    assert!(umbra(&driver));
    let bear = driver.id_of("Bear")?;
    driver.add_effect(
        driver.on(bear),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    assert!(!umbra(&driver));
    assert_eq!(
        names(driver.state(), Zone::Battlefield).len(),
        4,
        "nothing left the battlefield"
    );
    Ok(())
}
