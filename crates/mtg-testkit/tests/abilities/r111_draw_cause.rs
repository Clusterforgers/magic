//! Goal: a draw knows the ability that causes it (CR 614.11) at 2 to 4 seats: a replacement of the draw of a cycling
//! ability of another nonland card (`ReplaceWhen::R111DrawBy`, Unpredictable Cyclone's shape with the found card put
//! into the hand) exiles until a card that shares a card type with the cycled card (`Dealer`, as it was) and puts the
//! rest on the bottom; the cycling of a land card draws as usual; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

/// Unpredictable Cyclone's replacement with the found card put into the hand in place of the free cast.
const REPLACEMENT: &str = r#"{"Replacement":{"when":{"R111DrawBy":{"kind":"Cycling","what":["Other",{"Not":{"Type":"Land"}}]}},"by":{"Instead":[{"ExileUntil":{"filter":[{"SharesTypeOf":{"with":"Dealer","types":["Artifact","Battle","Creature","Enchantment","Instant","Kindred","Land","Planeswalker","Sorcery"]}}],"as":"found","exiled_as":"exiled"}},{"Move":{"what":{"Bound":"found"},"to":"Hand"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}}"#;

fn cyclone() -> Arc<CardDefinition> {
    let mut definition = Arc::unwrap_or_clone(card("Unpredictable Cyclone").unwrap());
    let ability: Ability = serde_json::from_str(REPLACEMENT).unwrap();
    definition.rules = vec![Arc::new(ability)];
    definition.manual.clear();
    Arc::new(definition)
}

/// Seat 0 controls the cyclone with a Swamp and a Plains, holds `cycled`, and its library is (top first) a Forest,
/// Grizzly Bears, then two Forests.
fn table(seats: usize, cycled: &str) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, [cycled])
        .battlefield(0, [ready(cyclone()), ready(swamp()), ready(plains())])
        .library(
            0,
            [forest(), forest(), card("Grizzly Bears").unwrap(), forest()],
        )
}

/// Cycles the card in hand and plays on until the stack is empty.
fn cycle(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn cycling_a_nonland_card_exiles_until_a_shared_card_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, "Lurching Rotbeast").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Lurching Rotbeast"))?;
    cycle(&mut driver, "Lurching Rotbeast")?;
    // The draw was replaced: the Bears came by the replacement, the Forest exiled before them went to the bottom.
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Grizzly Bears"]);
    let library = names(driver.state(), Zone::Library(pid(0)));
    assert_eq!(library, ["Forest", "Forest", "Forest"]);
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn cycling_a_land_card_draws(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let setup = table(seats, "Secluded Steppe").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Secluded Steppe"))?;
    cycle(&mut driver, "Secluded Steppe")?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Forest"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
    replays(&driver, &setup);
    Ok(())
}
