//! Goal: the Curses that trigger on an attack at the enchanted player play as the rules say at tables of two, three and four seats: the trigger of "whenever enchanted player is attacked" fires once however many creatures attack that player and not for an attack at another player or at a planeswalker of theirs, the Curse's controller gets the effect and so does each opponent of the controller with a creature attacking that player (an attacker who controls the Curse once), the Curses of Bounty, Disturbance, Opulence and Verbosity give what they say to those players only, "whenever a player attacks enchanted player with one or more creatures" fires once for the attacking player who may take the offer or decline it, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{bear, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, enchanting, ready, tapped};
use rstest::rstest;

const BEAR: &str = "Grizzly Bears";

/// Answers until the stack is empty and nothing resolves; a yes-or-no question gets `yes`.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 has `bears` Bears to attack with, `controller` has the Curse on the player `enchanted`, `extra` are more
/// permanents by seat, and every seat has cards to draw and `hand` cards in hand.
fn game(
    seats: usize,
    curse: &str,
    (controller, enchanted): (usize, usize),
    bears: usize,
    extra: Vec<(usize, PresetPermanent)>,
) -> Scenario {
    let mut permanents: Vec<Vec<PresetPermanent>> = vec![Vec::new(); seats];
    permanents[0].extend((0..bears).map(|_| ready(bear())));
    permanents[controller].push(enchanting(ready(card(curse).unwrap()), enchanted));
    for (seat, permanent) in extra {
        permanents[seat].push(permanent);
    }
    let mut scenario = Scenario::new(seats);
    for (seat, list) in permanents.into_iter().enumerate() {
        scenario = scenario
            .battlefield(seat, list)
            .library(seat, vec![bear(), bear(), bear(), bear()]);
    }
    scenario
}

/// Plays to the declaration of attackers and attacks with the Bears at these seats, then lets the triggers resolve.
fn attack(driver: &mut Driver, at: &[usize], yes: bool) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = at.iter().map(|seat| (BEAR, *seat)).collect();
    driver.attack(&attacks)?;
    settle(driver, yes)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// How many permanents of the name `seat` controls (tapped ones only when `tapped_only`).
fn permanents(driver: &Driver, seat: usize, name: &str, tapped_only: bool) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name == name)
        .filter(|object| !tapped_only || object.permanent().unwrap().tapped())
        .count()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Who gets the effect of "whenever enchanted player is attacked, ... each opponent attacking that player does the same",
/// by seat: (seats, (controller, enchanted), the seat each of seat 0's Bears attacks, who is affected).
#[rstest]
#[case::the_attacker_controls_the_curse(2, (0, 1), &[1], &[true, false])]
#[case::the_enchanted_player_controls_it(2, (1, 1), &[1], &[true, true])]
#[case::an_opponent_controls_it(2, (1, 0), &[1], &[false, false])]
#[case::two_creatures_trigger_it_once(3, (2, 1), &[1, 1], &[true, false, true])]
#[case::an_attack_at_another_player(3, (2, 1), &[2], &[false, false, false])]
#[case::an_attack_at_two_players(3, (2, 1), &[1, 2], &[true, false, true])]
#[case::four_seats(4, (3, 1), &[1, 2, 3], &[true, false, false, true])]
#[case::a_third_player_controls_it_and_is_not_attacked(4, (2, 1), &[1, 3], &[true, false, true, false])]
fn the_controller_and_the_attacking_opponents_get_the_effect(
    #[case] seats: usize,
    #[case] who: (usize, usize),
    #[case] at: &[usize],
    #[case] affected: &[bool],
) -> Result<(), ScriptError> {
    let scenario = || game(seats, "Curse of Vitality", who, at.len(), Vec::new());
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    attack(&mut driver, at, false)?;
    for (seat, gains) in affected.iter().enumerate() {
        let gained = life(&driver, seat) - before[seat];
        assert_eq!(gained, if *gains { 2 } else { 0 }, "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn an_attack_at_a_planeswalker_of_the_enchanted_player_does_not_trigger_it()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        3,
        "Curse of Vitality",
        (2, 1),
        1,
        vec![(1, ready(walker("Walker", 2)))],
    ))?;
    let before: Vec<i32> = (0..3).map(|seat| life(&driver, seat)).collect();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack_planeswalkers(&[(BEAR, "Walker")])?;
    settle(&mut driver, false)?;
    for (seat, was) in before.iter().enumerate() {
        assert_eq!(life(&driver, seat), *was, "seat {seat}");
    }
    Ok(())
}

/// What each of the Curses gives to who is affected: (Curse, a measure of it for a seat).
#[rstest]
#[case::cards("Curse of Verbosity", |d: &Driver, seat: usize| hand(d, seat))]
#[case::zombies("Curse of Disturbance", |d: &Driver, seat: usize| permanents(d, seat, "Zombie", false))]
#[case::gold("Curse of Opulence", |d: &Driver, seat: usize| permanents(d, seat, "Gold", false))]
fn the_controller_and_the_attacking_opponents_each_get_one(
    #[case] curse: &str,
    #[case] measure: fn(&Driver, usize) -> usize,
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] bears: usize,
) -> Result<(), ScriptError> {
    // Seat 0 attacks seat 1 with every Bear; seat 1 is enchanted and the last seat controls the Curse.
    let who = (seats - 1, 1);
    let scenario = || game(seats, curse, who, bears, Vec::new());
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before: Vec<usize> = (0..seats).map(|seat| measure(&driver, seat)).collect();
    attack(&mut driver, &vec![1; bears], false)?;
    for (seat, was) in before.iter().enumerate() {
        let got = measure(&driver, seat) - was;
        let expected = usize::from(seat == 0 || seat == seats - 1);
        assert_eq!(got, expected, "{curse}: seat {seat} of {seats}");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_curse_of_bounty_untaps_the_nonland_permanents_of_the_controller_and_the_attacking_opponents(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 0 attacks seat 1 and the last seat controls the Curse; each of them has a tapped Bear and a tapped land,
    // and seat 1 (enchanted, neither) has a tapped Bear as well.
    let last = seats - 1;
    let mut extra = Vec::new();
    for seat in [0, 1, last] {
        extra.push((seat, tapped(bear())));
        extra.push((seat, tapped(mtg_testkit::cards::forest())));
    }
    let mut driver = Driver::from_scenario(game(seats, "Curse of Bounty", (last, 1), 1, extra))?;
    attack(&mut driver, &[1], false)?;
    let tapped_bears = |seat: usize| permanents(&driver, seat, BEAR, true);
    assert_eq!(
        tapped_bears(0),
        0,
        "the attacker's Bears, the attacking one too"
    );
    assert_eq!(tapped_bears(last), 0, "the controller's Bear");
    assert_eq!(
        tapped_bears(1),
        1,
        "the enchanted player's Bear stays tapped"
    );
    for seat in [0, 1, last] {
        assert_eq!(
            permanents(&driver, seat, "Forest", true),
            1,
            "a land is not untapped"
        );
    }
    Ok(())
}

#[rstest]
fn the_curse_of_shallow_graves_offers_the_attacking_player_a_tapped_zombie(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] yes: bool,
) -> Result<(), ScriptError> {
    let who = (seats - 1, 1);
    let setup = game(seats, "Curse of Shallow Graves", who, 2, Vec::new())
        .setup()
        .unwrap();
    let mut driver =
        Driver::from_scenario(game(seats, "Curse of Shallow Graves", who, 2, Vec::new()))?;
    attack(&mut driver, &[1, 1], yes)?;
    let zombies: Vec<usize> = (0..seats)
        .map(|seat| permanents(&driver, seat, "Zombie", false))
        .collect();
    assert_eq!(zombies[0], usize::from(yes), "once for two attackers");
    assert_eq!(zombies.iter().skip(1).sum::<usize>(), 0, "no one else");
    assert_eq!(permanents(&driver, 0, "Zombie", true), usize::from(yes));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_curse_of_chaos_lets_the_attacking_player_discard_a_card_to_draw_one(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] yes: bool,
) -> Result<(), ScriptError> {
    let who = (seats - 1, 1);
    let scenario = || game(seats, "Curse of Chaos", who, 1, Vec::new()).hand(0, [bear()]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let graveyard = |driver: &Driver| driver.state().zone(Zone::Graveyard(pid(0))).len();
    let (cards, buried) = (hand(&driver, 0), graveyard(&driver));
    attack(&mut driver, &[1], yes)?;
    assert_eq!(
        hand(&driver, 0),
        cards,
        "a card out and a card in, or neither"
    );
    assert_eq!(graveyard(&driver), buried + usize::from(yes));
    for seat in 1..seats {
        assert_eq!(hand(&driver, seat), 0, "no one else");
    }
    replays(&driver, &setup);
    Ok(())
}
