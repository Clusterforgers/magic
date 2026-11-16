//! Goal: the r67-02 turn-up cards play as they read at two to four seats: Gift of Doom, turned face up for its morph
//! cost, may be attached to a creature it could enchant (never the creature sacrificed to pay that cost, never one it
//! can't enchant) and gives it deathtouch and indestructible, or goes to the graveyard when its controller attaches it to
//! nothing; Deathmist Raptor returns from its owner's graveyard face up or face down, as its owner chooses, when a
//! permanent they control is turned face up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};

/// The name of an object.
fn r6702_name(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// Passes priority until the stack is empty.
fn r6702_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The names offered by the card choice asked now.
fn r6702_offered(driver: &Driver) -> Result<Vec<String>, ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a card choice is asked: {}", driver.decision()?.kind.name());
    };
    let mut names: Vec<String> = options
        .from
        .iter()
        .map(|id| r6702_name(driver, *id))
        .collect();
    names.sort();
    Ok(names)
}

/// Answers the card choice asked now with the cards of these names.
fn r6702_take(driver: &mut Driver, names: &[&str]) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a card choice is asked: {}", driver.decision()?.kind.name());
    };
    let taken: Vec<ObjectId> = options
        .from
        .iter()
        .copied()
        .filter(|id| names.contains(&r6702_name(driver, *id).as_str()))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
    Ok(())
}

/// Seat 0 casts the named card face down with three Mountains, beside a Wolf, a Bear and a Gargoyle of
/// protection from black, and gets priority back with the stack empty.
fn r6702_face_down(seats: usize, name: &str, graveyard: &[&str]) -> Result<Driver, ScriptError> {
    let mut gargoyle = (*free_creature("Gargoyle", 2, 2)).clone();
    gargoyle
        .rules
        .push(std::sync::Arc::new(mtg_cards::ability::Ability::Keyword(
            Keyword::Protection(mtg_cards::refs::Selector(vec![
                mtg_cards::refs::Pred::Color(mtg_core::color::Color::Black),
            ])),
        )));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [card(name).unwrap()])
            .graveyard_named(0, graveyard.iter().copied())
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(mountain()),
                    ready(mountain()),
                    ready(free_creature("Wolf", 2, 2)),
                    ready(free_creature("Bear", 2, 2)),
                    ready(std::sync::Arc::new(gargoyle)),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast face down")?;
    }
    pay(&mut driver)?;
    r6702_settle(&mut driver)?;
    Ok(driver)
}

/// Begins turning seat 0's only face-down permanent face up.
fn r6702_turn_up(driver: &mut Driver) -> Result<ObjectId, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority to turn a permanent face up");
    };
    let id = *options.turnable.first().expect("a face-down permanent");
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    Ok(id)
}

#[rstest]
#[case::onto_the_bear_two_seats(2, Some("Bear"))]
#[case::onto_the_bear_four_seats(4, Some("Bear"))]
#[case::onto_nothing(3, None)]
fn r6702_gift_of_doom_is_attached_as_it_turns_face_up(
    #[case] seats: usize,
    #[case] host: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = r6702_face_down(seats, "Gift of Doom", &[])?;
    let gift = r6702_turn_up(&mut driver)?;
    // Morph—Sacrifice another creature.
    r6702_take(&mut driver, &["Wolf"])?;
    // The Wolf is being sacrificed and the Gargoyle has protection from black: neither is offered.
    assert_eq!(r6702_offered(&driver)?, vec!["Bear".to_owned()]);
    r6702_take(&mut driver, host.as_slice())?;
    r6702_settle(&mut driver)?;
    let state = driver.state();
    let me = PlayerId::from_index(0);
    assert!(
        state
            .zone(Zone::Graveyard(me))
            .iter()
            .any(|id| r6702_name(&driver, *id) == "Wolf")
    );
    if let Some(host) = host {
        let bear = driver.id_of(host)?;
        let chars = mtg_engine::characteristics(state, bear).unwrap();
        assert!(chars.keywords.contains(&Keyword::Deathtouch));
        assert!(chars.keywords.contains(&Keyword::Indestructible));
        assert!(state.zone(Zone::Battlefield).contains(&gift));
    } else {
        assert!(
            state
                .zone(Zone::Graveyard(me))
                .iter()
                .any(|id| r6702_name(&driver, *id) == "Gift of Doom")
        );
    }
    Ok(())
}

#[rstest]
#[case::face_up_two_seats(2, "Face up", true)]
#[case::face_down_three_seats(3, "Face down", false)]
#[case::face_up_four_seats(4, "Face up", true)]
fn r6702_deathmist_raptor_returns_when_a_permanent_turns_face_up(
    #[case] seats: usize,
    #[case] way: &str,
    #[case] face_up: bool,
) -> Result<(), ScriptError> {
    let mut driver = r6702_face_down(seats, "Gift of Doom", &["Deathmist Raptor"])?;
    r6702_turn_up(&mut driver)?;
    r6702_take(&mut driver, &["Wolf"])?;
    r6702_take(&mut driver, &["Bear"])?;
    // The Raptor's trigger: yes, then the way it returns.
    for _ in 0..20 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(true))?;
            }
            // The first option returns it face up, the second face down.
            DecisionKind::ChooseOption(options) => {
                assert_eq!(options.options.len(), 2, "{way}");
                let at = usize::from(!face_up);
                driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let raptor = driver.id_of("Deathmist Raptor")?;
    let object = driver.state().object(raptor).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.face_down().is_none(), face_up);
    Ok(())
}
