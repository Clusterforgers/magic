//! Goal: the identity of a card (its oracle id and printing) reaches exactly the viewers who may identify the card: every seat and a spectator for a public permanent, the owner alone for a card in hand, the controller alone for a face-down creature; a card with no identity (a basic built in a test, a token) leaves the field out of the JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::{CardDefinition, CardIdentity};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

fn identity(oracle_id: &str, printing_id: Option<&str>) -> CardIdentity {
    CardIdentity {
        oracle_id: oracle_id.to_owned(),
        printing_id: printing_id.map(str::to_owned),
    }
}

fn identified(card: &Arc<CardDefinition>, id: &CardIdentity) -> Arc<CardDefinition> {
    let mut def = (**card).clone();
    def.name = format!("{} {}", def.name, id.oracle_id);
    def.identity = Some(id.clone());
    Arc::new(def)
}

/// A disguise creature that is a 3/3 for {1}{G} and turned face up for {G}.
fn masked_ape(id: &CardIdentity) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Disguise(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    def.identity = Some(id.clone());
    Arc::new(def)
}

/// Seat 0 holds an identified Bear and a Masked Ape and has six Forests; seat 1 has an identified Bear in play.
fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand(
            0,
            [
                identified(&bear(), &identity("oracle-hand", None)),
                masked_ape(&identity("oracle-ape", Some("printing-ape"))),
            ],
        )
        .battlefield(0, (0..6).map(|_| ready(forest())).collect::<Vec<_>>())
        .battlefield(
            1,
            [ready(identified(&bear(), &identity("field", Some("p2"))))],
        );
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 6]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn named(view: &StateView, name: &str) -> Option<ObjectView> {
    view.objects.iter().find(|o| o.name == name).cloned()
}

#[rstest]
fn a_public_card_is_identified_to_every_viewer_and_a_hand_card_to_its_owner(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats);
    for viewer in viewers(seats) {
        let view = StateView::for_player(driver.state(), viewer);
        let field = named(&view, "Grizzly Bears field").unwrap();
        assert_eq!(field.identity, Some(identity("field", Some("p2"))));
        let hand = named(&view, "Grizzly Bears oracle-hand");
        if viewer == seat(0) {
            assert_eq!(hand.unwrap().identity, Some(identity("oracle-hand", None)));
        } else {
            assert!(hand.is_none(), "{viewer:?} sees no card of seat 0's hand");
            let text = serde_json::to_string(&view).unwrap();
            assert!(!text.contains("oracle-hand") && !text.contains("oracle-ape"));
        }
        let land = view.objects.iter().find(|o| o.name == "Forest").unwrap();
        let json = serde_json::to_value(land).unwrap();
        assert!(
            json.get("identity").is_none(),
            "a card with no identity leaves the field out"
        );
    }
}

/// Casts the Masked Ape face down for {3} and resolves it.
fn cast_face_down(driver: &mut Driver) -> ObjectId {
    let id = driver
        .state()
        .zone(Zone::Hand(seat(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Masked Ape")
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
fn a_face_down_creature_is_identified_to_its_controller_alone(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats);
    let id = cast_face_down(&mut driver);
    for viewer in viewers(seats) {
        let object = ObjectView::of(driver.state(), viewer, id).unwrap();
        assert!(object.face_down);
        if viewer == seat(0) {
            assert_eq!(
                object.identity,
                Some(identity("oracle-ape", Some("printing-ape")))
            );
        } else {
            assert_eq!(object.identity, None, "{viewer:?} may not look at it");
            let text =
                serde_json::to_string(&StateView::for_player(driver.state(), viewer)).unwrap();
            assert!(!text.contains("oracle-ape") && !text.contains("printing-ape"));
        }
    }
}
