//! Goal: a mana ability the payment planner does not take (a discard cost, an untap cost) is activated at priority, resolves at once without a player receiving priority in between, adds its mana and leaves priority with the player who activated it, active or not, at two and at four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// Pays mana automatically and answers every other question of the activation passively until a player has priority again.
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

/// The mana `source` added in `log`.
fn added_by(log: &[Event], source: ObjectId) -> Vec<ManaType> {
    log.iter()
        .filter_map(|event| match event {
            Event::ManaAdded {
                mana,
                source: Some(from),
                ..
            } if *from == source => Some(*mana),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::two_seats_active(2, 0)]
#[case::two_seats_not_active(2, 1)]
#[case::four_seats_active(4, 0)]
#[case::four_seats_third_seat(4, 2)]
fn skirge_familiar_discards_adds_black_and_its_controller_keeps_priority(
    #[case] seats: usize,
    #[case] seat: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(seat, ["Grizzly Bears", "Forest"])
            .battlefield_named(seat, ["Skirge Familiar"]),
    )?;
    for _ in 0..seat {
        driver.pass()?;
    }
    let me = PlayerId(seat as u64);
    assert_eq!(driver.decision()?.player, me);
    let skirge = driver.id_of("Skirge Familiar")?;
    let mut log = driver.begin_activation("Skirge Familiar")?;
    log.extend(finish(&mut driver)?);
    assert_eq!(added_by(&log, skirge), [ManaType::Black], "{log:?}");
    assert!(
        log.iter()
            .any(|e| matches!(e, Event::CardDiscarded { player, .. } if *player == me))
    );
    let state = driver.state();
    assert!(state.stack().is_empty(), "the ability resolved at once");
    assert_eq!(state.zone(Zone::Hand(me)).len(), 1);
    assert_eq!(
        driver.decision()?.player,
        me,
        "the activator keeps priority"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn no_player_receives_priority_while_the_mana_ability_resolves(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Forest"])
            .battlefield_named(0, ["Skirge Familiar"]),
    )?;
    let skirge = driver.id_of("Skirge Familiar")?;
    let log = driver.begin_activation("Skirge Familiar")?;
    // The only card is discarded without a question, so the activation and the resolution are one answer.
    assert_eq!(added_by(&log, skirge), [ManaType::Black], "{log:?}");
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack().len(), 0);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn a_mana_ability_with_nothing_to_pay_its_cost_is_not_offered(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let driver =
        Driver::from_scenario(Scenario::new(seats).battlefield_named(0, ["Skirge Familiar"]))?;
    assert_eq!(driver.offered_abilities("Skirge Familiar")?.len(), 0);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn pili_pala_untaps_for_two_and_adds_a_mana_of_the_chosen_color(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let forest = || ready(card("Forest").unwrap());
    let pala = |tap: bool| {
        let card = card("Pili-Pala").unwrap();
        if tap { tapped(card) } else { ready(card) }
    };
    let untapped = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [pala(false), forest(), forest()]),
    )?;
    // Untapped, Pili-Pala cannot pay {Q}.
    assert_eq!(untapped.offered_abilities("Pili-Pala")?.len(), 0);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [pala(true), forest(), forest()]),
    )?;
    let pala = driver.id_of("Pili-Pala")?;
    let mut log = driver.begin_activation("Pili-Pala")?;
    log.extend(finish(&mut driver)?);
    assert!(
        log.iter()
            .any(|e| matches!(e, Event::PermanentUntapped { object } if *object == pala))
    );
    assert_eq!(added_by(&log, pala).len(), 1, "{log:?}");
    let begun = log
        .iter()
        .position(|e| matches!(e, Event::ResolutionBegan { .. }));
    let spent = log
        .iter()
        .position(|e| matches!(e, Event::ManaSpent { .. }));
    assert!(
        spent < begun,
        "the cost is paid before the ability resolves: {log:?}"
    );
    assert_eq!(driver.state().stack().len(), 0);
    assert_eq!(driver.decision()?.player, PlayerId(0));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn jack_o_lantern_exiles_itself_from_the_graveyard_for_a_mana(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .graveyard_named(0, ["Jack-o'-Lantern"])
            .battlefield_named(0, ["Forest"]),
    )?;
    let jack = driver.state().zone(Zone::Graveyard(PlayerId(0)))[0];
    let mut log = driver.begin_activation("Jack-o'-Lantern")?;
    log.extend(finish(&mut driver)?);
    assert!(log.iter().any(|e| matches!(
        e,
        Event::ZoneChanged { object, to: Zone::Exile, .. } if *object == jack
    )));
    assert_eq!(added_by(&log, jack).len(), 1, "{log:?}");
    assert_eq!(driver.state().stack().len(), 0);
    Ok(())
}
