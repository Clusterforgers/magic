//! Goal: a counterspell that then searches the countered spell's controller's graveyard, hand and library for cards of
//! its name plays as printed at 2 to 4 seats: Quash exiles every copy, Test of Talents exiles the copies its caster
//! picks and has that player draw one card for each taken from their hand, and Invasive Surgery searches only with
//! delirium; the searched player shuffles and nobody chooses which of their zones are searched.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, CostsChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, drain, instant, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Passes until the stack is empty, paying every optional cost and finding every card a search offers.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(!o.only_no)),
            DecisionKind::ChooseCosts(o) => Answer::ChooseCosts(CostsChoice::Costs(
                o.optional.iter().map(|cost| cost.index).collect(),
            )),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Four card types for seat 1's graveyard: an artifact, a creature, a land and an instant.
fn four_types() -> Vec<Arc<CardDefinition>> {
    vec![
        Arc::new(CardDefinition::new("Bauble").with_type(CardType::Artifact)),
        creature("Bear", 1, &[Color::Green], 2, 2),
        island(),
        instant("Opt", 0, &[Color::Blue]),
    ]
}

/// Seat 0 has `spell` twice in hand, once in its graveyard and twice in its library over two Mountains; seat 1 holds
/// `answer` with five Islands, and a graveyard of four card types when `delirium`.
fn table(seats: usize, spell: &Arc<CardDefinition>, answer: &str, delirium: bool) -> Scenario {
    let table = Scenario::new(seats)
        .hand(0, [spell.clone(), spell.clone()])
        .graveyard(0, [spell.clone()])
        .library(0, [spell.clone(), mountain(), spell.clone(), mountain()])
        .battlefield(0, [ready(mountain()), ready(swamp()), ready(swamp())])
        .hand(1, [card(answer).unwrap()])
        .battlefield(1, (0..5).map(|_| ready(island())));
    if delirium {
        table.graveyard(1, four_types())
    } else {
        table
    }
}

/// How many cards of the name each of seat 0's zones holds once seat 1 has answered the spell, and seat 0's hand size.
fn counted(
    seats: usize,
    spell: &Arc<CardDefinition>,
    answer: &str,
    delirium: bool,
) -> Result<([usize; 4], usize), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, spell, answer, delirium))?;
    driver.begin_cast(&spell.name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
            Target::Player(pid(1)),
        ])))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    driver.cast_at(answer, &[at])?;
    settle(&mut driver)?;
    let state = driver.state();
    let of = |zone: Zone| {
        names(state, zone)
            .iter()
            .filter(|n| **n == spell.name)
            .count()
    };
    let zones = [
        of(Zone::Graveyard(pid(0))),
        of(Zone::Hand(pid(0))),
        of(Zone::Library(pid(0))),
        of(Zone::Exile),
    ];
    Ok((zones, state.zone(Zone::Hand(pid(0))).len()))
}

#[rstest]
#[case::quash(2, "Quash", [0, 0, 0, 5], 0)]
#[case::quash_at_four(4, "Quash", [0, 0, 0, 5], 0)]
#[case::test_of_talents(3, "Test of Talents", [0, 0, 0, 5], 1)]
#[case::test_of_talents_at_four(4, "Test of Talents", [0, 0, 0, 5], 1)]
fn every_copy_of_the_countered_instant_is_exiled(
    #[case] seats: usize,
    #[case] answer: &str,
    #[case] zones: [usize; 4],
    #[case] hand: usize,
) -> Result<(), ScriptError> {
    let spell = burn("Shock", 2);
    assert_eq!(counted(seats, &spell, answer, false)?, (zones, hand));
    Ok(())
}

#[rstest]
#[case::with_delirium(2, true, [0, 0, 0, 5])]
#[case::with_delirium_at_three(3, true, [0, 0, 0, 5])]
#[case::without_delirium(4, false, [2, 1, 2, 0])]
fn invasive_surgery_searches_only_with_delirium(
    #[case] seats: usize,
    #[case] delirium: bool,
    #[case] zones: [usize; 4],
) -> Result<(), ScriptError> {
    let spell = drain("Rite");
    let (found, _) = counted(seats, &spell, "Invasive Surgery", delirium)?;
    assert_eq!(found, zones);
    Ok(())
}

#[test]
fn quash_may_not_answer_a_creature_spell() -> Result<(), ScriptError> {
    let bear = creature("Grizzly", 1, &[Color::Red], 2, 2);
    let mut driver = Driver::from_scenario(table(2, &bear, "Quash", false))?;
    driver.cast("Grizzly")?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    assert!(driver.cast_at("Quash", &[at]).is_err());
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn spellshift_lets_the_controller_cast_the_next_instant_free(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [burn("Shock", 2)])
            .library(0, [mountain(), burn("Zap", 3), mountain()])
            .battlefield(0, [ready(mountain())])
            .hand(1, [card("Spellshift").unwrap()])
            .battlefield(1, (0..4).map(|_| ready(island()))),
    )?;
    driver.begin_cast("Shock")?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
        Target::Player(pid(1)),
    ])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    driver.cast_at("Spellshift", &[at])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![Target::Player(pid(1))]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    let state = driver.state();
    assert_eq!(
        state.player(pid(1)).unwrap().life(),
        17,
        "Zap was cast for free, Shock was countered"
    );
    let graveyard = names(state, Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&"Shock".to_owned()) && graveyard.contains(&"Zap".to_owned()));
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 2);
    Ok(())
}

#[rstest]
#[case::with_evidence(2, true)]
#[case::with_evidence_at_four(4, true)]
#[case::without_evidence(3, false)]
fn deadly_cover_up_exiles_a_name_only_when_evidence_was_collected(
    #[case] seats: usize,
    #[case] evidence: bool,
) -> Result<(), ScriptError> {
    let shock = burn("Shock", 2);
    let mut table = Scenario::new(seats)
        .hand(0, [card("Deadly Cover-Up").unwrap()])
        .battlefield(
            0,
            (0..6).map(|_| ready(swamp())).chain([ready(creature(
                "Bear",
                1,
                &[Color::Green],
                2,
                2,
            ))]),
        )
        .hand(1, [shock.clone(), shock.clone()])
        .graveyard(1, [shock.clone()])
        .library(1, [shock.clone(), island(), island()]);
    if evidence {
        table = table.graveyard(0, [creature("Ogre", 5, &[Color::Black], 5, 5)]);
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.begin_cast("Deadly Cover-Up")?;
    settle(&mut driver)?;
    let state = driver.state();
    assert!(!names(state, Zone::Battlefield).contains(&"Bear".to_owned()));
    let shocks = |zone: Zone| names(state, zone).iter().filter(|n| *n == "Shock").count();
    let left = shocks(Zone::Graveyard(pid(1)))
        + shocks(Zone::Hand(pid(1)))
        + shocks(Zone::Library(pid(1)));
    assert_eq!(left, if evidence { 0 } else { 4 });
    let hand = state.zone(Zone::Hand(pid(1))).len();
    assert_eq!(
        hand, 2,
        "two Shocks taken from the hand are two cards drawn"
    );
    Ok(())
}
