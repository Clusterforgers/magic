//! Goal: the r47-01 casting rules play at two to four seats: a card cast "from your graveyard, but not from anywhere else"
//! is offered from its owner's graveyard and never from a hand (Haakon); a sorcery cast as though it had flash for an
//! extra part is offered on another player's turn only while that part can be paid (three untapped flyers, a Dragon to
//! behold); "each opponent can't cast instant or sorcery spells during that player's next turn" bans each opponent's
//! spells during that opponent's own next turn only (Azor); and a boast ability is used twice on its controller's turn
//! under "can boast twice" and once without it (Birgi).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Whether the deciding player is offered to cast the named card now.
fn r4701_castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.iter().any(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        }),
        _ => false,
    }
}

/// The player who decides now.
fn r4701_decider(driver: &Driver) -> PlayerId {
    driver.decision().unwrap().player
}

/// The other players pass priority on their turn until seat 0 decides.
fn r4701_to_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    while r4701_decider(driver) != PlayerId::from_index(0) {
        driver.pass()?;
    }
    Ok(())
}

/// Priority passes until the stack is empty.
fn r4701_settle(driver: &mut Driver) -> Result<(), ScriptError> {
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

#[rstest]
#[case::graveyard_two_seats(2, true)]
#[case::hand_two_seats(2, false)]
#[case::graveyard_four_seats(4, true)]
#[case::hand_three_seats(3, false)]
fn r4701_haakon_is_cast_only_from_the_graveyard(
    #[case] seats: usize,
    #[case] in_graveyard: bool,
) -> Result<(), ScriptError> {
    let haakon = card("Haakon, Stromgald Scourge").unwrap();
    let scenario =
        Scenario::new(seats).battlefield(0, (0..3).map(|_| ready(swamp())).collect::<Vec<_>>());
    let scenario = if in_graveyard {
        scenario.graveyard(0, [haakon])
    } else {
        scenario.hand(0, [haakon])
    };
    let driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    assert_eq!(
        r4701_castable(&driver, "Haakon, Stromgald Scourge"),
        in_graveyard
    );
    Ok(())
}

#[rstest]
#[case::three_flyers(2, 3, true)]
#[case::two_flyers(3, 2, false)]
#[case::four_flyers_four_seats(4, 4, true)]
fn r4701_tegwylls_scouring_has_flash_with_three_flyers(
    #[case] seats: usize,
    #[case] flyers: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut side: Vec<_> = (0..6).map(|_| ready(swamp())).collect();
    side.extend((0..flyers).map(|_| ready(card("Serra Angel").unwrap())));
    let scenario = Scenario::new(seats)
        .battlefield(0, side)
        .hand(0, [card("Tegwyll's Scouring").unwrap()])
        .at(1, 1, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    r4701_to_seat_zero(&mut driver)?;
    assert_eq!(r4701_castable(&driver, "Tegwyll's Scouring"), offered);
    Ok(())
}

#[rstest]
#[case::a_dragon_in_hand(2, true)]
#[case::no_dragon(2, false)]
#[case::a_dragon_at_three_seats(3, true)]
fn r4701_molten_exhale_has_flash_when_a_dragon_is_beheld(
    #[case] seats: usize,
    #[case] dragon: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![card("Molten Exhale").unwrap()];
    if dragon {
        hand.push(card("Archwing Dragon").unwrap());
    }
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(card("Grizzly Bears").unwrap())])
        .hand(0, hand)
        .at(1, 1, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    r4701_to_seat_zero(&mut driver)?;
    assert_eq!(r4701_castable(&driver, "Molten Exhale"), dragon);
    Ok(())
}

/// Seat 0 casts Azor; on each later turn of the round every opponent who holds a Lightning Bolt is offered it exactly
/// when the turn is not their first one since.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4701_azor_bans_each_opponent_during_their_own_next_turn(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let lands = vec![
        ready(plains()),
        ready(plains()),
        ready(island()),
        ready(island()),
        ready(plains()),
        ready(island()),
    ];
    let mut scenario = Scenario::new(seats)
        .battlefield(0, lands)
        .hand(0, [card("Azor, the Lawbringer").unwrap()]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(mountain())])
            .hand(seat, [card("Lightning Bolt").unwrap()]);
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    driver.cast("Azor, the Lawbringer")?;
    r4701_settle(&mut driver)?;
    r4701_settle(&mut driver)?;
    for turn in 2..=u32::try_from(seats).unwrap() {
        driver.idle_until(turn, Step::Main1)?;
        let active = usize::try_from(turn).unwrap() - 1;
        for step in 0..seats {
            let seat = (active + step) % seats;
            assert_eq!(r4701_decider(&driver), PlayerId::from_index(seat));
            if seat != 0 {
                // Banned only on the seat's own next turn; earlier seats had theirs, later seats wait for theirs.
                assert_eq!(
                    r4701_castable(&driver, "Lightning Bolt"),
                    seat != active,
                    "turn {turn}, seat {seat}"
                );
            }
            if step + 1 < seats {
                driver.pass()?;
            }
        }
    }
    Ok(())
}

/// Seat 0 attacks with Axgard Braggart and boasts as often as it may with four Plains.
#[rstest]
#[case::birgi_two_seats(2, true, 2)]
#[case::birgi_four_seats(4, true, 2)]
#[case::alone_three_seats(3, false, 1)]
fn r4701_birgi_lets_creatures_boast_twice(
    #[case] seats: usize,
    #[case] birgi: bool,
    #[case] boasts: usize,
) -> Result<(), ScriptError> {
    let mut side: Vec<_> = (0..4).map(|_| ready(plains())).collect();
    side.push(ready(card("Axgard Braggart").unwrap()));
    if birgi {
        side.push(ready(
            card("Birgi, God of Storytelling // Harnfel, Horn of Bounty").unwrap(),
        ));
    }
    let scenario = Scenario::new(seats)
        .battlefield(0, side)
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Axgard Braggart", 1)])?;
    let mut used = 0;
    while !driver.offered_abilities("Axgard Braggart")?.is_empty() {
        driver.begin_activation("Axgard Braggart")?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        r4701_settle(&mut driver)?;
        used += 1;
        assert!(used <= 2, "boasted a third time");
    }
    assert_eq!(used, boasts);
    Ok(())
}
