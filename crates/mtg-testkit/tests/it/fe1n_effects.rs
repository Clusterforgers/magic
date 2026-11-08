//! Goal: the effects that end the game, set or swap life, clear damage, repeat while a condition holds and separate piles play through the engine at tables of two to four seats: a loss or a draw ends the game as the rules say, life changes are logged as gains and losses of the difference, a repeat stops at its bound whatever its condition says, and a pile is chosen by another player and bound to its name.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid, spell};
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameResult};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready};
use rstest::rstest;

fn instant(effects: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell("Trick", CardType::Instant, "[]", effects)
}

/// Casts the trick and lets the stack resolve, answering nothing.
fn play(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.cast("Trick")?;
    while !driver.state().stack().is_empty() && driver.state().result().is_none() {
        driver.pass()?;
    }
    Ok(())
}

fn life_events(driver: &Driver) -> Vec<Event> {
    driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::LifeGained { .. } | Event::LifeLost { .. }))
        .cloned()
        .collect()
}

#[rstest]
#[case::you(r#""You""#, &[0])]
#[case::opponents(r#""Opponents""#, &[1, 2, 3])]
#[case::each_player(r#""EachPlayer""#, &[0, 1, 2, 3])]
fn a_player_made_to_lose_leaves_the_game(
    #[case] who: &str,
    #[case] lost: &[usize],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = format!(r#"[{{"LoseGame":{{"who":{who}}}}}]"#);
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(0, [instant(&effects)]))?;
    play(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(
            driver.state().is_alive(pid(seat)),
            !lost.contains(&seat),
            "seat {seat}"
        );
    }
    let alive: Vec<usize> = (0..seats).filter(|s| !lost.contains(s)).collect();
    let want = match alive.as_slice() {
        [] => Some(GameResult::Draw),
        [only] => Some(GameResult::Winner(pid(*only))),
        _ => None,
    };
    assert_eq!(driver.state().result(), want);
    Ok(())
}

#[rstest]
fn a_drawn_game_ends_with_no_winner(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(
        0,
        [instant(r#"[{"DrawGame":{}},{"GainLife":{"amount":3}}]"#)],
    ))?;
    play(&mut driver)?;
    assert_eq!(driver.state().result(), Some(GameResult::Draw));
    assert!((0..seats).all(|s| !driver.state().is_alive(pid(s))));
    Ok(())
}

#[rstest]
#[case::up(20, 30, 10)]
#[case::down(20, 7, -13)]
#[case::same(20, 20, 0)]
#[case::to_nought(5, 0, -5)]
fn a_life_total_set_is_gained_or_lost_as_the_difference(
    #[case] start: i32,
    #[case] set: i32,
    #[case] change: i64,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = format!(r#"[{{"SetLife":{{"amount":{set}}}}}]"#);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .life(0, start)
            .hand(0, [instant(&effects)]),
    )?;
    play(&mut driver)?;
    let want = match change {
        0 => vec![],
        c if c > 0 => vec![Event::LifeGained {
            player: pid(0),
            amount: u32::try_from(c).unwrap(),
        }],
        c => vec![Event::LifeLost {
            player: pid(0),
            amount: u32::try_from(-c).unwrap(),
        }],
    };
    assert_eq!(life_events(&driver), want);
    if set > 0 {
        driver.expect_life(0, set)?;
    }
    Ok(())
}

#[rstest]
#[case::lower_for_you(10, 25)]
#[case::higher_for_you(30, 4)]
#[case::equal(20, 20)]
fn exchanged_life_totals_swap(
    #[case] mine: i32,
    #[case] theirs: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .life(0, mine)
            .life(1, theirs)
            .hand(0, [instant(r#"[{"ExchangeLife":{"with":"Opponents"}}]"#)]),
    )?;
    play(&mut driver)?;
    driver.expect_life(0, theirs)?;
    driver.expect_life(1, mine)?;
    let events = life_events(&driver);
    assert_eq!(events.len(), if mine == theirs { 0 } else { 2 });
    Ok(())
}

#[rstest]
#[case::damaged(3, true)]
#[case::undamaged(0, false)]
fn removed_damage_leaves_the_creature_whole(
    #[case] marked: u32,
    #[case] logged: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [damaged(free_creature("Subject", 2, 4), marked)])
            .hand(
                0,
                [instant(
                    r#"[{"RemoveDamage":{"what":{"Each":[{"Named":"Subject"}]}}}]"#,
                )],
            ),
    )?;
    play(&mut driver)?;
    let subject = driver.id_of("Subject")?;
    assert_eq!(
        driver
            .state()
            .object(subject)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    let removed = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::DamageRemoved { .. }))
        .count();
    assert_eq!(removed, usize::from(logged));
    Ok(())
}

const LOW_LIFE: &str = r#"{"Compare":{"left":{"Life":"You"},"op":"Lt","right":30}}"#;

#[rstest]
#[case::until_the_condition_fails(LOW_LIFE, 10, 20, 32)]
#[case::already_false(LOW_LIFE, 10, 40, 40)]
#[case::bounded_by_most(LOW_LIFE, 2, 20, 28)]
#[case::bounded_by_one(LOW_LIFE, 1, 20, 24)]
#[case::always_true_stops_at_the_bound(r#""YourTurn""#, 7, 20, 48)]
fn a_repeat_runs_while_its_condition_holds_and_at_most_its_bound(
    #[case] condition: &str,
    #[case] most: u32,
    #[case] start: i32,
    #[case] end: i32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = format!(
        r#"[{{"RepeatWhile":{{"condition":{condition},"effects":[{{"GainLife":{{"amount":4}}}}],"most":{most}}}}},{{"GainLife":{{"amount":1}}}}]"#
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .life(0, start)
            .hand(0, [instant(&effects)]),
    )?;
    play(&mut driver)?;
    // The effect after the repeat runs once, after the last pass.
    driver.expect_life(0, end + 1)
}

#[rstest]
fn a_repeat_inside_a_repeat_counts_its_passes_afresh(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = r#"[{"RepeatEach":{"over":"EachPlayer","as":"each","effects":[
        {"RepeatWhile":{"condition":"YourTurn","effects":[{"GainLife":{"who":{"Bound":"each"},"amount":1}}],"most":3}}]}}]"#;
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(0, [instant(effects)]))?;
    play(&mut driver)?;
    for seat in 0..seats {
        driver.expect_life(seat, 23)?;
    }
    Ok(())
}

const PILES: &str = r#"[
    {"SeparatePiles":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"chooser":"Opponents","as":"keep","rest_as":"lose"}},
    {"Destroy":{"what":{"Bound":"lose"}}}]"#;

/// Answers the piles: the first pile holds `first`, and the chooser takes pile `pick` (0 or 1).
fn separate(
    driver: &mut Driver,
    first: &[&str],
    pick: usize,
) -> Result<(usize, usize), ScriptError> {
    let mut seats = (usize::MAX, usize::MAX);
    driver.cast("Trick")?;
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseCards(_) => {
                seats.0 = decision.player.index();
                let ids = first
                    .iter()
                    .map(|n| driver.id_of(n))
                    .collect::<Result<_, _>>()?;
                driver.answer(Answer::ChooseCards(CardsChoice(ids)))?;
            }
            DecisionKind::ChooseOption(_) => {
                seats.1 = decision.player.index();
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            _ if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(seats)
}

#[rstest]
#[case::first_pile_kept(&["Ant", "Bat"], 0, &["Ant", "Bat"])]
#[case::second_pile_kept(&["Ant", "Bat"], 1, &["Cat"])]
#[case::empty_first_pile_kept(&[], 0, &[])]
#[case::empty_first_pile_second_taken(&[], 1, &["Ant", "Bat", "Cat"])]
#[case::everything_in_the_first_pile(&["Ant", "Bat", "Cat"], 0, &["Ant", "Bat", "Cat"])]
#[case::everything_in_the_first_pile_second_taken(&["Ant", "Bat", "Cat"], 1, &[])]
fn a_pile_is_separated_by_one_player_and_chosen_by_another(
    #[case] first: &[&str],
    #[case] pick: usize,
    #[case] kept: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(free_creature("Ant", 1, 1)),
                    ready(free_creature("Bat", 1, 1)),
                    ready(free_creature("Cat", 1, 1)),
                ],
            )
            .hand(0, [instant(PILES)]),
    )?;
    let (separator, chooser) = separate(&mut driver, first, pick)?;
    assert_eq!(separator, 0, "the controller separates");
    assert_eq!(chooser, 1, "the first opponent chooses");
    let mut left = names(driver.state(), Zone::Battlefield);
    left.sort();
    assert_eq!(
        left,
        kept.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    Ok(())
}

#[rstest]
#[case::three_times(3, 7, 20 + 12)]
#[case::never(0, 7, 20)]
#[case::most_is_the_lower_bound(9, 2, 20 + 8)]
fn a_repeat_with_times_stops_after_that_many_passes(
    #[case] times: u32,
    #[case] most: u32,
    #[case] end: i32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = format!(
        r#"[{{"RepeatWhile":{{"condition":"YourTurn","effects":[{{"GainLife":{{"amount":4}}}}],"most":{most},"times":{times}}}}},{{"GainLife":{{"amount":1}}}}]"#
    );
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(0, [instant(&effects)]))?;
    play(&mut driver)?;
    driver.expect_life(0, end + 1)
}
