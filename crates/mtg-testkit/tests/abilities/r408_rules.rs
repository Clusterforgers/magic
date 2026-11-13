//! Goal: the r40-8 rules play as their cards say at two to four seats: Experimental Frenzy keeps lands and spells in the
//! hand while the top of the library stays open, Cornered Market keeps a nonbasic land with a nontoken permanent's name
//! in the hand but not a basic one, Discontinuity costs {1}{U} during its caster's turn and ends the turn, and Intervention
//! Pact prevents the next damage of the chosen source, gains that much life and is paid for at the next upkeep or the
//! game is lost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityOptions};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {}", other.name()),
    }
}

fn libraries(mut scenario: Scenario, seats: usize, top: fn() -> std::sync::Arc<mtg_cards::CardDefinition>) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| top()));
    }
    scenario
}

#[rstest]
#[case::two_seats(2, true)]
#[case::three_seats(3, true)]
#[case::four_seats(4, true)]
#[case::without_the_frenzy(2, false)]
fn experimental_frenzy_closes_the_hand_and_opens_the_library(
    #[case] seats: usize,
    #[case] frenzy: bool,
) {
    let mut board = vec![ready(mountain())];
    if frenzy {
        board.push(ready(card("Experimental Frenzy").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, [forest(), burn("Shock", 2)]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats, mountain)).unwrap();
    let offered = options(&driver);
    let hand = driver.state().zone(Zone::Hand(pid(0))).to_vec();
    let top = *driver.state().zone(Zone::Library(pid(0))).last().unwrap();
    assert_eq!(
        offered.playable_lands.iter().any(|id| hand.contains(id)),
        !frenzy,
        "the land in hand"
    );
    assert_eq!(offered.playable_lands.contains(&top), frenzy, "the top land");
    assert_eq!(offered.castable.iter().any(|id| hand.contains(id)), !frenzy, "the spell in hand");
    if frenzy {
        driver.play_land("Mountain").unwrap();
        assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 2);
        assert!(driver.state().zone(Zone::Battlefield).contains(&top));
    }
}

#[rstest]
#[case::two_seats_a_named_land(2, true)]
#[case::three_seats_a_named_land(3, true)]
#[case::four_seats_a_named_land(4, true)]
#[case::four_seats_no_such_permanent(4, false)]
fn cornered_market_keeps_a_named_nonbasic_land_in_hand(#[case] seats: usize, #[case] named: bool) {
    let mut board = vec![
        ready(card("Cornered Market").unwrap()),
        ready(mountain()),
    ];
    if named {
        board.push(ready(card("Command Tower").unwrap()));
    }
    // The permanent may be another player's.
    let (mine, theirs) = if named && seats > 2 {
        (board[..2].to_vec(), board[2..].to_vec())
    } else {
        (board, Vec::new())
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(seats - 1, theirs)
        .hand(0, [card("Command Tower").unwrap(), mountain()]);
    let driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    let offered: Vec<String> = options(&driver)
        .playable_lands
        .iter()
        .map(|id| crate::common::name_of(driver.state(), *id))
        .collect();
    assert!(offered.contains(&"Mountain".to_owned()), "a basic land is always open");
    assert_eq!(offered.contains(&"Command Tower".to_owned()), !named);
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn discontinuity_costs_two_on_your_turn_and_ends_it(#[case] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(island()), ready(island())])
        .hand(0, [card("Discontinuity").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    driver.cast("Discontinuity").unwrap();
    for _ in 0..40 {
        if driver.state().turn().number() > 2 {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(names(driver.state(), Zone::Exile), ["Discontinuity"]);
    assert_eq!(driver.state().turn().number(), 3);
    assert_eq!(driver.state().active_player(), Some(pid(1)));
}

#[test]
fn discontinuity_costs_its_full_price_on_another_turn() {
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .battlefield(1, (0..5).map(|_| ready(island())))
        .hand(1, [card("Discontinuity").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, 2, forest)).unwrap();
    driver.pass().unwrap();
    let offered = options(&driver);
    let hand = driver.state().zone(Zone::Hand(pid(1))).to_vec();
    assert!(!offered.castable.iter().any(|id| hand.contains(id)));
}

/// Casts Intervention Pact while Shock aimed at seat 0 waits on the stack, choosing Shock as the source.
fn pact_against_shock(seats: usize, lands: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, (0..lands).map(|_| ready(plains())).chain([ready(mountain()), ready(mountain())]))
        .hand(0, [burn("Shock", 2), burn("Shock", 2), card("Intervention Pact").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    let me = driver.target_player(0);
    driver.begin_cast("Shock").unwrap();
    driver.choose_targets(&[me]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    driver.cast("Intervention Pact").unwrap();
    for _ in 0..20 {
        if driver.state().stack().len() <= 1 {
            break;
        }
        match &driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let shock = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| crate::common::name_of(driver.state(), *id) == "Shock")
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![shock]))).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    driver
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn intervention_pact_prevents_the_next_damage_and_gains_that_much(#[case] seats: usize) {
    let mut driver = pact_against_shock(seats, 0);
    let life = |d: &Driver| d.state().player(pid(0)).unwrap().life();
    while !driver.state().stack().is_empty() {
        driver.answer_passively().unwrap();
    }
    assert_eq!(life(&driver), 22, "Shock prevented, 2 life gained");
    // The shield is spent: the second Shock is not the chosen source and deals its damage.
    let me = driver.target_player(0);
    driver.begin_cast("Shock").unwrap();
    driver.choose_targets(&[me]).unwrap();
    let _ = driver.answer(Answer::PayMana(PayChoice::Auto));
    while !driver.state().stack().is_empty() {
        driver.answer_passively().unwrap();
    }
    assert!(life(&driver) <= 22);
}

#[rstest]
#[case::paid_two_seats(2, true)]
#[case::paid_four_seats(4, true)]
#[case::unpaid_three_seats(3, false)]
fn intervention_pact_is_paid_at_the_next_upkeep_or_the_game_is_lost(
    #[case] seats: usize,
    #[case] pays: bool,
) {
    let mut driver = pact_against_shock(seats, 3);
    for _ in 0..400 {
        if !driver.state().is_alive(pid(0)) || driver.state().turn().number() > u32::try_from(seats).unwrap() + 2 {
            break;
        }
        match &driver.decision().unwrap().kind.clone() {
            DecisionKind::YesNo(_) if driver.decision().unwrap().player == pid(0) => {
                driver.answer(Answer::YesNo(YesNoChoice(pays))).unwrap();
            }
            DecisionKind::PayMana(_) if pays => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(driver.state().is_alive(pid(0)), pays);
}

#[rstest]
#[case::paid_at_two_seats(2, 15, true)]
#[case::paid_at_three_seats(3, 20, true)]
#[case::declined_at_four_seats(4, 15, false)]
fn garzas_assassin_recovers_for_half_the_life_rounded_up(
    #[case] seats: usize,
    #[case] life: i32,
    #[case] pays: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .life(0, life)
        .battlefield(0, [ready(mountain()), ready(mtg_testkit::cards::bear())])
        .graveyard(0, [card("Garza's Assassin").unwrap()])
        .hand(0, [burn("Shock", 2)]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    let bear = driver.target_named("Grizzly Bears").unwrap();
    driver.begin_cast("Shock").unwrap();
    driver.choose_targets(&[bear]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    for _ in 0..30 {
        if driver.state().stack().is_empty()
            && !names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Garza's Assassin".to_owned())
        {
            break;
        }
        match &driver.decision().unwrap().kind.clone() {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(pays))).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let now = driver.state().player(pid(0)).unwrap().life();
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    if pays {
        assert_eq!(now, life - (life + 1) / 2, "half the life, rounded up");
        assert!(hand.contains(&"Garza's Assassin".to_owned()));
    } else {
        assert_eq!(now, life);
        assert!(names(driver.state(), Zone::Exile).contains(&"Garza's Assassin".to_owned()));
    }
}

#[rstest]
#[case::two_seats(2, 2)]
#[case::three_seats(3, 3)]
#[case::four_seats(4, 2)]
fn sekki_turns_damage_into_spirits(#[case] seats: usize, #[case] amount: i32) {
    use mtg_cards::value::CounterKind;
    let sekki = mtg_testkit::scenario::with_counters(
        ready(card("Sekki, Seasons' Guide").unwrap()),
        CounterKind::PlusOnePlusOne,
        8,
    );
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [sekki, ready(mountain())])
        .hand(0, [burn("Blast", amount)]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    let at = driver.target_named("Sekki, Seasons' Guide").unwrap();
    driver.begin_cast("Blast").unwrap();
    driver.choose_targets(&[at]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.answer_passively().unwrap();
    }
    let id = driver.id_of("Sekki, Seasons' Guide").unwrap();
    let permanent = driver.state().object(id).unwrap().permanent().unwrap();
    let amount = u32::try_from(amount).unwrap();
    assert_eq!(permanent.damage(), 0, "the damage is prevented");
    assert_eq!(
        permanent.counters_of(&CounterKind::PlusOnePlusOne),
        8 - amount
    );
    let board = names(driver.state(), Zone::Battlefield);
    assert_eq!(
        board.iter().filter(|name| *name == "Spirit").count(),
        usize::try_from(amount).unwrap()
    );
}

#[rstest]
#[case::mine_with_one_island(2, Some(0), 1, true)]
#[case::none_with_one_island(3, None, 1, false)]
#[case::theirs_with_three_islands(4, Some(1), 3, false)]
#[case::none_with_three_islands(2, None, 3, true)]
fn catalyst_stone_changes_flashback_costs(
    #[case] seats: usize,
    #[case] stone: Option<usize>,
    #[case] islands: usize,
    #[case] castable: bool,
) {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, (0..islands).map(|_| ready(island())))
        .graveyard(0, [card("Flash of Insight").unwrap()]);
    if let Some(seat) = stone {
        let mut board: Vec<_> = if seat == 0 {
            (0..islands).map(|_| ready(island())).collect()
        } else {
            Vec::new()
        };
        board.push(ready(card("Catalyst Stone").unwrap()));
        scenario = scenario.battlefield(seat, board);
    }
    let driver = Driver::from_scenario(libraries(scenario, seats, forest)).unwrap();
    let flash = *driver.state().zone(Zone::Graveyard(pid(0))).first().unwrap();
    assert_eq!(options(&driver).castable.contains(&flash), castable);
}
