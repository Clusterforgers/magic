//! Goal: jump-start and retrace cast a card from its owner's graveyard for its printed mana cost and the discard of a card (a land card for retrace): the cast is offered only with a card (a land) to discard and the mana of the cost, and a cast that cannot be paid is refused without a trace; the discard is picked among the right cards of the hand and paid with the cast; a jump-started card is exiled after it resolves and one cast from the hand is not, a retraced card returns to the graveyard and can be retraced again, and a card exiled by jump-start cannot be cast that way again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;
use std::sync::Arc;

/// A card of the hand by name: the basic lands are built, anything else is a corpus card.
fn card(name: &str) -> Arc<CardDefinition> {
    match name {
        "Island" => island(),
        "Mountain" => mountain(),
        other => corpus::card(other).unwrap(),
    }
}

/// The first object called `name` in `zone`.
fn find(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

fn count(state: &GameState, zone: Zone, name: &str) -> usize {
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

/// Seat 0 has `card` in its graveyard (or its hand), `hand` in its hand, and the lands in play.
fn game(
    seats: usize,
    named: &str,
    in_hand: bool,
    hand: &[&str],
    islands: usize,
    mountains: usize,
) -> Result<Driver, ScriptError> {
    let lands = (0..islands)
        .map(|_| ready(island()))
        .chain((0..mountains).map(|_| ready(mountain())));
    let mut cards: Vec<Arc<CardDefinition>> = hand.iter().map(|name| card(name)).collect();
    let mut scenario = Scenario::new(seats);
    if in_hand {
        cards.push(card(named));
    } else {
        scenario = scenario.graveyard(0, [card(named)]);
    }
    Driver::from_scenario(scenario.hand(0, cards).battlefield(0, lands))
}

/// The cast of the card is offered from where it is.
fn castable(driver: &Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

/// The cards the pending question offers, by name, sorted.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    from
}

/// Seat 0 casts `name`, picks the first card called `discard` when it is asked, aims at seat 1 when asked and pays.
fn cast(driver: &mut Driver, name: &str, discard: Option<&str>) -> Result<Vec<Event>, ScriptError> {
    driver.begin_cast(name)?;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(_) => {
                let pick = find(
                    driver.state(),
                    Zone::Hand(pid(0)),
                    discard.expect("a discard was asked"),
                );
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(1);
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => return driver.answer(Answer::PayMana(PayChoice::Auto)),
            other => panic!("unexpected question {other:?}"),
        }
    }
}

/// Passes priority until the stack is empty.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::jump_start_with_a_card_and_the_mana("Radical Idea", &["Staggershock"], 2, 0, true)]
#[case::jump_start_discarding_a_land("Radical Idea", &["Island"], 2, 0, true)]
#[case::jump_start_with_an_empty_hand("Radical Idea", &[], 2, 0, false)]
#[case::jump_start_one_land_short("Radical Idea", &["Staggershock"], 1, 0, false)]
#[case::jump_start_with_mana_of_the_wrong_colour("Radical Idea", &["Staggershock"], 0, 2, false)]
#[case::retrace_with_a_land("Flame Jab", &["Island"], 0, 1, true)]
#[case::retrace_with_a_spell_and_a_land("Flame Jab", &["Staggershock", "Island"], 0, 1, true)]
#[case::retrace_with_a_spell_only("Flame Jab", &["Staggershock"], 0, 1, false)]
#[case::retrace_with_an_empty_hand("Flame Jab", &[], 0, 1, false)]
#[case::retrace_without_the_mana("Flame Jab", &["Island"], 2, 0, false)]
fn the_cast_is_offered_with_a_card_to_discard_and_the_mana_and_refused_without_a_trace(
    #[case] named: &str,
    #[case] hand: &[&str],
    #[case] islands: usize,
    #[case] mountains: usize,
    #[case] payable: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, named, false, hand, islands, mountains)?;
    assert_eq!(castable(&driver, named), payable);
    let events = driver.engine().events().len();
    let hash = driver.state().state_hash();
    if payable {
        driver.begin_cast(named)?;
    } else {
        assert!(driver.begin_cast(named).is_err());
        assert_eq!(driver.engine().events().len(), events, "nothing is logged");
        assert_eq!(driver.state().state_hash(), hash, "nothing changed");
    }
    Ok(())
}

#[rstest]
#[case::jump_start_offers_every_card("Radical Idea", &["Island", "Island", "Staggershock"])]
#[case::retrace_offers_the_land_cards_only("Flame Jab", &["Island", "Island"])]
fn the_discard_is_picked_from_the_hand_and_paid_with_the_cast(
    #[case] named: &str,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = game(2, named, false, &["Island", "Island", "Staggershock"], 2, 2)?;
    driver.begin_cast(named)?;
    assert_eq!(offered(&driver), expected);
    let logged = driver.engine().events().len();
    let pick = find(driver.state(), Zone::Hand(pid(0)), "Island");
    driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "the pick waits for the payment"
    );
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let target = driver.target_player(1);
        driver.choose_targets(&[target])?;
    }
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let way = if named == "Radical Idea" {
        CastWay::JumpStart
    } else {
        CastWay::Retrace
    };
    assert!(events.iter().any(|event| matches!(
        event,
        Event::SpellCast { choices, .. } if choices.way == Some(way)
    )));
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand - 1);
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), "Island"), 1);
    assert_eq!(
        count(driver.state(), Zone::Graveyard(pid(0)), "Island"),
        1,
        "the discard is a cost"
    );
    find(driver.state(), Zone::Stack, named);
    Ok(())
}

#[rstest]
#[case::jump_start_from_the_hand_is_not_exiled("Radical Idea", true, Zone::Graveyard(pid(0)))]
#[case::jump_start_from_the_graveyard_is_exiled("Radical Idea", false, Zone::Exile)]
#[case::retrace_from_the_hand_goes_to_the_graveyard("Flame Jab", true, Zone::Graveyard(pid(0)))]
#[case::retrace_from_the_graveyard_returns_there("Flame Jab", false, Zone::Graveyard(pid(0)))]
fn the_card_is_exiled_after_it_resolves_only_when_jump_started(
    #[case] named: &str,
    #[case] in_hand: bool,
    #[case] ends: Zone,
) -> Result<(), ScriptError> {
    let mut driver = game(2, named, in_hand, &["Island"], 2, 2)?;
    let events = cast(&mut driver, named, Some("Island"))?;
    let alternative = events.iter().any(|event| {
        matches!(
            event,
            Event::SpellCast { choices, .. } if choices.way.is_some()
        )
    });
    assert_eq!(alternative, !in_hand);
    resolve(&mut driver)?;
    find(driver.state(), ends, named);
    assert_eq!(
        count(driver.state(), Zone::Exile, named),
        usize::from(ends == Zone::Exile)
    );
    assert_eq!(
        count(driver.state(), Zone::Hand(pid(0)), "Island"),
        usize::from(in_hand)
    );
    Ok(())
}

#[test]
fn the_spells_do_what_they_say() -> Result<(), ScriptError> {
    let mut driver = game(2, "Flame Jab", false, &["Island"], 0, 1)?;
    cast(&mut driver, "Flame Jab", Some("Island"))?;
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 19);
    let mut driver = game(2, "Direct Current", false, &["Island"], 0, 3)?;
    cast(&mut driver, "Direct Current", Some("Island"))?;
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    Ok(())
}

#[test]
fn a_retraced_card_can_be_retraced_again_and_a_jump_started_one_cannot() -> Result<(), ScriptError>
{
    let mut driver = game(2, "Flame Jab", false, &["Island", "Island"], 0, 2)?;
    for life in [19, 18] {
        cast(&mut driver, "Flame Jab", Some("Island"))?;
        resolve(&mut driver)?;
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), life);
        find(driver.state(), Zone::Graveyard(pid(0)), "Flame Jab");
    }
    assert_eq!(count(driver.state(), Zone::Graveyard(pid(0)), "Island"), 2);
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert!(
        !castable(&driver, "Flame Jab"),
        "no land is left to discard"
    );

    let mut driver = game(2, "Radical Idea", false, &["Island", "Island"], 2, 0)?;
    cast(&mut driver, "Radical Idea", Some("Island"))?;
    resolve(&mut driver)?;
    find(driver.state(), Zone::Exile, "Radical Idea");
    assert!(
        !castable(&driver, "Radical Idea"),
        "the exiled card has no way to be cast"
    );
    Ok(())
}
