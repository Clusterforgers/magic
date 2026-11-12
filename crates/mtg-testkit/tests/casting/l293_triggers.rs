//! Goal: the l29-3 trigger pieces play as printed at tables of two, three and four seats: a trigger on any clash of its
//! controller (Entangling Trap, Rebellion of the Flamekin) runs after a win and a loss alike and does its "If you won"
//! part only after a win; Rielle draws for the first discards of a turn only, and Veronica makes a Junk for the first nonland
//! discards of a turn even after a land was discarded; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A vanilla creature card of mana value `value`.
fn l293_costing(value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Cost {value}"))
            .with_type(CardType::Creature)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(1, 1),
    )
}

/// A free instant with the one spell ability given as JSON.
fn l293_spell(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// Answers until the stack is empty and seat 0 has priority: pays automatically, keeps each clashed card on top, says
/// `yes` to any other question and takes the first target offered.
fn l293_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(options) => {
                let keep = options.text.starts_with("Clash");
                driver.answer(Answer::YesNo(YesNoChoice(!keep)))?;
            }
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn l293_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 0's first main phase of turn 2 with the two clash cards, a Mountain and a clash spell; seat 0's library is of
/// mana value `mine`, every other of `theirs`, and seat 1 has a Bear.
fn l293_clash_table(seats: usize, mine: u32, theirs: u32) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .library(0, (0..8).map(|_| l293_costing(mine)))
        .battlefield(
            0,
            [
                ready(card("Entangling Trap").unwrap()),
                ready(card("Rebellion of the Flamekin").unwrap()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            [l293_spell(
                "Clasher",
                r#"{"Spell":{"effects":[{"Clash":{}}]}}"#,
            )],
        )
        .battlefield(1, [ready(bear())]);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| l293_costing(theirs)));
    }
    scenario
}

#[rstest]
fn l293_a_clash_triggers_won_or_lost(
    #[values(2, 3, 4)] seats: usize,
    #[values((5, 1, true), (1, 1, false), (0, 3, false))] clash: (u32, u32, bool),
) -> Result<(), ScriptError> {
    let (mine, theirs, won) = clash;
    let mut driver = Driver::from_scenario(l293_clash_table(seats, mine, theirs))?;
    driver.cast("Clasher")?;
    l293_settle(&mut driver)?;
    let state = driver.state();
    let token = driver.id_of("Elemental Shaman")?;
    assert_eq!(state.object(token).unwrap().controller(), pid(0));
    assert_eq!(
        has_keyword(state, token, &Keyword::Haste),
        won,
        "haste only after a win"
    );
    let bear = driver.id_of("Grizzly Bears")?;
    assert!(state.object(bear).unwrap().permanent().unwrap().tapped());
    l293_replays(&driver, l293_clash_table(seats, mine, theirs));
    // Seat 1's untap step comes next: the Bear stays tapped only after a won clash.
    driver.idle_until(3, Step::Upkeep)?;
    let tapped = driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, won, "the Bear doesn't untap only after a win");
    assert!(driver.state().zone(Zone::Battlefield).contains(&token));
    Ok(())
}

/// A free instant that discards `count` cards of the kind `filter` (JSON).
fn l293_toss(name: &str, count: u32, filter: &str) -> Arc<CardDefinition> {
    l293_spell(
        name,
        &format!(
            r#"{{"Spell":{{"effects":[{{"Discard":{{"count":{count},"filter":{filter}}}}}]}}}}"#
        ),
    )
}

const L293_CREATURES: &str = r#"[{"Type":"Creature"}]"#;
const L293_LANDS: &str = r#"[{"Type":"Land"}]"#;

/// Seat 0's first main phase with `permanent` and a hand of the given spells, four Bears and two Forests.
fn l293_discard_table(seats: usize, permanent: &str, spells: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut hand = spells;
    hand.extend((0..4).map(|_| bear()));
    hand.extend((0..2).map(|_| forest()));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .library(0, (0..8).map(|_| l293_costing(1)))
        .battlefield(0, [ready(card(permanent).unwrap())])
        .hand(0, hand);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| l293_costing(1)));
    }
    scenario
}

fn l293_hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

#[rstest]
fn l293_rielle_draws_for_the_first_discards_of_the_turn_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        l293_discard_table(
            seats,
            "Rielle, the Everwise",
            vec![
                l293_toss("Toss", 2, L293_CREATURES),
                l293_toss("Toss Again", 1, L293_CREATURES),
            ],
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    assert_eq!(l293_hand(&driver), 8);
    driver.cast("Toss")?;
    l293_settle(&mut driver)?;
    assert_eq!(l293_hand(&driver), 7, "two discarded, two drawn");
    driver.cast("Toss Again")?;
    l293_settle(&mut driver)?;
    assert_eq!(
        l293_hand(&driver),
        5,
        "the second discard of the turn draws nothing"
    );
    l293_replays(&driver, table());
    Ok(())
}

fn l293_junk(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == "Junk"))
        .count()
}

#[rstest]
fn l293_veronica_counts_the_first_nonland_discards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        l293_discard_table(
            seats,
            "Veronica, Dissident Scribe",
            vec![
                l293_toss("Toss Land", 1, L293_LANDS),
                l293_toss("Toss", 2, L293_CREATURES),
                l293_toss("Toss Again", 1, L293_CREATURES),
            ],
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Toss Land")?;
    l293_settle(&mut driver)?;
    assert_eq!(l293_junk(&driver), 0, "a land is no nonland card");
    driver.cast("Toss")?;
    l293_settle(&mut driver)?;
    assert_eq!(
        l293_junk(&driver),
        1,
        "the first nonland discards, after a land"
    );
    driver.cast("Toss Again")?;
    l293_settle(&mut driver)?;
    assert_eq!(l293_junk(&driver), 1, "only the first time each turn");
    l293_replays(&driver, table());
    Ok(())
}
