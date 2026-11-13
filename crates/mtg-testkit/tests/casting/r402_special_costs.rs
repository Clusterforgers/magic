//! Goal: changes of the costs of special actions play at tables of two to four seats: Inquisitive Glimmer takes {1} from
//! unlocking the doors of its controller's Rooms (an opponent's Rooms aside, and never below the colored mana), and
//! Exiled Doomsayer adds {2} to turning a face-down permanent up for its morph cost whoever controls either, so a player
//! with the mana for the morph cost alone can no longer turn it up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay, resolve};

/// The priority options of the pending decision.
fn r402_options(driver: &Driver) -> Result<PriorityOptions, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => Ok(options.clone()),
        other => panic!("no priority decision: {other:?}"),
    }
}

/// The cost of unlocking each locked door seat 0 may unlock, by door name.
fn r402_unlock_costs(driver: &Driver) -> Result<Vec<(String, String)>, ScriptError> {
    let mut costs: Vec<(String, String)> = r402_options(driver)?
        .unlockable
        .iter()
        .map(|face| {
            (
                face.name.clone(),
                face.cost.as_ref().map(ToString::to_string).unwrap_or_default(),
            )
        })
        .collect();
    costs.sort();
    Ok(costs)
}

#[rstest]
#[case::two_seats_own_glimmer(2, Some(0), "{2}{W}", "{1}{W}")]
#[case::three_seats_no_glimmer(3, None, "{3}{W}", "{2}{W}")]
#[case::four_seats_an_opponents_glimmer(4, Some(2), "{3}{W}", "{2}{W}")]
fn r402_glimmer_takes_one_from_unlock_costs(
    #[case] seats: usize,
    #[case] glimmer: Option<usize>,
    #[case] theater: &str,
    #[case] prop_room: &str,
) -> Result<(), ScriptError> {
    let room = card("Dazzling Theater // Prop Room").unwrap();
    let mut mine = vec![ready(room)];
    mine.extend(lands(6, plains));
    let mut scenario = Scenario::new(seats);
    if let Some(seat) = glimmer.filter(|seat| *seat != 0) {
        scenario = scenario.battlefield(seat, vec![ready(card("Inquisitive Glimmer").unwrap())]);
    } else if glimmer.is_some() {
        mine.push(ready(card("Inquisitive Glimmer").unwrap()));
    }
    let driver = Driver::from_scenario(scenario.battlefield(0, mine))?;
    let parse = |text: &str| mtg_core::mana::ManaCost::parse(text).unwrap().to_string();
    assert_eq!(
        r402_unlock_costs(&driver)?,
        vec![
            ("Dazzling Theater".to_owned(), parse(theater)),
            ("Prop Room".to_owned(), parse(prop_room)),
        ]
    );
    Ok(())
}

/// A 3/3 for {1}{G} with morph {G}.
fn r402_ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Morph(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    Arc::new(def)
}

/// Seat 0 casts the Ape face down with three of `forests` Forests (the Doomsayer on seat `doomsayer`, if any), and
/// whether it can then turn the Ape face up.
fn r402_can_turn_up(
    seats: usize,
    forests: usize,
    doomsayer: Option<usize>,
) -> Result<bool, ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, vec![r402_ape()]);
    let mut mine = lands(forests, forest);
    match doomsayer {
        Some(0) => mine.push(ready(card("Exiled Doomsayer").unwrap())),
        Some(seat) => {
            scenario =
                scenario.battlefield(seat, vec![ready(card("Exiled Doomsayer").unwrap())]);
        }
        None => {}
    }
    let mut driver = Driver::from_scenario(scenario.battlefield(0, mine))?;
    driver.begin_cast("Masked Ape")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast face down")?;
    }
    pay(&mut driver)?;
    resolve(&mut driver)?;
    let ape = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .unwrap();
    Ok(r402_options(&driver)?.turnable.contains(&ape))
}

#[rstest]
#[case::no_doomsayer_one_left(2, 4, None, true)]
#[case::own_doomsayer_one_left(2, 4, Some(0), false)]
#[case::own_doomsayer_three_left(3, 6, Some(0), true)]
#[case::an_opponents_doomsayer_two_left(4, 5, Some(3), false)]
#[case::an_opponents_doomsayer_three_left(4, 6, Some(1), true)]
fn r402_doomsayer_adds_two_to_morph_costs(
    #[case] seats: usize,
    #[case] forests: usize,
    #[case] doomsayer: Option<usize>,
    #[case] turnable: bool,
) -> Result<(), ScriptError> {
    assert_eq!(r402_can_turn_up(seats, forests, doomsayer)?, turnable);
    Ok(())
}
