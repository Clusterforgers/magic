//! Goal: a card with morph, megamorph or disguise is cast face down from the hand for {3} as a 2/2 creature spell with no name, cost, colour, type besides creature or ability (ward {2} for a disguise), and becomes a face-down permanent that only its controller may look at; turning it face up is a special action for the keyword's cost that does not use the stack, needs that cost, gives the card its characteristics and abilities (a megamorph also a +1/+1 counter) and triggers "when turned face up"; a face-down permanent that leaves the battlefield is its card again; a replay rebuilds the game; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::{FaceDown, PermanentState};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::{name_of, names, pid};

/// `count` Forests and a Mountain for seat 0, untapped.
fn lands(count: usize) -> Vec<PresetPermanent> {
    let mut lands: Vec<PresetPermanent> = (0..count).map(|_| ready(forest())).collect();
    lands.push(ready(mountain()));
    lands
}

/// A disguise creature: a 3/3 for {1}{G} that is turned face up for {G}.
fn masked_ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Disguise(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    Arc::new(def)
}

/// Seat 0 holds `cards` and `forests` Forests and a Mountain in the first main phase of turn 1; seat 1 has Sol Ring.
fn table(seats: usize, cards: &[&str], forests: usize) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand_named(0, cards.iter().copied())
            .battlefield(0, lands(forests))
            .battlefield_named(1, ["Sol Ring"]),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    )
}

/// Lets the stack resolve: priority passes until it is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts `name` face down and lets it resolve, returning the events of the cast.
fn cast_face_down(driver: &mut Driver, name: &str) -> Result<Vec<Event>, ScriptError> {
    let logged = driver.engine().events().len();
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Cast face down")?;
    }
    pay(driver)?;
    let cast = driver.engine().events()[logged..].to_vec();
    settle(driver)?;
    Ok(cast)
}

/// The face-down permanent on the battlefield.
fn face_down_permanent(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent")
}

/// The permanents seat 0 controls that are tapped.
fn tapped(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(0) && o.permanent().is_some_and(PermanentState::tapped))
        .count()
}

/// The face-down permanents the holder of priority may turn face up now.
fn turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

/// Turns the permanent `id` face up and pays for it, then answers the target of a trigger with Sol Ring when asked.
fn turn_face_up(driver: &mut Driver, id: ObjectId) -> Result<Vec<Event>, ScriptError> {
    let logged = driver.engine().events().len();
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    pay(driver)?;
    assert!(
        driver.state().stack().is_empty(),
        "turning a permanent face up does not use the stack"
    );
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let ring = driver.target_named("Sol Ring")?;
        driver.choose_targets(&[ring])?;
    }
    let events = driver.engine().events()[logged..].to_vec();
    settle(driver)?;
    Ok(events)
}

#[rstest]
#[case::a_morph_with_a_trigger("Nantuko Vigilante")]
#[case::a_megamorph("Ainok Survivalist")]
#[case::a_morph_of_another_colour("Exalted Angel")]
#[case::a_vanilla_morph("Krosan Colossus")]
#[case::a_land("Zoetic Cavern")]
fn a_card_with_morph_is_cast_face_down_for_three_as_a_nameless_2_2(
    #[case] card: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[card], 6))?;
    let cast = cast_face_down(&mut driver, card)?;
    assert!(cast.iter().any(|event| matches!(
        event,
        Event::SpellCast { choices, .. } if choices.way == Some(CastWay::FaceDown)
    )));
    assert_eq!(tapped(&driver), 3, "{{3}} was paid");
    let id = face_down_permanent(&driver);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.face_down(), Some(FaceDown::Morph));
    assert_eq!(object.whole_card().name, card, "the card is still there");
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!(object.card().name, "");
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    assert!(chars.colors.is_empty() && chars.subtypes.is_empty());
    assert_eq!(keywords_of(driver.state(), id).len(), 0);
    assert!(driver.state().may_look_at(pid(0), id));
    for seat in 1..seats {
        assert!(!driver.state().may_look_at(pid(seat), id), "seat {seat}");
    }
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        Vec::<String>::new()
    );
    Ok(())
}

#[rstest]
#[case::a_morph("Nantuko Vigilante", 3, 2, 0)]
#[case::a_megamorph("Ainok Survivalist", 3, 2, 1)]
fn turning_a_card_face_up_costs_its_cost_without_the_stack_and_triggers(
    #[case] card: &str,
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] counters: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[card], 6))?;
    cast_face_down(&mut driver, card)?;
    let id = face_down_permanent(&driver);
    assert_eq!(turnable(&driver), [id]);
    let events = turn_face_up(&mut driver, id)?;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TurnedFaceUp { object, .. } if *object == id))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. })),
        "a special action is not a spell"
    );
    assert_eq!(tapped(&driver), 5, "{{3}} and {{1}}{{G}} were paid");
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), None);
    assert_eq!(object.card().name, card);
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(power), Some(toughness))
    );
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        counters
    );
    // "When turned face up, destroy target artifact": Sol Ring is gone.
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Sol Ring"]);
    assert!(driver.state().may_look_at(pid(1), id));
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn turning_a_card_face_up_needs_its_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Two Forests and the Mountain: the cast takes all three, so {1}{G} is not left.
    let mut driver = Driver::from_scenario(table(seats, &["Nantuko Vigilante"], 2))?;
    cast_face_down(&mut driver, "Nantuko Vigilante")?;
    let id = face_down_permanent(&driver);
    assert_eq!(tapped(&driver), 3);
    assert_eq!(
        turnable(&driver),
        Vec::<ObjectId>::new(),
        "{{1}}{{G}} is not there"
    );
    let refused = driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)));
    assert!(refused.is_err(), "the action is not offered");
    Ok(())
}

#[rstest]
fn a_disguise_has_ward_while_face_down_and_not_after(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [masked_ape()])
            .battlefield(0, lands(6)),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    cast_face_down(&mut driver, "Masked Ape")?;
    let id = face_down_permanent(&driver);
    assert_eq!(
        driver.state().object(id).unwrap().face_down(),
        Some(FaceDown::Disguise)
    );
    let wards = |driver: &Driver| {
        keywords_of(driver.state(), id)
            .iter()
            .filter(|keyword| matches!(keyword, Keyword::Ward(_)))
            .count()
    };
    assert_eq!(wards(&driver), 1);
    turn_face_up(&mut driver, id)?;
    assert_eq!(wards(&driver), 0);
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    assert_eq!(name_of(driver.state(), id), "Masked Ape");
    Ok(())
}

#[rstest]
fn a_face_down_permanent_that_dies_is_its_card_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Nantuko Vigilante", "Lightning Bolt"], 6);
    let mut driver = Driver::from_scenario(scenario)?;
    cast_face_down(&mut driver, "Nantuko Vigilante")?;
    let id = face_down_permanent(&driver);
    let target = Target::Object(driver.state().object(id).unwrap().reference());
    driver.begin_cast("Lightning Bolt")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.sort();
    assert_eq!(graveyard, ["Lightning Bolt", "Nantuko Vigilante"]);
    let card = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|card| name_of(driver.state(), *card) == "Nantuko Vigilante")
        .unwrap();
    assert_eq!(driver.state().object(card).unwrap().face_down(), None);
    for seat in 1..seats {
        assert!(driver.state().may_look_at(pid(seat), card));
    }
    Ok(())
}
