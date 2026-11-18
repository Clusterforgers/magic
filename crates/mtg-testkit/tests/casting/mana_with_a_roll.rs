//! Goal: a mana ability that does more than add mana and asks for what the payment cannot run (Vexing Puzzlebox's "{T}: Add one mana of any color. Roll a d20.") is offered at priority, resolves at once with its mana and its roll, its "whenever you roll" trigger goes on the stack afterwards and puts that many charge counters on it, and its floating mana pays a later cast; the payment never counts or taps it on its own; while paying for a spell it is offered too, resolves at once and the payment is asked again with its mana, its trigger waiting until the cast is done, and automatic payment never activates it (docs/engine.md), at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Answers every question of the activation passively (the colour) until a player has priority again.
fn finish(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        let answer = match driver.decision()?.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(driver.decision()?),
        };
        log.extend(driver.answer(answer)?);
    }
    Ok(log)
}

/// The die `log` rolled for `player`.
fn rolled(log: &[Event], me: PlayerId) -> Option<u32> {
    log.iter().find_map(|event| match event {
        Event::DieRolled {
            player,
            sides: 20,
            result,
            ..
        } if *player == me => Some(*result),
        _ => None,
    })
}

fn charge_on(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::Charge))
}

/// Activates the Puzzlebox's mana ability at priority as `me` and checks the mana and the roll.
fn tap_puzzlebox(driver: &mut Driver, me: PlayerId) -> Result<(ObjectId, u32), ScriptError> {
    let puzzlebox = driver.id_of("Vexing Puzzlebox")?;
    assert_eq!(driver.decision()?.player, me);
    assert!(
        driver.offered_abilities("Vexing Puzzlebox")?.contains(&1),
        "the mana ability is offered at priority"
    );
    let mut log = driver.begin_ability("Vexing Puzzlebox", 1)?;
    log.extend(finish(driver)?);
    let added = log
        .iter()
        .filter(|e| matches!(e, Event::ManaAdded { source: Some(s), .. } if *s == puzzlebox))
        .count();
    assert_eq!(added, 1, "{log:?}");
    let roll = rolled(&log, me).expect("a d20 is rolled");
    assert!((1..=20).contains(&roll));
    assert_eq!(
        driver.decision()?.player,
        me,
        "the activator keeps priority"
    );
    assert_eq!(driver.state().player(me).unwrap().mana().total(), 1);
    Ok((puzzlebox, roll))
}

#[rstest]
#[case::two_seats(2, 0)]
#[case::two_seats_not_active(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 0)]
#[case::four_seats_third_seat(4, 2)]
fn puzzlebox_adds_mana_rolls_and_its_trigger_puts_charge_counters(
    #[case] seats: usize,
    #[case] seat: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield_named(seat, ["Vexing Puzzlebox"]))?;
    for _ in 0..seat {
        driver.pass()?;
    }
    let me = PlayerId(seat as u64);
    let (puzzlebox, roll) = tap_puzzlebox(&mut driver, me)?;
    // The roll's trigger waits on the stack, above nothing, until the players pass.
    assert_eq!(
        driver.state().stack().len(),
        1,
        "the trigger is on the stack"
    );
    assert_eq!(charge_on(&driver, puzzlebox), 0);
    for _ in 0..seats {
        driver.pass()?;
        if driver.state().stack().is_empty() {
            break;
        }
    }
    assert_eq!(charge_on(&driver, puzzlebox), roll);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn puzzlebox_mana_tapped_at_priority_pays_a_cast(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Vexing Puzzlebox", "Forest"]),
    )?;
    let me = PlayerId(0);
    let puzzlebox = driver.id_of("Vexing Puzzlebox")?;
    // The payment counts no source with a roll, so the Forest alone cannot cast the creature: the player taps the
    // Puzzlebox at priority first and its mana floats.
    let bears = driver.state().zone(Zone::Hand(me))[0];
    let castable = |driver: &Driver| match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&bears),
        _ => false,
    };
    assert!(!castable(&driver));
    let (_, roll) = tap_puzzlebox(&mut driver, me)?;
    // The trigger resolves first; the mana floats until the step ends.
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(charge_on(&driver, puzzlebox), roll);
    assert_eq!(driver.state().player(me).unwrap().mana().total(), 1);
    assert!(castable(&driver), "the floating mana counts for the cast");
    driver.begin_cast("Grizzly Bears")?;
    let log = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(
        log.iter().any(|e| matches!(e, Event::SpellCast { .. })),
        "{log:?}"
    );
    assert_eq!(
        driver.state().player(me).unwrap().mana().total(),
        0,
        "the floating mana paid"
    );
    Ok(())
}

fn tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

/// The active seat holds Grizzly Bears with the Puzzlebox and two Forests, and begins to cast it.
fn paying(seats: usize) -> Result<(Driver, PlayerId), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Vexing Puzzlebox", "Forest", "Forest"]),
    )?;
    driver.begin_cast("Grizzly Bears")?;
    Ok((driver, PlayerId(0)))
}

/// Answers the questions of the activation passively (the colour) until the payment is asked again.
fn back_to_payment(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    while !matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        let answer = Driver::passive_answer(driver.decision()?);
        log.extend(driver.answer(answer)?);
    }
    Ok(log)
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn puzzlebox_is_activated_from_the_payment_and_the_payment_is_asked_again(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, me) = paying(seats)?;
    let puzzlebox = driver.id_of("Vexing Puzzlebox")?;
    let offered = driver.pay_options()?.announced.clone();
    assert_eq!(offered.len(), 1, "the mana ability is offered while paying");
    let ability = offered[0];
    assert_eq!(ability.object, puzzlebox);
    let mut log = driver.answer(Answer::PayMana(PayChoice::Activate(ability)))?;
    log.extend(back_to_payment(&mut driver)?);
    assert!(rolled(&log, me).is_some(), "a d20 is rolled: {log:?}");
    assert!(tapped(&driver, puzzlebox));
    let options = driver.pay_options()?;
    assert_eq!(options.pool.total(), 1, "its mana floats into the payment");
    assert!(
        options.announced.is_empty(),
        "a tapped Puzzlebox is offered no more"
    );
    assert!(
        driver.state().stack().is_empty(),
        "the roll's trigger waits until the cast is done"
    );
    assert!(
        driver
            .answer(Answer::PayMana(PayChoice::Activate(ability)))
            .is_err(),
        "an ability not offered is refused"
    );
    let log = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(
        log.iter().any(|e| matches!(e, Event::SpellCast { .. })),
        "{log:?}"
    );
    assert_eq!(driver.state().player(me).unwrap().mana().total(), 0);
    assert_eq!(
        driver.state().stack().len(),
        2,
        "the Bears and, above them, the roll's trigger"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn automatic_payment_never_activates_it(#[case] seats: usize) -> Result<(), ScriptError> {
    let (mut driver, _) = paying(seats)?;
    let puzzlebox = driver.id_of("Vexing Puzzlebox")?;
    let log = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(log.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    assert!(!tapped(&driver, puzzlebox));
    assert!(!log.iter().any(|e| matches!(e, Event::DieRolled { .. })));
    Ok(())
}

#[test]
fn backing_out_after_it_leaves_its_mana_floating() -> Result<(), ScriptError> {
    let (mut driver, me) = paying(2)?;
    let ability = driver.pay_options()?.announced[0];
    driver.answer(Answer::PayMana(PayChoice::Activate(ability)))?;
    back_to_payment(&mut driver)?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(
        driver.state().zone(Zone::Hand(me)).len(),
        1,
        "the Bears went back to the hand"
    );
    assert_eq!(driver.state().player(me).unwrap().mana().total(), 1);
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    Ok(())
}
