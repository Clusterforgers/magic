//! Goal: the rules that read a card with several faces read the face in use: a back face land enters tapped, or tapped unless the player controls two basic lands, by its own replacement effect and not the front face's, a back face on the battlefield leaves as its front face, and the exile of an Adventure that resolves is a replacement ordered with Rest in Peace by its controller (either way the card is exiled and none is put into a graveyard; only the Adventure's own exile leaves it on an adventure).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::definition::Layout;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::ReplacementChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::removal::resolve_the_stack;
use crate::common::{name_of, pid, spell};

const TAPPED: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"Tapped"}}"#;
const TAPPED_UNLESS_TWO_BASICS: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"Tapped","condition":{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},{"Supertype":"Basic"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}}}}"#;

/// A modal double-faced card whose front face is a creature with `front` and whose back face is a land with `back`.
fn modal_land(front: Option<&str>, back: Option<&str>) -> Arc<CardDefinition> {
    let with = |mut card: CardDefinition, rule: Option<&str>| {
        if let Some(json) = rule {
            let ability: Ability = serde_json::from_str(json).expect("the rule is DSL");
            card = card.with_rules(ability);
        }
        card
    };
    let first = with(
        CardDefinition::new("Front Beast")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_power_toughness(2, 2),
        front,
    );
    let second = with(
        CardDefinition::new("Back Grove").with_type(CardType::Land),
        back,
    );
    Arc::new(
        first
            .clone()
            .with_faces(Layout::ModalDfc, vec![first, second]),
    )
}

fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::the_back_face_enters_tapped(None, Some(TAPPED), &[], true)]
#[case::unless_two_basic_lands_it_is_tapped_with_none(None, Some(TAPPED_UNLESS_TWO_BASICS), &[], true)]
#[case::with_one_basic_land_it_is_tapped(None, Some(TAPPED_UNLESS_TWO_BASICS), &["Forest"], true)]
#[case::with_two_basic_lands_it_is_not(None, Some(TAPPED_UNLESS_TWO_BASICS), &["Forest", "Island"], false)]
#[case::the_front_face_effect_is_not_the_lands(Some(TAPPED), None, &[], false)]
#[case::the_front_face_effect_is_not_the_lands_either(Some(TAPPED), Some(TAPPED_UNLESS_TWO_BASICS), &["Forest", "Island"], false)]
fn the_enter_replacement_of_a_back_face_land_is_read_from_the_face_it_is_played_as(
    #[values(2, 3, 4)] seats: usize,
    #[case] front: Option<&str>,
    #[case] back: Option<&str>,
    #[case] others: &[&str],
    #[case] tapped_after: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let others = others.iter().map(|name| match *name {
            "Forest" => ready(forest()),
            _ => ready(island()),
        });
        Scenario::new(seats)
            .hand(0, [modal_land(front, back)])
            .battlefield(0, others)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let card = in_zone(&driver, Zone::Hand(pid(0)), "Front Beast").unwrap();
    driver.answer(Answer::Priority(PriorityAction::PlayLandFace(card, 1)))?;
    // Nothing to ask, whichever face's effect applies.
    assert!(!driver.engine().replacing());
    assert_eq!(tapped(&driver, "Back Grove"), tapped_after);
    replays(&driver, &setup);
    Ok(())
}

/// A free instant that either destroys target land or returns it to its owner's hand.
fn demolish(effect: &str) -> Arc<CardDefinition> {
    spell(
        "Demolish",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Land"}]}}]"#,
        effect,
    )
}

#[rstest]
#[case::destroyed(r#"[{"Destroy":{"what":{"Target":0}}}]"#, false)]
#[case::returned_to_the_hand(r#"[{"Move":{"what":{"Target":0},"to":"Hand"}}]"#, true)]
fn a_back_face_on_the_battlefield_leaves_as_its_front_face(
    #[values(2, 3, 4)] seats: usize,
    #[case] effect: &str,
    #[case] to_hand: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [modal_land(None, None), demolish(effect)])
            .battlefield(0, [ready(mountain())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let card = in_zone(&driver, Zone::Hand(pid(0)), "Front Beast").unwrap();
    driver.answer(Answer::Priority(PriorityAction::PlayLandFace(card, 1)))?;
    let land = driver.id_of("Back Grove")?;
    assert_eq!(driver.state().object(land).unwrap().face(), Some(1));
    let target = driver.target_named("Back Grove")?;
    driver.cast_at("Demolish", &[target])?;
    resolve_the_stack(&mut driver)?;
    let zone = if to_hand {
        Zone::Hand(pid(0))
    } else {
        Zone::Graveyard(pid(0))
    };
    let left = in_zone(&driver, zone, "Front Beast").expect("it is its front face again");
    assert_eq!(driver.state().object(left).unwrap().face(), None);
    assert!(in_zone(&driver, zone, "Back Grove").is_none());
    assert!(in_zone(&driver, Zone::Battlefield, "Back Grove").is_none());
    replays(&driver, &setup);
    Ok(())
}

/// Casts Stomp at the second player with Rest in Peace in play and answers the order of the two replacement effects
/// with the card that `first` names; returns the driver after the spell resolved.
fn stomp_under_rest_in_peace(seats: usize, first: &str) -> Result<(Driver, bool), ScriptError> {
    let scenario = Scenario::new(seats)
        .library(0, vec![forest(); 12])
        .library(1, vec![forest(); 12])
        .hand_named(0, ["Bonecrusher Giant // Stomp"])
        .battlefield(0, (0..5).map(|_| ready(mountain())))
        .battlefield_named(1, ["Rest in Peace"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Bonecrusher Giant")?;
    choose_way(&mut driver, "Stomp")?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    let asked = resolve_asking(&mut driver, first)?;
    Ok((driver, asked))
}

/// Passes until the spell resolved, answering the one question of the replacement step with the effect of the card
/// named `first`; says whether the question was asked.
fn resolve_asking(driver: &mut Driver, first: &str) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseReplacement(options) => {
                asked = true;
                assert_eq!(options.candidates.len(), 2, "{options:?}");
                let at = options
                    .candidates
                    .iter()
                    .position(|c| name_of(driver.state(), c.source) == first)
                    .expect("the effect is offered");
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(at)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(asked)
}

#[rstest]
#[case::the_adventure_first("Stomp", true)]
#[case::rest_in_peace_first("Rest in Peace", false)]
fn an_adventure_and_rest_in_peace_are_ordered_and_the_card_is_exiled_either_way(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: &str,
    #[case] on_an_adventure: bool,
) -> Result<(), ScriptError> {
    let (driver, asked) = stomp_under_rest_in_peace(seats, first)?;
    assert!(asked, "two effects replace the move to the graveyard");
    let card =
        in_zone(&driver, Zone::Exile, "Bonecrusher Giant").expect("exiled, never in a graveyard");
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .all(|id| name_of(driver.state(), *id) != "Bonecrusher Giant")
    );
    let adventure = driver.state().object(card).unwrap().on_adventure();
    assert_eq!(adventure, on_an_adventure.then_some(pid(0)));
    // Only a card on an adventure is offered as the creature.
    let offered = match &driver.decision()?.kind {
        DecisionKind::Priority(options) => options.castable.contains(&card),
        other => panic!("priority: {}", other.name()),
    };
    assert_eq!(offered, on_an_adventure);
    Ok(())
}

#[rstest]
fn an_adventure_with_nothing_else_replacing_the_graveyard_asks_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .library(0, vec![forest(); 12])
        .library(1, vec![forest(); 12])
        .hand_named(0, ["Bonecrusher Giant // Stomp"])
        .battlefield(0, (0..5).map(|_| ready(mountain())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Bonecrusher Giant")?;
    choose_way(&mut driver, "Stomp")?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    let asked = resolve_asking(&mut driver, "Bonecrusher Giant")?;
    assert!(!asked);
    let card = in_zone(&driver, Zone::Exile, "Bonecrusher Giant").unwrap();
    assert_eq!(
        driver.state().object(card).unwrap().on_adventure(),
        Some(pid(0))
    );
    Ok(())
}
