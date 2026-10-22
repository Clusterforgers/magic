//! Goal: a face-down creature is a 2/2 on the battlefield for every seat to see, and only its controller sees which card it is: the other seats and a spectator get an object with no name, cost or abilities, in the live view, the event batches of the cast and the replay of the log; turning it face up names it to everyone, and so does its leaving the battlefield for a graveyard or exile; a disguise shows its ward to every seat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_protocol::{EventBatch, EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, morph_fuzz_setup, ready};
use rstest::rstest;
use serde_json::Value;

use crate::common::{run_game, with_unique_names};

const CARD: &str = "Nantuko Vigilante";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// A disguise creature that is a 3/3 for {1}{G} and turned face up for {G}.
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

/// Seat 0 holds `card` and six Forests; every other seat has a hand of its own.
fn table(seats: usize, card: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand(0, [card])
        .battlefield(0, (0..6).map(|_| ready(forest())).collect::<Vec<_>>());
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 6])
            .hand_named(other, ["Serra Angel", "Doom Blade"]);
    }
    scenario
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn object_of(batch: &EventBatch, id: ObjectId) -> Option<&ObjectView> {
    batch.objects.iter().find(|object| object.id == id)
}

/// Seat 0 casts the card face down and lets it resolve; the id of the permanent.
fn cast_face_down(driver: &mut Driver, card: &str) -> ObjectId {
    let id = driver
        .state()
        .zone(Zone::Hand(seat(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == card)
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(id)))
        .unwrap();
    if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
        let at = options.options.len() - 1;
        driver
            .answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(at)))
            .unwrap();
    }
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    id
}

#[rstest]
fn only_the_controller_sees_which_card_lies_face_down(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats, corpus_card(CARD));
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = cast_face_down(&mut driver, CARD);
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    assert_eq!(after.object(id).unwrap().zone(), Zone::Battlefield);
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees the permanent");
        assert_eq!(object.zone, Zone::Battlefield);
        assert!(object.face_down, "it lies face down for {viewer:?}");
        // The characteristics of a face-down creature are public (CR 708.2a).
        assert_eq!(object.types, [CardType::Creature]);
        assert_eq!((object.power, object.toughness), (Some(2), Some(2)));
        assert_eq!(object.colors.len(), 0);
        assert_eq!(object.mana_cost.is_some(), owner, "the cost is on the card");
        assert!(object.permanent.is_some());
        assert_eq!(
            batch_names(batch, CARD),
            owner,
            "who is told the name: {viewer:?}"
        );
        if owner {
            assert_eq!(object.name, CARD);
            assert_eq!(object.keywords, ["Morph"], "the controller reads the card");
        } else {
            assert_eq!(object.name, "");
            assert!(object.abilities.is_empty() && object.keywords.is_empty());
        }
    }
    // The state a client resumes from, and a replay of the log, tell the same.
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    for state in [&after, &again] {
        for viewer in &all {
            let view = StateView::for_player(state, *viewer);
            let object = view.objects.iter().find(|object| object.id == id).unwrap();
            assert_eq!(object.name == CARD, *viewer == seat(0));
            assert_eq!(
                names(&serde_json::to_value(&view).unwrap(), CARD),
                *viewer == seat(0)
            );
        }
    }
}

#[rstest]
fn turning_it_face_up_names_it_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats, corpus_card(CARD))).unwrap();
    let id = cast_face_down(&mut driver, CARD);
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver
        .answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let events = driver.engine().events()[logged..].to_vec();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::TurnedFaceUp { object, .. }) if *object == id
            )),
            "{viewer:?} sees the card turned up"
        );
        let object = object_of(batch, id).expect("the permanent changed for everyone");
        assert_eq!(object.name, CARD, "{viewer:?} reads the card");
        assert!(!object.face_down);
        assert_eq!((object.power, object.toughness), (Some(3), Some(2)));
    }
}

#[rstest]
#[case::dying(Zone::Graveyard(seat(0)), ZoneCause::Destroyed)]
#[case::exiled(Zone::Exile, ZoneCause::Exiled)]
fn leaving_the_battlefield_for_an_open_zone_names_it_to_every_seat(
    #[case] to: Zone,
    #[case] cause: ZoneCause,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, corpus_card(CARD))).unwrap();
    let id = cast_face_down(&mut driver, CARD);
    let before = driver.state().clone();
    let event = Event::ZoneChanged {
        object: id,
        from: Zone::Battlefield,
        to,
        cause,
    };
    let all = viewers(seats);
    let batches = project_events(&before, &[event], &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let object = object_of(batch, id).expect("the card is in the open");
        assert_eq!(object.name, CARD, "{viewer:?} reads the card");
        assert!(!object.face_down);
        assert_eq!(object.zone, to);
    }
}

#[rstest]
fn a_disguise_shows_its_ward_to_every_seat_and_its_card_to_the_controller(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, masked_ape())).unwrap();
    let id = cast_face_down(&mut driver, "Masked Ape");
    for viewer in viewers(seats) {
        let object = ObjectView::of(driver.state(), viewer, id).unwrap();
        assert!(object.face_down);
        if viewer == seat(0) {
            assert_eq!(object.name, "Masked Ape");
            assert_eq!(object.keywords, ["Disguise"]);
        } else {
            assert_eq!(object.name, "");
            assert_eq!(object.keywords, ["Ward"]);
        }
    }
}

/// The names of the cards that lie face down in `state` and that `viewer` may not look at.
fn face_down_names(state: &GameState, viewer: PlayerId) -> Vec<String> {
    state
        .objects()
        .filter(|o| o.face_down().is_some() && !state.may_look_at(viewer, o.id()))
        .map(|o| o.whole_card().name.clone())
        .collect()
}

/// Games with the morph cards under unique names: no part of what a seat is told names a card that lies face down after the
/// answer and that the seat may not look at.
#[rstest]
#[case(2, 41)]
#[case(3, 42)]
#[case(4, 43)]
fn no_face_down_card_reaches_a_seat_that_may_not_look_at_it(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let all = viewers(players);
    let (mut cast, mut checked) = (0usize, 0usize);
    let setup = with_unique_names(morph_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step| {
        cast += step
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::SpellCast { choices, .. }
                        if choices.way == Some(mtg_cards::value::CastWay::FaceDown)
                )
            })
            .count();
        let batches = project_events(step.before, step.events, &all, step.from_seq).unwrap();
        for (viewer, batch) in all.iter().zip(&batches) {
            for name in face_down_names(step.after, *viewer) {
                checked += 1;
                assert!(
                    !batch_names(batch, &name),
                    "seed {seed} answer {} viewer {viewer:?}: the batch names {name}",
                    step.answers
                );
            }
            let view = StateView::for_player(step.after, *viewer);
            for name in face_down_names(step.after, *viewer) {
                assert!(
                    !names(&serde_json::to_value(&view).unwrap(), &name),
                    "seed {seed} answer {} viewer {viewer:?}: the view names {name}",
                    step.answers
                );
            }
        }
    });
    assert!(engine.result().is_some(), "the game finishes");
    assert!(
        cast > 0 && checked > 0,
        "the test is not vacuous: {cast} {checked}"
    );
}

fn corpus_card(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}
