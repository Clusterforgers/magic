//! Goal: an object tells every seat that sees its card whether a clause left to the table is owed when it resolves, exactly as `ManualClause::owed_at_resolution` decides for each clause shape; the flag is left out while false, an ability on the stack tells it as its source does, and a card no seat may look at carries none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::{CardDefinition, ManualClause};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{by_hand, free_creature, healer};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const OWED: &str = "Whenever this creature dies, each opponent loses 1 life.";
const GAIN: &str = "{T}: Add {C}.";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// The card named `name` in seat 0's hand.
fn in_hand(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .zone(Zone::Hand(seat(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
        .unwrap()
}

fn object(driver: &Driver, viewer: PlayerId, id: ObjectId) -> Option<ObjectView> {
    StateView::for_player(driver.state(), viewer)
        .objects
        .into_iter()
        .find(|object| object.id == id)
}

/// Seat 0 holds `card` in hand and every other seat has a library.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for other in 1..seats {
        scenario = scenario.library(other, vec![free_creature("Filler", 1, 1); 3]);
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::mana_ability(&[GAIN], false)]
#[case::activated_ability(&["{2}, {T}: Draw a card."], false)]
#[case::loyalty_abilities(&["+1: Draw a card.\n\u{2212}2: Destroy target creature."], false)]
#[case::two_standing_clauses(&["{T}: Add {C}.", "{2}, {T}: Draw a card."], false)]
#[case::a_trigger(&[OWED], true)]
#[case::a_trigger_among_standing_clauses(&[GAIN, OWED], true)]
#[case::a_standing_line_among_other_lines(&["{T}: Add {C}.\nWhen ~ enters, you gain 2 life."], true)]
fn every_seat_reads_the_flag_the_server_holds_the_seat_by(
    #[values(2, 3, 4)] seats: usize,
    #[case] clauses: &[&str],
    #[case] owed: bool,
) {
    let card = clauses
        .iter()
        .fold(free_creature("Held", 1, 1), |card, text| {
            by_hand(card, text)
        });
    let expected = card.manual_owed();
    assert_eq!(expected, owed, "{clauses:?}");
    let mut driver = table(seats, vec![card]);
    let id = in_hand(&driver, "Held");
    driver.cast("Held").unwrap();
    for stage in ["on the stack", "on the battlefield"] {
        for viewer in viewers(seats) {
            let shown = object(&driver, viewer, id).unwrap();
            assert_eq!(shown.manual.len(), clauses.len(), "{stage}");
            assert_eq!(shown.manual_owed, owed, "{viewer:?} {stage}: {clauses:?}");
            assert_eq!(
                shown.manual.iter().any(ManualClause::owed_at_resolution),
                shown.manual_owed,
                "the flag is the rule over the clauses shown"
            );
            let json = serde_json::to_value(&shown).unwrap();
            assert_eq!(
                json.get("manual_owed").is_some(),
                owed,
                "left out while false"
            );
        }
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
    }
    assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
}

#[rstest]
fn a_card_with_no_clause_is_not_flagged(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, vec![free_creature("Plain", 1, 1)]);
    let id = in_hand(&driver, "Plain");
    driver.cast("Plain").unwrap();
    for viewer in viewers(seats) {
        let shown = object(&driver, viewer, id).unwrap();
        assert!(shown.manual.is_empty() && !shown.manual_owed);
        let json = serde_json::to_value(&shown).unwrap();
        assert!(json.get("manual_owed").is_none());
    }
}

#[rstest]
fn a_card_in_a_hand_is_flagged_to_nobody_but_its_owner(#[values(2, 3, 4)] seats: usize) {
    let card = by_hand(free_creature("Secret", 1, 1), OWED);
    let driver = table(seats, vec![card]);
    let id = in_hand(&driver, "Secret");
    assert!(object(&driver, seat(0), id).unwrap().manual_owed);
    for viewer in viewers(seats)
        .into_iter()
        .filter(|viewer| *viewer != seat(0))
    {
        assert!(
            object(&driver, viewer, id).is_none(),
            "{viewer:?} sees no hand card"
        );
        let text = serde_json::to_string(&StateView::for_player(driver.state(), viewer)).unwrap();
        assert!(!text.contains("manual_owed"), "{viewer:?} is told nothing");
    }
}

/// A disguise creature for {1}{G}, turned face up for {G}, whose card holds an owed clause.
fn masked_ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Disguise(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    by_hand(Arc::new(def), OWED)
}

#[rstest]
fn a_face_down_card_is_flagged_to_its_controller_alone(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .hand(0, [masked_ape()])
        .battlefield(
            0,
            (0..3)
                .map(|_| ready(mtg_testkit::cards::forest()))
                .collect::<Vec<_>>(),
        )
        .library(0, vec![mtg_testkit::cards::forest(); 3]);
    for other in 1..seats {
        scenario = scenario.library(other, vec![mtg_testkit::cards::forest(); 3]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = in_hand(&driver, "Masked Ape");
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
    assert!(driver.state().object(id).unwrap().face_down().is_some());
    for viewer in viewers(seats) {
        let shown = object(&driver, viewer, id).unwrap();
        assert!(shown.face_down);
        let json = serde_json::to_value(&shown).unwrap();
        if viewer == seat(0) {
            assert!(shown.manual_owed, "its controller reads the card");
        } else {
            assert!(shown.manual.is_empty() && !shown.manual_owed, "{viewer:?}");
            assert!(json.get("manual_owed").is_none(), "{viewer:?}");
        }
    }
}

#[rstest]
#[case::owed(OWED, true)]
#[case::standing(GAIN, false)]
fn an_ability_on_the_stack_is_flagged_as_its_source_is(
    #[values(2, 3, 4)] seats: usize,
    #[case] clause: &str,
    #[case] owed: bool,
) {
    let card = by_hand(healer("Cleric", 0), clause);
    let mut scenario = Scenario::new(seats).battlefield(0, [ready(card)]);
    for other in 0..seats {
        scenario = scenario.library(other, vec![free_creature("Filler", 1, 1); 3]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Cleric").unwrap();
    let stacked = driver.state().stack().to_vec();
    let ability = *stacked.last().unwrap();
    assert_eq!(
        driver.state().object(ability).unwrap().kind(),
        ObjectKind::Ability
    );
    let source = driver.id_of("Cleric").unwrap();
    for viewer in viewers(seats) {
        let shown = object(&driver, viewer, ability).unwrap();
        assert!(shown.manual.is_empty(), "nothing else of the printed side");
        assert_eq!(
            shown.manual_owed, owed,
            "{viewer:?}: it holds its controller as the permanent does"
        );
        let source = object(&driver, viewer, source).unwrap();
        assert_eq!(source.manual_owed, owed, "{viewer:?} reads the permanent");
    }
}
