//! Goal: three shapes the translator reads in round 13 play at tables of two, three and four seats: a replacement that
//! waits for the tapped state of its source, a static that waits for the top card of the library, and a spell that makes
//! every other creature a copy of a target until end of turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::copying::settle;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// "As long as this is tapped, other permanents enter tapped. As long as this is untapped, other permanents enter untapped."
fn archelos() -> Arc<CardDefinition> {
    let tapped_state = r#"{"Matches":{"selector":["Tapped"],"subject":"This"}}"#;
    Arc::new(
        CardDefinition::new("Gatekeeper")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3)
            .with_rules(ability(&format!(
                r#"{{"Replacement":{{"by":"Tapped","condition":{tapped_state},"when":{{"Enters":{{"what":[]}}}}}}}}"#
            )))
            .with_rules(ability(&format!(
                r#"{{"Replacement":{{"by":"Untapped","condition":{{"Not":{tapped_state}}},"when":{{"Enters":{{"what":[]}}}}}}}}"#
            ))),
    )
}

#[rstest]
fn permanents_enter_tapped_while_the_source_is_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] source_tapped: bool,
) -> Result<(), ScriptError> {
    let source = if source_tapped {
        tapped(archelos())
    } else {
        ready(archelos())
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [free_creature("Bear", 2, 2)])
            .battlefield(0, [source]),
    )?;
    driver.cast("Bear")?;
    settle(&mut driver)?;
    let bear = driver.id_of("Bear")?;
    let enters_tapped = driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(enters_tapped, source_tapped);
    Ok(())
}

/// "As long as the top card of your library is black, this gets +2/+1."
fn nocturnus() -> Arc<CardDefinition> {
    let top_black = r#"{"Compare":{"left":{"Count":[{"In":"Library"},{"OwnedBy":"You"},"TopOfLibrary",{"Color":"Black"}]},"op":"Ge","right":1}}"#;
    Arc::new(
        CardDefinition::new("Nocturne")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(&format!(
                r#"{{"Static":{{"affected":"This","condition":{top_black},"mods":[{{"ModifyPT":{{"power":2,"toughness":1}}}}]}}}}"#
            ))),
    )
}

#[rstest]
fn a_static_reads_the_top_card_of_the_library(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] black: bool,
) -> Result<(), ScriptError> {
    let top = Arc::new(
        CardDefinition::new("Top")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[if black { Color::Black } else { Color::Green }]))
            .with_power_toughness(1, 1),
    );
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .library(0, [top, free_creature("Under", 1, 1)])
            .battlefield(0, [ready(nocturnus())]),
    )?;
    let id = driver.id_of("Nocturne")?;
    let stats = characteristics(driver.state(), id).unwrap();
    let expected = if black { (4, 3) } else { (2, 2) };
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(expected.0), Some(expected.1))
    );
    Ok(())
}

/// "Each other creature becomes a copy of target creature until end of turn."
fn weaver() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Weaver")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(
                r#"{"Spell":{"effects":[{"BecomeCopy":{"duration":"UntilEndOfTurn","of":{"Target":0},"what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#,
            )),
    )
}

#[rstest]
fn every_other_creature_becomes_a_copy_of_the_target(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [weaver()])
            .battlefield(
                0,
                [
                    ready(free_creature("Giant", 5, 5)),
                    ready(free_creature("Mouse", 1, 1)),
                ],
            )
            .battlefield(1, [ready(free_creature("Rat", 1, 2))]),
    )?;
    let giant = driver.id_of("Giant")?;
    let others = [driver.id_of("Mouse")?, driver.id_of("Rat")?];
    driver.begin_cast("Weaver")?;
    driver.choose_targets(&[mtg_engine::choose::Target::Object(
        driver.state().object(giant).unwrap().reference(),
    )])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    settle(&mut driver)?;
    for id in others {
        let stats = characteristics(driver.state(), id).unwrap();
        assert_eq!(
            (stats.power, stats.toughness),
            (Some(5), Some(5)),
            "a creature that is not the target is a copy of it"
        );
    }
    let stats = characteristics(driver.state(), giant).unwrap();
    assert_eq!(stats.power, Some(5));
    Ok(())
}
