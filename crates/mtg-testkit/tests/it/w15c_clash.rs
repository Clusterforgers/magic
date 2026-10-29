//! Goal: the clash leftovers play whole at tables of two, three and four seats: Marvo, Deep Operative clashes with the player it attacks (no opponent is asked for) and on a win draws and may cast a spell of mana value 8 or less from the hand for free; Scattering Stroke counters its target and on a win may add as much {C} as the countered spell's mana value at the beginning of the caster's next main phase; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A vanilla creature card of mana value `value`.
fn w15c_costing(value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Cost {value}"))
            .with_type(CardType::Creature)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(1, 1),
    )
}

/// A table at seat 0's first main phase of turn 2: seat 0's library is eight cards of mana value `mine` and every other
/// library eight of mana value `theirs`.
fn w15c_table(seats: usize, mine: u32, theirs: u32) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .library(0, (0..8).map(|_| w15c_costing(mine)));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| w15c_costing(theirs)));
    }
    scenario
}

/// Answers until the stack is empty and seat 0 has priority: pays automatically, keeps each clashed card on top, says
/// `yes` to any other question and picks the first card offered; the opponents asked to be chosen and the clashes seen.
fn w15c_resolve(driver: &mut Driver, yes: bool) -> Result<(usize, Vec<Event>), ScriptError> {
    let (mut chosen, mut clashes) = (0, Vec::new());
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        let events = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => {
                return Ok((chosen, clashes));
            }
            DecisionKind::PayMana(_) => driver.answer(Answer::PayMana(PayChoice::Auto))?,
            DecisionKind::YesNo(options) => {
                let keep = options.text.starts_with("Clash");
                driver.answer(Answer::YesNo(YesNoChoice(yes && !keep)))?
            }
            DecisionKind::ChooseCards(options) => {
                driver.answer(Answer::ChooseCards(CardsChoice(options.from.iter().copied().take(1).collect())))?
            }
            DecisionKind::ChooseOption(_) => {
                chosen += 1;
                driver.answer(Driver::passive_answer(&decision))?
            }
            _ => driver.answer(Driver::passive_answer(&decision))?,
        };
        clashes.extend(events.into_iter().filter(|event| matches!(event, Event::Clashed { .. })));
    }
    panic!("the stack did not empty");
}

fn w15c_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn w15c_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| driver.state().object(*id).is_some_and(|o| o.card().name == name))
}

#[rstest]
fn w15c_marvo_clashes_with_the_player_it_attacks(
    #[values(2, 3, 4)] seats: usize,
    #[values((5, 1, true), (1, 1, false))] clash: (u32, u32, bool),
) -> Result<(), ScriptError> {
    let (mine, theirs, won) = clash;
    let defending = seats - 1;
    let scenario = || {
        w15c_table(seats, mine, theirs)
            .battlefield(0, [ready(card("Marvo, Deep Operative").unwrap())])
            .hand(0, [w15c_costing(8)])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Marvo, Deep Operative", defending)])?;
    let (chosen, clashes) = w15c_resolve(&mut driver, true)?;
    assert_eq!(chosen, 0, "the defending player is not chosen");
    assert_eq!(
        clashes,
        [Event::Clashed {
            player: pid(0),
            opponent: pid(defending),
            won
        }]
    );
    let hand = driver.state().zone(Zone::Hand(pid(0)));
    assert_eq!(w15c_on_battlefield(&driver, "Cost 8"), won, "the free cast");
    assert_eq!(hand.len(), 1, "a won clash draws one card and casts one");
    w15c_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn w15c_scattering_stroke_adds_colorless_at_the_next_main_phase(
    #[values(2, 3, 4)] seats: usize,
    #[values((5, 1, true), (1, 1, false))] clash: (u32, u32, bool),
    #[values(true, false)] take: bool,
) -> Result<(), ScriptError> {
    let (mine, theirs, won) = clash;
    let scenario = || {
        let lands: Vec<_> = (0..7).map(|_| ready(land(Color::Blue))).collect();
        w15c_table(seats, mine, theirs)
            .battlefield(0, lands)
            .hand(0, [card("Scattering Stroke").unwrap(), w15c_costing(3)])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Cost 3")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let spell = driver.target_named("Cost 3")?;
    driver.cast_at("Scattering Stroke", &[spell])?;
    let (_, clashes) = w15c_resolve(&mut driver, false)?;
    assert_eq!(clashes.len(), 1);
    assert!(!w15c_on_battlefield(&driver, "Cost 3"), "countered");
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    driver.pass_until(Step::Main2)?;
    w15c_resolve(&mut driver, take)?;
    let added = driver.state().player(pid(0)).unwrap().mana().get(ManaType::Colorless);
    assert_eq!(added, if won && take { 3 } else { 0 });
    w15c_replays(&driver, scenario());
    Ok(())
}
