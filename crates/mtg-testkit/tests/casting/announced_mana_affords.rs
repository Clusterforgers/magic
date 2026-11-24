//! Goal: priority offers a spell that only a mana ability activated while paying makes affordable (Vexing Puzzlebox's roll, Skirge Familiar's discard, Jack-o'-Lantern's exile from the graveyard); the payment is asked with that ability offered and "Pay for me" off, activating it pays the spell, and a spell it cannot cover, or a discard with no other card to discard, stays unoffered (docs/engine.md), at two and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// A table where seat 0 holds `hand`, controls `battlefield` and has `graveyard` in its graveyard.
fn table(
    seats: usize,
    hand: &[&str],
    battlefield: &[&str],
    graveyard: &[&str],
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, hand.iter().copied())
            .battlefield_named(0, battlefield.iter().copied())
            .graveyard_named(0, graveyard.iter().copied()),
    )
}

/// The card of seat 0's hand named `name`.
fn in_hand(driver: &Driver, name: &str) -> ObjectId {
    let hand = driver.state().zone(Zone::Hand(PlayerId(0)));
    *hand
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == name)
        .unwrap()
}

fn castable(driver: &Driver, card: ObjectId) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&card),
        _ => false,
    }
}

/// Answers every question passively but the payment, until it is asked again or a player has priority.
fn to_payment(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    loop {
        match driver.decision()?.kind {
            DecisionKind::PayMana(_) | DecisionKind::Priority(_) => return Ok(log),
            _ => {
                let answer = Driver::passive_answer(driver.decision()?);
                log.extend(driver.answer(answer)?);
            }
        }
    }
}

#[rstest]
#[case::puzzlebox("Grizzly Bears", &[], &["Vexing Puzzlebox", "Forest"], &[], "Vexing Puzzlebox")]
#[case::skirge_familiar(
    "Grizzly Bears",
    &["Forest"],
    &["Skirge Familiar", "Forest"],
    &[],
    "Skirge Familiar"
)]
#[case::jack_o_lantern("Lightning Bolt", &[], &["Forest"], &["Jack-o'-Lantern"], "Jack-o'-Lantern")]
fn a_spell_only_the_announced_ability_affords_is_offered_and_paid(
    #[case] spell: &str,
    #[case] more_hand: &[&str],
    #[case] battlefield: &[&str],
    #[case] graveyard: &[&str],
    #[case] source: &str,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<&str> = [spell].into_iter().chain(more_hand.iter().copied()).collect();
    let mut driver = table(seats, &hand, battlefield, graveyard)?;
    let card = in_hand(&driver, spell);
    assert!(castable(&driver, card), "{spell} is offered");
    driver.begin_cast(spell)?;
    to_payment(&mut driver)?;
    let options = driver.pay_options()?;
    assert!(!options.auto_available, "the sources alone do not pay");
    let source_id = driver.id_of(source)?;
    let ability = *options
        .announced
        .iter()
        .find(|ability| ability.object == source_id)
        .expect("the ability is offered while paying");
    let mut log = driver.answer(Answer::PayMana(PayChoice::Activate(ability)))?;
    log.extend(to_payment(&mut driver)?);
    assert!(driver.pay_options()?.auto_available, "its mana pays the rest: {log:?}");
    let log = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(
        log.iter()
            .any(|e| matches!(e, Event::SpellCast { object, .. } if *object == card)
                || matches!(e, Event::SpellCast { .. })),
        "{log:?}"
    );
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().mana().total(), 0);
    Ok(())
}

#[rstest]
#[case::more_than_it_adds("Grizzly Bears", &[], &["Vexing Puzzlebox"])]
#[case::no_card_to_discard("Grizzly Bears", &[], &["Skirge Familiar", "Forest"])]
#[case::no_announced_ability("Grizzly Bears", &[], &["Forest"])]
fn a_spell_it_cannot_cover_stays_unoffered(
    #[case] spell: &str,
    #[case] more_hand: &[&str],
    #[case] battlefield: &[&str],
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<&str> = [spell].into_iter().chain(more_hand.iter().copied()).collect();
    let driver = table(seats, &hand, battlefield, &[])?;
    assert!(!castable(&driver, in_hand(&driver, spell)));
    Ok(())
}
