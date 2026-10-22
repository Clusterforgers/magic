//! Goal: an `AnyOf` trigger of the card and of another object ("Whenever ~ or another creature you control dies or enters", as the compiler reads it) puts one ability on the stack for each event, for the card itself and for another creature and for none the selector leaves out, and not two when both events match the same object, at two, three and four seats; two creatures dying together are two events.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::ability::Ability;
use mtg_cards::compile::compile;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, free_creature, gain_life, mountain, twin_burn};
use mtg_testkit::trigger_cards::{creature_with, trigger, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The triggered ability the compiler reads from a Watcher card with this one line of rules.
fn read(text: &str) -> Ability {
    let face = PrintedFace {
        name: "Watcher".to_owned(),
        type_line: "Creature \u{2014} Elf".to_owned(),
        oracle_text: text.to_owned(),
        mana_cost: String::new(),
        power: Some("1".to_owned()),
        toughness: Some("1".to_owned()),
        ..PrintedFace::default()
    };
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789abc".to_owned(),
        name: "Watcher".to_owned(),
        faces: vec![face],
        ..PrintedCard::default()
    };
    let card = compile(&printed).card.expect("the card compiles");
    let ability = card.faces[0].abilities[0].clone();
    assert!(
        matches!(&ability, Ability::Triggered { when: TriggerPattern::AnyOf(events), .. } if events.len() == 2),
        "{text}: {ability:?}"
    );
    ability
}

/// What the Watcher waits for.
#[derive(Clone, Copy, Debug)]
enum Rules {
    /// "Whenever ~ or another creature you control dies, you gain 1 life." as the compiler reads it.
    DiesRead,
    /// The same with "enters".
    EntersRead,
    /// "Whenever ~ or a creature you control dies": both events match the Watcher itself.
    DiesOverlapping,
    /// "Whenever ~ or a creature you control enters": both events match the Watcher itself.
    EntersOverlapping,
}

fn overlapping(from: Option<ZoneKind>, to: ZoneKind) -> Ability {
    let mine = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    let event = |what| TriggerPattern::ZoneChange {
        what,
        from,
        to: Some(to),
    };
    trigger(
        when::either(event(Selector::this()), event(mine)),
        vec![gain_life(1)],
    )
}

impl Rules {
    fn ability(self) -> Ability {
        match self {
            Rules::DiesRead => {
                read("Whenever ~ or another creature you control dies, you gain 1 life.")
            }
            Rules::EntersRead => {
                read("Whenever ~ or another creature you control enters, you gain 1 life.")
            }
            Rules::DiesOverlapping => overlapping(Some(ZoneKind::Battlefield), ZoneKind::Graveyard),
            Rules::EntersOverlapping => overlapping(None, ZoneKind::Battlefield),
        }
    }

    /// Whether the events are creatures entering, so the creatures are cast and not burned.
    fn enters(self) -> bool {
        matches!(self, Rules::EntersRead | Rules::EntersOverlapping)
    }
}

/// A table of `seats` where seat 0 is active in its first main phase with three Mountains, a Bolt and a Twin Burn, and
/// the other seats each have a Foe. For events that kill, the Watcher and a Friend are on the battlefield of seat 0;
/// for events that enter, the Friend waits in hand and the Watcher is on the battlefield or in hand.
fn table(seats: usize, rules: Rules, watcher_in_hand: bool) -> Scenario {
    let watcher = creature_with("Watcher", vec![rules.ability()]);
    let friend = free_creature("Friend", 1, 1);
    let mut hand = vec![burn("Bolt", 3), twin_burn("Twin")];
    let mut board: Vec<_> = (0..3).map(|_| ready(mountain())).collect();
    if rules.enters() {
        hand.push(friend);
    } else {
        board.push(ready(friend));
    }
    if watcher_in_hand {
        hand.push(watcher);
    } else {
        board.push(ready(watcher));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, board);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("Foe {seat}"), 1, 1))]);
    }
    scenario
}

/// Passes until the stack is empty and returns every event of the way.
fn resolve_everything(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

fn life(state: &GameState, seat: usize) -> i32 {
    state.player(pid(seat)).unwrap().life()
}

/// The triggered abilities put on the stack in `events`.
fn triggers_put(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(put) if put.cause.is_some()))
        .count()
}

#[rstest]
#[case::the_watcher_dies(Rules::DiesRead, &["Watcher"], 1)]
#[case::a_friend_dies(Rules::DiesRead, &["Friend"], 1)]
#[case::a_foe_dies(Rules::DiesRead, &["Foe 1"], 0)]
#[case::both_together(Rules::DiesRead, &["Watcher", "Friend"], 2)]
#[case::the_friend_and_a_foe_together(Rules::DiesRead, &["Friend", "Foe 1"], 1)]
#[case::overlapping_watcher_dies(Rules::DiesOverlapping, &["Watcher"], 1)]
#[case::overlapping_friend_dies(Rules::DiesOverlapping, &["Friend"], 1)]
#[case::overlapping_foe_dies(Rules::DiesOverlapping, &["Foe 1"], 0)]
#[case::overlapping_both_together(Rules::DiesOverlapping, &["Watcher", "Friend"], 2)]
fn a_death_triggers_the_card_once_for_each_creature_that_dies(
    #[case] rules: Rules,
    #[case] victims: &[&str],
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, rules, false))?;
    let before = life(driver.state(), 0);
    let mut events = match victims {
        [only] => {
            let target = driver.target_named(only)?;
            driver.cast_at("Bolt", &[target])?
        }
        [first, second] => {
            let (first, second) = (driver.target_named(first)?, driver.target_named(second)?);
            driver.begin_cast("Twin")?;
            driver.choose_targets(&[first])?;
            driver.choose_targets(&[second])?;
            driver.answer(Answer::PayMana(PayChoice::Auto))?
        }
        _ => unreachable!("one or two victims"),
    };
    events.extend(resolve_everything(&mut driver)?);

    assert_eq!(
        triggers_put(&events),
        expected,
        "abilities put on the stack"
    );
    let gained = i32::try_from(expected).unwrap();
    assert_eq!(life(driver.state(), 0) - before, gained, "life gained");
    Ok(())
}

#[rstest]
#[case::the_watcher_enters(Rules::EntersRead, true, "Watcher")]
#[case::a_friend_enters(Rules::EntersRead, false, "Friend")]
#[case::overlapping_watcher_enters(Rules::EntersOverlapping, true, "Watcher")]
#[case::overlapping_friend_enters(Rules::EntersOverlapping, false, "Friend")]
fn an_entering_creature_triggers_the_card_once(
    #[case] rules: Rules,
    #[case] watcher_in_hand: bool,
    #[case] cast: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, rules, watcher_in_hand))?;
    let before = life(driver.state(), 0);
    let mut events = driver.cast(cast)?;
    events.extend(resolve_everything(&mut driver)?);

    assert_eq!(triggers_put(&events), 1, "abilities put on the stack");
    assert_eq!(life(driver.state(), 0) - before, 1, "life gained");
    Ok(())
}
