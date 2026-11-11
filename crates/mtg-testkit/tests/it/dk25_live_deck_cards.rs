//! Goal: the live-deck cards of pack dk25 play as printed: protection from each color outside the controller's commander identity stops exactly the sources of such a color (every color without a commander, never by another player's commander), Commander's Plate gives it with +3/+3 to the creature it equips, Sliver Legion counts every other Sliver on the battlefield for each Sliver creature, and Necrologia is cast only in its caster's end step for X life and draws X.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::combat::to_blockers;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::Supertype;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature, keyworded, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A legendary 2/2 of `colors` for one generic more, to stand as a commander.
fn dk25_commander(colors: &[Color]) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::clone(&creature("Cmd", 1, colors, 2, 2))
            .with_supertype(Supertype::Legendary),
    )
}

/// Seat 0 attacks seat 1's Guard (protection from each color outside its controller's commander identity) with a 4/4
/// of `attacker`; the Guard blocks, and the damage marked on it is what its protection let through.
#[rstest]
#[case::outside_the_identity(Some(Color::Red), None, Color::Green, 0)]
#[case::inside_the_identity(Some(Color::Red), None, Color::Red, 4)]
#[case::no_commander_protects_from_every_color(None, None, Color::Red, 0)]
#[case::the_attackers_commander_does_not_count(None, Some(Color::Green), Color::Green, 0)]
fn dk25_protection_from_colors_outside_the_commander_identity(
    #[case] mine: Option<Color>,
    #[case] theirs: Option<Color>,
    #[case] attacker: Color,
    #[case] marked: u32,
) -> Result<(), ScriptError> {
    let guard = keyworded(
        "Guard",
        1,
        9,
        &[Keyword::Protection(Selector(vec![
            Pred::OutsideCommanderIdentity,
        ]))],
    );
    let mut scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .library(0, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .library(1, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .battlefield(0, [ready(creature("Ogre", 0, &[attacker], 4, 4))])
        .battlefield(1, [ready(guard)]);
    if let Some(color) = mine {
        scenario = scenario.commanders(1, [dk25_commander(&[color])]);
    }
    if let Some(color) = theirs {
        scenario = scenario.commanders(0, [dk25_commander(&[color])]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Ogre", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Guard", "Ogre")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let guard = driver.id_of("Guard")?;
    let dealt = driver
        .state()
        .object(guard)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(dealt, Some(marked));
    Ok(())
}

/// Commander's Plate's static ability names the creature it equips: +3/+3 and the protection above.
#[test]
fn dk25_commanders_plate_gives_its_bonus_and_protection_to_the_equipped_creature() {
    let plate = card("Commander's Plate").unwrap();
    let static_mods = serde_json::to_string(&plate.rules).unwrap();
    assert!(
        static_mods.contains(r#""affected":"Attached""#),
        "{static_mods}"
    );
    assert!(
        static_mods.contains(r#""OutsideCommanderIdentity""#),
        "{static_mods}"
    );
    assert!(
        static_mods.contains(r#""power":3,"toughness":3"#),
        "{static_mods}"
    );
}

/// A Sliver creature of `slivers` Slivers under Sliver Legion gets +1/+1 for each other Sliver, the Legion included.
#[rstest]
fn dk25_sliver_legion_counts_every_other_sliver(
    #[values(0, 1, 3)] others: usize,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let sliver =
        || Arc::new(CardDefinition::clone(&free_creature("Muscle", 1, 1)).with_subtype("Sliver"));
    let mut mine = vec![ready(card("Sliver Legion").unwrap())];
    mine.extend((0..others).map(|_| ready(sliver())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    if seats > 2 {
        scenario = scenario.battlefield(seats - 1, [ready(sliver())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let all = others + 1 + usize::from(seats > 2);
    let bonus = i32::try_from(all - 1).unwrap();
    let legion = driver.id_of("Sliver Legion")?;
    let stats = mtg_engine::characteristics(driver.state(), legion).unwrap();
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(7 + bonus), Some(7 + bonus))
    );
    Ok(())
}

/// Necrologia is cast only during its caster's end step: X life is paid as it is cast and X cards are drawn.
#[rstest]
#[case::the_end_step(0, Step::End, true)]
#[case::the_first_main_phase(0, Step::Main1, false)]
#[case::another_players_end_step(1, Step::End, false)]
fn dk25_necrologia_draws_x_for_x_life_in_its_casters_end_step(
    #[case] active: usize,
    #[case] step: Step,
    #[case] castable: bool,
    #[values(0, 2)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .at(2, active, step)
        .hand(0, [card("Necrologia").unwrap()])
        .library(0, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .library(1, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .battlefield(0, (0..5).map(|_| ready(swamp())));
    let mut driver = Driver::from_scenario(scenario)?;
    if active != 0 {
        driver.pass()?;
    }
    // The hand once Necrologia has left it for the stack.
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).len() - 1;
    let started = driver.begin_cast("Necrologia");
    assert_eq!(started.is_ok(), castable);
    if !castable {
        return Ok(());
    }
    let life = driver.state().player(PlayerId(0)).map(|p| p.life());
    loop {
        match driver.decision().ok().map(|d| d.kind.clone()) {
            Some(DecisionKind::ChooseNumber(_)) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            Some(DecisionKind::PayMana(_)) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let x_life = i32::try_from(x).unwrap();
    assert_eq!(
        driver.state().player(PlayerId(0)).map(|p| p.life()),
        life.map(|life| life - x_life)
    );
    assert_eq!(
        driver.state().zone(Zone::Hand(PlayerId(0))).len(),
        hand + x as usize
    );
    Ok(())
}
