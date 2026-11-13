//! Goal: the rider of a spell copy plays at tables of two to four seats: a copy of a creature spell with Nalfeshnee's
//! rider becomes a token creature with haste that is sacrificed at the beginning of the end step (whose turn the rider
//! names decides which end step), a copy of an instant is copied whole and resolves twice, and Nalfeshnee itself does not
//! copy a spell cast from a hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, pid};

/// An enchantment that copies each spell its controller casts with Nalfeshnee's rider (`whose` end step: "You" or
/// none for each end step).
fn r402_copier(whose: Option<&str>) -> Arc<CardDefinition> {
    let when = match whose {
        Some(whose) => format!(r#"{{"StepBegins":{{"step":"End","whose":"{whose}"}}}}"#),
        None => r#"{"StepBegins":{"step":"End"}}"#.to_owned(),
    };
    let json = format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[],"by":"You"}}}},"effects":[{{"CopySpell":{{"what":"Triggering","new_targets":true,"except":[{{"AddKeyword":"Haste"}},{{"GrantAbility":{{"Triggered":{{"when":{when},"effects":[{{"Sacrifice":{{"filter":["This"]}}}}]}}}}}}]}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Copier")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// The Grizzly Bears on the battlefield, as (is a token, has haste).
fn r402_bears(driver: &Driver) -> Vec<(bool, bool)> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == "Grizzly Bears")
        .map(|id| {
            let token = state.object(id).unwrap().kind() == ObjectKind::Token;
            let haste = keywords_of(state, id).contains(&&Keyword::Haste);
            (token, haste)
        })
        .collect()
}

/// Answers passively until the stack is empty and a player has priority.
fn r402_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..64 {
        if driver.state().zone(Zone::Stack).is_empty()
            && matches!(
                driver.decision()?.kind,
                mtg_engine::decision::DecisionKind::Priority(_)
            )
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// Seat 0, the active player, controls `copier` and lands and casts `spell` from its hand.
fn r402_cast_with(
    seats: usize,
    copier: Arc<CardDefinition>,
    spell: Arc<CardDefinition>,
    targets: &[usize],
) -> Result<Driver, ScriptError> {
    let mut permanents = lands(4, forest);
    permanents.extend(lands(4, mountain));
    permanents.push(ready(copier));
    let name = spell.name.clone();
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![spell]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(&name)?;
    for seat in targets {
        let target = driver.target_player(*seat);
        driver.choose_targets(&[target])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r402_settle(&mut driver)?;
    Ok(driver)
}

#[rstest]
#[case::two_seats_each_end_step(2, None)]
#[case::three_seats_your_end_step(3, Some("You"))]
#[case::four_seats_each_end_step(4, None)]
fn r402_a_creature_copy_is_a_hasty_token_sacrificed_at_the_end_step(
    #[case] seats: usize,
    #[case] whose: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = r402_cast_with(seats, r402_copier(whose), bear(), &[])?;
    let mut bears = r402_bears(&driver);
    bears.sort_unstable();
    assert_eq!(bears, vec![(false, false), (true, true)]);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(r402_bears(&driver), vec![(false, false)]);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r402_an_instant_copy_resolves_as_the_spell(#[case] seats: usize) -> Result<(), ScriptError> {
    let driver = r402_cast_with(seats, r402_copier(None), burn("Shock Bolt", 3), &[1])?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 14);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn r402_nalfeshnee_leaves_a_spell_cast_from_a_hand(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let driver = r402_cast_with(seats, card("Nalfeshnee").unwrap(), bear(), &[])?;
    assert_eq!(r402_bears(&driver), vec![(false, false)]);
    Ok(())
}
