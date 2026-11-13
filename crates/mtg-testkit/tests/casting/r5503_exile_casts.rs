//! Goal: casts from among exiled cards play at tables of two to four seats: Intrepid Paleontologist casts only Dinosaur
//! cards its owner owns that it exiled, each entering with a finality counter; Reenact the Crime targets only a card put
//! into a graveyard this turn and casts its copy free; Gix exiles X cards of an opponent for X discarded cards and casts
//! them free; Nashi lets its controller play one of the exiled top cards, a spell for life equal to its mana value.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, forest, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, pid};

/// A 3/3 Dinosaur creature card for {1}{G}.
fn r5503_dinosaur(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Dinosaur")
            .with_mana_cost(ManaCost::parse("{1}{G}").unwrap())
            .with_color_indicator(ColorSet::EMPTY.with(Color::Green))
            .with_power_toughness(3, 3),
    )
}

/// Answers until the stack is empty and a player has priority: a number with `x`, a yes/no with `yes`, a payment
/// automatically, a discard and a choice of cards with the first cards offered, the rest passively.
fn r5503_settle(driver: &mut Driver, x: u32, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..96 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => {
                return Ok(());
            }
            DecisionKind::ChooseNumber(_) => {
                Answer::ChooseNumber(mtg_engine::choose::NumberChoice::Number(x))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(yes)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Discard(options) => {
                Answer::Discard(options.hand.iter().copied().take(options.count).collect())
            }
            DecisionKind::ChooseTargets(options) => {
                Answer::ChooseTargets(mtg_engine::choose::TargetChoice::Targets(
                    options
                        .legal
                        .iter()
                        .copied()
                        .take(usize::try_from(options.min).unwrap())
                        .collect(),
                ))
            }
            DecisionKind::ChooseCards(options) => {
                Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                    options
                        .from
                        .iter()
                        .copied()
                        .take(usize::try_from(options.min).unwrap())
                        .collect(),
                ))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// The newest object of that name in any zone, as a target.
fn r5503_anywhere(driver: &Driver, name: &str) -> Target {
    let state = driver.state();
    let object = state
        .objects()
        .filter(|object| object.card().name == name)
        .max_by_key(|object| object.id())
        .unwrap();
    Target::Object(object.reference())
}

/// The names of the cards in a zone.
fn r5503_names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(zone)
        .iter()
        .map(|id| name_of(state, *id).clone())
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5503_paleontologist_casts_its_own_exiled_dinosaurs(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(6, forest);
    permanents.push(ready(card("Intrepid Paleontologist").unwrap()));
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .graveyard(0, vec![r5503_dinosaur("Mine")])
        .graveyard(1, vec![r5503_dinosaur("Theirs")]);
    let mut driver = Driver::from_scenario(scenario)?;
    for name in ["Mine", "Theirs"] {
        driver.begin_ability("Intrepid Paleontologist", 1)?;
        let target = r5503_anywhere(&driver, name);
        driver.choose_targets(&[target])?;
        r5503_settle(&mut driver, 0, true)?;
    }
    assert_eq!(r5503_names(&driver, Zone::Exile), vec!["Mine", "Theirs"]);
    // Only the Dinosaur its controller owns may be cast.
    assert!(driver.begin_cast("Theirs").is_err());
    driver.begin_cast("Mine")?;
    r5503_settle(&mut driver, 0, true)?;
    let mine = driver.id_of("Mine")?;

    let state = driver.state();
    assert_eq!(state.object(mine).unwrap().zone(), Zone::Battlefield);
    assert_eq!(
        state
            .object(mine)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&mtg_cards::value::CounterKind::Other("finality".into())),
        1
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5503_reenact_copies_only_a_card_put_there_this_turn(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(4, island);
    permanents.extend(lands(2, mountain));
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .battlefield(1, vec![ready(bear())])
        .graveyard(1, vec![burn("Old Bolt", 3)])
        .hand(
            0,
            vec![burn("Shock Bolt", 2), card("Reenact the Crime").unwrap()],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    // Nothing was put into a graveyard this turn: Reenact the Crime has no target.
    assert!(driver.begin_cast("Reenact the Crime").is_err());
    let bear_target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Shock Bolt", &[bear_target])?;
    r5503_settle(&mut driver, 0, true)?;
    assert!(r5503_names(&driver, Zone::Graveyard(pid(1))).contains(&"Grizzly Bears".to_owned()));
    driver.begin_cast("Reenact the Crime")?;
    let options = match &driver.decision()?.kind {
        DecisionKind::ChooseTargets(options) => options.clone(),
        other => panic!("expected targets, got {other:?}"),
    };
    let old = r5503_anywhere(&driver, "Old Bolt");
    assert!(!format!("{options:?}").contains(&format!("{old:?}")));
    let dead = r5503_anywhere(&driver, "Grizzly Bears");
    driver.choose_targets(&[dead])?;
    r5503_settle(&mut driver, 0, true)?;
    // The bear is exiled and its copy, cast free, is a token creature under seat 0's control.
    assert_eq!(r5503_names(&driver, Zone::Exile), vec!["Grizzly Bears"]);
    let state = driver.state();
    let copies: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == "Grizzly Bears")
        .map(|id| state.object(*id).unwrap().controller())
        .collect();
    assert_eq!(copies, vec![pid(0)]);
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats_two(3, 2)]
#[case::four_seats_three(4, 3)]
fn r5503_gix_exiles_x_and_casts_them_free(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut permanents = lands(7, swamp);
    permanents.push(ready(card("Gix, Yawgmoth Praetor").unwrap()));
    let hand: Vec<_> = (0..3).map(|_| forest()).collect();
    let library: Vec<_> = (0..4).map(|_| bear()).collect();
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, hand)
        .library(1, library);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Gix, Yawgmoth Praetor", 1)?;
    let mut answered_x = false;
    for _ in 0..8 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(
                    mtg_engine::choose::NumberChoice::Number(x),
                ))?;
                answered_x = true;
            }
            DecisionKind::ChooseTargets(_) => {
                let opponent = driver.target_player(1);
                driver.choose_targets(&[opponent])?;
            }
            _ => break,
        }
    }
    assert!(answered_x, "X is announced for the discarded cards");
    r5503_settle(&mut driver, x, true)?;
    let x = usize::try_from(x).unwrap();
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 3 - x);
    let bears = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Grizzly Bears")
        .count();
    assert_eq!(bears, x);
    assert_eq!(driver.state().zone(Zone::Library(pid(1))).len(), 4 - x);
    Ok(())
}

/// Nashi has hit seat 1 in combat: seat 0's top card is a Grizzly Bears, every other seat's an Island; it is seat 0's
/// postcombat main phase and the trigger has resolved.
fn r5503_after_nashi(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(card("Nashi, Moon Sage's Scion").unwrap())])
        .library(0, vec![bear(), bear()]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![island(), island()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Nashi, Moon Sage's Scion", 1)])?;
    driver.pass_until(Step::Main2)?;
    r5503_settle(&mut driver, 0, true)?;
    Ok(driver)
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5503_nashi_casts_one_exiled_card_for_life(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r5503_after_nashi(seats)?;
    assert_eq!(driver.state().zone(Zone::Exile).len(), seats);
    driver.begin_cast("Grizzly Bears")?;
    r5503_settle(&mut driver, 0, true)?;
    let state = driver.state();
    // The bear is paid for with 2 life, its mana value, not with mana.
    assert_eq!(state.player(pid(0)).unwrap().life(), 18);
    assert!(r5503_names(&driver, Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    // One of those cards only: no Island may be played now.
    assert!(driver.play_land("Island").is_err());
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5503_nashi_plays_a_land_and_then_nothing_else(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r5503_after_nashi(seats)?;
    driver.play_land("Island")?;
    assert!(driver.begin_cast("Grizzly Bears").is_err());
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(())
}
