//! Goal: a morph cost that takes more than mana ("Morph—Discard a card", "Morph—Pay 5 life", "Morph—Return two Islands you control to their owner's hand", "Morph—Sacrifice two Mountains") turns the permanent face up as a special action that pays those parts: a pick is asked only when there is a choice, never offers the permanent itself, the parts are paid with the turn up, and a part that cannot be paid keeps the action from being offered; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::{names, pid};

/// A 4/4 creature for {5} whose morph cost is the cost `json`.
fn morph(name: &str, json: &str) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{5}").unwrap());
    let cost: Cost = serde_json::from_str(json).unwrap();
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Morph(cost)))];
    Arc::new(def)
}

/// Seat 0 holds `card` and `hand` and has three Forests and `lands`; every seat has a library.
fn table(
    seats: usize,
    card: Arc<CardDefinition>,
    hand: Vec<Arc<CardDefinition>>,
    lands: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut permanents: Vec<PresetPermanent> = (0..3).map(|_| ready(forest())).collect();
    permanents.extend(lands.into_iter().map(ready));
    let mut held = vec![card];
    held.extend(hand);
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, held)
            .battlefield(0, permanents),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    )
}

/// Seat 0 casts `name` face down with the Forests and lets it resolve; the face-down permanent.
fn face_down(driver: &mut Driver, name: &str) -> Result<ObjectId, ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Cast face down")?;
    }
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent"))
}

/// The face-down permanents the holder of priority may turn face up.
fn turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

#[rstest]
#[case::a_discard_with_one_card_takes_it(r#"[{"Discard":{}}]"#, 1, Vec::new(), 0)]
#[case::a_discard_with_a_choice_asks(r#"[{"Discard":{}}]"#, 2, Vec::new(), 1)]
#[case::a_return_of_two_islands(r#"[{"ReturnToHand":{"count":2,"filter":[{"Subtype":"Island"}]}}]"#, 0, vec![island(), island()], 0)]
#[case::a_sacrifice_of_two_mountains_among_three(r#"[{"Sacrifice":{"count":2,"filter":[{"Subtype":"Mountain"}]}}]"#, 0, vec![mountain(), mountain(), mountain()], 1)]
#[case::life(r#"[{"PayLife":5}]"#, 0, Vec::new(), 0)]
#[case::mana_and_a_discard(r#"[{"Mana":"{G}"},{"Discard":{}}]"#, 1, vec![forest()], 0)]
fn a_morph_cost_beyond_mana_is_paid_as_the_permanent_turns_face_up(
    #[case] json: &str,
    #[case] cards: usize,
    #[case] lands: Vec<Arc<CardDefinition>>,
    #[case] asked: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![mountain(); cards];
    let mut driver = Driver::from_scenario(table(seats, morph("Stone Ogre", json), hand, lands))?;
    let id = face_down(&mut driver, "Stone Ogre")?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(turnable(&driver), [id]);
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    let mut picks = 0;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                assert!(
                    !options.from.contains(&id),
                    "the permanent never pays for itself"
                );
                let take: Vec<ObjectId> = options.from[..options.min as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(take)))?;
                picks += 1;
            }
            DecisionKind::PayMana(_) => {
                pay(&mut driver)?;
            }
            _ => break,
        }
    }
    assert_eq!(picks, asked, "a pick is asked only when there is a choice");
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), None, "{json}");
    assert_eq!(object.card().name, "Stone Ogre");
    assert!(driver.state().stack().is_empty(), "a special action");
    let after = driver.state().player(pid(0)).unwrap().life();
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    match json {
        j if j.contains("Discard") => {
            assert_eq!(hand.len(), cards - 1);
            assert_eq!(graveyard, ["Mountain"]);
        }
        j if j.contains("ReturnToHand") => assert_eq!(hand, ["Island", "Island"]),
        j if j.contains("Sacrifice") => assert_eq!(graveyard, ["Mountain", "Mountain"]),
        _ => assert_eq!(life - after, 5),
    }
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::no_card_to_discard(r#"[{"Discard":{}}]"#, Vec::new())]
#[case::one_island_of_two(r#"[{"ReturnToHand":{"count":2,"filter":[{"Subtype":"Island"}]}}]"#, vec![island()])]
#[case::a_creature_to_sacrifice_is_not_itself(
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#,
    Vec::new()
)]
fn a_morph_cost_whose_part_cannot_be_paid_is_not_offered(
    #[case] json: &str,
    #[case] lands: Vec<Arc<CardDefinition>>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, morph("Stone Ogre", json), Vec::new(), lands))?;
    let id = face_down(&mut driver, "Stone Ogre")?;
    assert_eq!(turnable(&driver), Vec::<ObjectId>::new());
    let refused = driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)));
    assert!(refused.is_err(), "the action is not offered");
    assert!(driver.state().object(id).unwrap().face_down().is_some());
    Ok(())
}
