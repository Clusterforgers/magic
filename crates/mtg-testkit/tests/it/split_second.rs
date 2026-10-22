//! Goal: split second plays as the rules say at two to four seats. While a spell with it is on the stack no seat is offered a spell or an ability that is not a mana ability (and the engine refuses them), mana abilities stay, triggered abilities still trigger and resolve above it, the spell resolves as any other, and the players act again once it has gone; a spell without it leaves every seat its options.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction, PriorityOptions};
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds the spell `cast` and a Lightning Bolt with four Mountains, a Mother of Runes, a Sol Ring and a Monastery
/// Swiftspear in play; each other seat holds a Lightning Bolt with two Mountains, a Mother of Runes and a Sol Ring.
fn table(seats: usize, cast: &str, prowess: bool) -> Driver {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(cast).unwrap(), card("Lightning Bolt").unwrap()])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(mountain()))
                .chain([
                    ready(card("Mother of Runes").unwrap()),
                    ready(card("Sol Ring").unwrap()),
                ])
                .chain(prowess.then(|| ready(card("Monastery Swiftspear").unwrap())))
                .collect::<Vec<_>>(),
        );
    for seat in 1..seats {
        scenario = scenario
            .hand(seat, [card("Lightning Bolt").unwrap()])
            .battlefield(
                seat,
                (0..2)
                    .map(|_| ready(mountain()))
                    .chain([
                        ready(card("Mother of Runes").unwrap()),
                        ready(card("Sol Ring").unwrap()),
                    ])
                    .collect::<Vec<_>>(),
            );
    }
    Driver::from_scenario(scenario).unwrap()
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority was expected, not {}", other.name()),
    }
}

/// Seat 0 casts `spell` at seat 1 (or at a seat-1 creature-less table: the player), paying with two Mountains.
fn cast(driver: &mut Driver, spell: &str, lands: usize) {
    driver.begin_cast(spell).unwrap();
    let target = driver.target_player(1);
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"].repeat(lands), 0).unwrap();
}

#[rstest]
#[case::split_second("Sudden Shock", true)]
#[case::an_ordinary_instant("Lightning Bolt", false)]
fn the_seats_may_cast_and_activate_only_without_split_second_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] silenced: bool,
) {
    let mut driver = table(seats, spell, false);
    let lands = if spell == "Sudden Shock" { 2 } else { 1 };
    // The seat holding priority has a spell and an ability to use before anything is cast.
    let before = options(&driver);
    assert!(!before.castable.is_empty() && !before.abilities.is_empty());
    cast(&mut driver, spell, lands);
    assert_eq!(driver.state().stack().len(), 1);
    for seat in 0..seats {
        let decision = driver.decision().unwrap();
        assert_eq!(decision.player, pid(seat));
        let now = options(&driver);
        assert_eq!(now.castable.is_empty(), silenced, "seat {seat}");
        assert_eq!(now.abilities.is_empty(), silenced, "seat {seat}");
        assert_eq!(now.can_act, !silenced, "seat {seat}");
        // Mana abilities and passing stay.
        assert!(!now.mana_abilities.is_empty(), "seat {seat}");
        assert!(now.can_pass);
        driver.pass().unwrap();
    }
    // The spell resolved: its target lost life and the stack is empty.
    assert_eq!(driver.state().stack(), []);
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert!(life(1) < 20, "the spell resolved");
    // Seat 0 acts again.
    assert_eq!(driver.decision().unwrap().player, pid(0));
    let after = options(&driver);
    assert!(!after.castable.is_empty() && !after.abilities.is_empty());
}

#[rstest]
fn the_engine_refuses_what_the_options_leave_out(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, "Sudden Shock", false);
    // Seat 0 holds a Lightning Bolt and a Mother of Runes it could use.
    let bolt = options(&driver)
        .castable
        .into_iter()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Lightning Bolt")
        })
        .unwrap();
    cast(&mut driver, "Sudden Shock", 2);
    assert!(
        driver
            .answer(Answer::Priority(PriorityAction::CastSpell(bolt)))
            .is_err()
    );
    assert!(driver.begin_cast("Lightning Bolt").is_err());
    assert!(driver.begin_activation("Mother of Runes").is_err());
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(driver.decision().unwrap().player, pid(0));
}

#[rstest]
fn a_mana_ability_can_still_be_used_but_nothing_is_cast_with_the_mana(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = table(seats, "Sudden Shock", false);
    cast(&mut driver, "Sudden Shock", 2);
    driver.pass().unwrap();
    // Seat 1 taps a Sol Ring for mana: it floats, and the Lightning Bolt stays out of reach.
    assert_eq!(driver.decision().unwrap().player, pid(1));
    driver.tap_for_mana("Sol Ring").unwrap();
    assert_eq!(driver.state().player(pid(1)).unwrap().mana().total(), 2);
    assert_eq!(driver.decision().unwrap().player, pid(1));
    assert_eq!(options(&driver).castable, []);
    assert!(!options(&driver).can_act);
}

#[rstest]
fn a_triggered_ability_still_triggers_and_resolves_above_the_spell(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = table(seats, "Sudden Shock", true);
    let swiftspear = driver.id_of("Monastery Swiftspear").unwrap();
    let power = |driver: &Driver| {
        mtg_engine::characteristics::characteristics(driver.state(), swiftspear)
            .unwrap()
            .power
    };
    assert_eq!(power(&driver), Some(1));
    cast(&mut driver, "Sudden Shock", 2);
    // Prowess triggered on the cast and waits above the spell.
    assert_eq!(driver.state().stack().len(), 2);
    for _ in 0..seats {
        assert_eq!(options(&driver).castable, []);
        driver.pass().unwrap();
    }
    assert_eq!(driver.state().stack().len(), 1, "the trigger resolved");
    assert_eq!(power(&driver), Some(2));
    // The spell is still on the stack and still holds everyone back.
    assert_eq!(options(&driver).castable, []);
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    assert_eq!(driver.state().stack(), []);
    assert_eq!(
        options(&driver).castable.len(),
        1,
        "the Lightning Bolt is castable again"
    );
}
