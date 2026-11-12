//! Goal: Urza's Saga, a Saga that is a land, plays whole at tables of two, three and four seats: it is played as the land of the turn and gets a lore counter as it enters and after each of its controller's draw steps, chapter I gives it a mana ability for {C}, chapter II gives it "{2}, {T}" for a Construct token that grows with the artifacts its controller has, chapter III goes on the stack and searches for exactly the artifact cards whose whole mana cost is {0} or {1} (not {U}, {2} or no cost), puts the one chosen onto the battlefield and shuffles, and the Saga is sacrificed once chapter III has resolved, keeping its granted abilities until then.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{cost, plains, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SAGA: &str = "Urza's Saga";

/// An artifact with this mana cost; `None` is a card with no mana cost at all.
fn artifact(name: &str, mana: Option<ManaCost>, creature: bool) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name).with_type(CardType::Artifact);
    if let Some(mana) = mana {
        def = def.with_mana_cost(mana);
    }
    if creature {
        def = def.with_type(CardType::Creature).with_power_toughness(0, 2);
    }
    Arc::new(def)
}

/// Seat 0's library from the top down: lands to draw, then the artifacts the search looks through.
fn library() -> Vec<Arc<CardDefinition>> {
    let mut cards: Vec<Arc<CardDefinition>> = (0..6).map(|_| plains()).collect();
    cards.extend([
        card("Sol Ring").unwrap(),
        artifact("Mishra's Bauble", Some(cost(0, &[])), false),
        artifact("Ornithopter", Some(cost(0, &[])), true),
        artifact("Blue Trinket", Some(cost(0, &[Color::Blue])), false),
        artifact("Two Trinket", Some(cost(2, &[])), false),
        artifact("Costless Relic", None, false),
    ]);
    cards
}

fn scenario(seats: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card(SAGA).unwrap()])
        .battlefield(0, [ready(wastes()), ready(wastes())])
        .library(0, library())
        .at(1, 0, Step::Main1)
}

fn saga_id(driver: &Driver) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == SAGA)
}

fn lore(driver: &Driver) -> u32 {
    saga_id(driver)
        .and_then(|id| driver.state().object(id))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::lore()))
}

/// The abilities on the stack whose source is the Saga.
fn chapters_on_stack(driver: &Driver) -> usize {
    let saga = saga_id(driver).unwrap();
    driver
        .state()
        .stack()
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter_map(|object| object.ability())
        .filter(|ability| ability.source().id == saga)
        .count()
}

/// Passes until the stack is empty or something other than priority is asked.
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let from = driver.engine().events().len();
    for _ in 0..100 {
        let busy = !driver.state().stack().is_empty() || driver.state().resolving().is_some();
        if !busy || !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            break;
        }
        driver.pass()?;
    }
    Ok(driver.engine().events()[from..].to_vec())
}

#[rstest]
fn urzas_saga_plays_its_three_chapters_and_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = scenario(seats);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(SAGA)
        .unwrap();
    assert!(matches!(support.support, Support::Verified), "{support:?}");

    // Played as the land of the turn: it enters with a lore counter and chapter I goes on the stack.
    driver.play_land(SAGA)?;
    assert_eq!(lore(&driver), 1);
    assert_eq!(chapters_on_stack(&driver), 1);
    settle(&mut driver)?;
    let events = driver.tap_for_mana(SAGA)?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ManaAdded { mana: ManaType::Colorless, source: Some(source), .. } if Some(*source) == saga_id(&driver)
    )));

    // The next turn's counter comes after the draw step: chapter II, and the Saga keeps its mana ability.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(lore(&driver), 2);
    assert_eq!(chapters_on_stack(&driver), 1);
    settle(&mut driver)?;
    driver.begin_activation(SAGA)?;
    driver.pay_tapping(&["Wastes", "Wastes"], 0)?;
    settle(&mut driver)?;
    let construct = driver.id_of("Construct").unwrap();
    let found = characteristics(driver.state(), construct).unwrap();
    // The token is the one artifact its controller has.
    assert_eq!((found.power, found.toughness), (Some(1), Some(1)));
    assert!(found.types.contains(CardType::Artifact));

    // Chapter III: the trigger goes on the stack and the Saga waits for it.
    driver.idle_until(2 * next - 1, Step::Main1)?;
    assert_eq!(lore(&driver), 3);
    assert_eq!(chapters_on_stack(&driver), 1);
    // The table shows its words on the stack.
    let top = driver.state().stack().last().copied().unwrap();
    let rules = driver
        .state()
        .object(top)
        .unwrap()
        .ability()
        .unwrap()
        .rules()
        .clone();
    let words = mtg_cards::compile::render_ability(&rules).unwrap();
    assert!(
        words.contains("Search your library for an artifact card with mana cost {0} or {1}"),
        "{words}"
    );
    settle(&mut driver)?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the search asks for a card, not {}", decision.kind.name());
    };
    assert_eq!(decision.player, pid(0));
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    offered.sort();
    assert_eq!(offered, ["Mishra's Bauble", "Ornithopter", "Sol Ring"]);
    assert!(saga_id(&driver).is_some());
    let sol_ring = options
        .from
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Sol Ring")
        .unwrap();
    let from = driver.engine().events().len();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![sol_ring])))?;
    settle(&mut driver)?;
    let events = &driver.engine().events()[from..];
    let entered = events.iter().position(|event| {
        matches!(
            event,
            Event::ZoneChanged {
                to: Zone::Battlefield,
                ..
            }
        )
    });
    let shuffled = events.iter().position(
        |event| matches!(event, Event::LibraryShuffled { player, .. } if *player == pid(0)),
    );
    assert!(entered.unwrap() < shuffled.unwrap());
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Sol Ring".to_owned()));
    assert!(names(driver.state(), Zone::Library(pid(0))).contains(&"Mishra's Bauble".to_owned()));

    // CR 714.4: with chapter III resolved the Saga is sacrificed.
    assert!(saga_id(&driver).is_none());
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&SAGA.to_owned()));
    // Sol Ring and the token are the artifacts now.
    let found = characteristics(driver.state(), construct).unwrap();
    assert_eq!(found.power, Some(2));

    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
