//! Goal: the corpus rules of the r30-1b pack play as printed at tables of two to four seats: Rasputin removes the dream
//! counters its controller chooses (one or more) and adds that much {C}, and Vazi gives its target opponent as many
//! Treasures as its controller created this turn, and Tree of Redemption exchanges its controller's life total with its
//! toughness, and Seasinger takes only a creature whose controller controls an Island.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::state::GameState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize, first: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, first);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    scenario
}

/// Answers every question of the pending activation: `x` for a number, `target` for a target, until the stack is empty.
fn settle(driver: &mut Driver, x: u32, target: Option<PlayerId>) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                let chosen = driver.target_player(target.map_or(0, PlayerId::index));
                Answer::ChooseTargets(TargetChoice::Targets(vec![chosen]))
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Ok(())
}

fn dream_counters(state: &GameState) -> u32 {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Rasputin, the Oneiromancer")
        .filter_map(|object| object.permanent())
        .flat_map(|permanent| permanent.counters().map(|(_, n)| n).collect::<Vec<_>>())
        .sum()
}

#[rstest]
fn rasputin_adds_that_much_colorless(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] removed: u32,
) -> Result<(), ScriptError> {
    let rasputin = with_counters(
        ready(card("Rasputin, the Oneiromancer").unwrap()),
        CounterKind::Other("dream".into()),
        3,
    );
    let mut driver = Driver::from_scenario(table(seats, vec![rasputin]))?;
    driver.begin_ability("Rasputin, the Oneiromancer", 1)?;
    settle(&mut driver, removed, None)?;
    let state = driver.state();
    assert_eq!(state.player(PlayerId(0)).unwrap().mana().total(), removed);
    assert_eq!(dream_counters(state), 3 - removed);
    Ok(())
}

/// An artifact that creates a Treasure token for {T}.
fn treasure_maker(name: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_value(serde_json::json!({"Activated": {
        "cost": ["Tap"],
        "effects": [{"CreateToken": {"token": {"Named": "treasure"}}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

fn treasures(state: &GameState, seat: u64) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            object.card().name == "Treasure"
                && mtg_engine::characteristics::controller_of(state, object.id())
                    == Some(PlayerId(seat))
        })
        .count()
}

#[rstest]
fn vazi_gives_as_many_treasures_as_you_created(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] made: usize,
) -> Result<(), ScriptError> {
    let mut seat0 = vec![ready(card("Vazi, Keen Negotiator").unwrap())];
    seat0.extend((0..made).map(|at| ready(treasure_maker(&format!("Maker {at}")))));
    let mut driver = Driver::from_scenario(table(seats, seat0))?;
    for at in 0..made {
        driver.begin_activation(&format!("Maker {at}"))?;
        settle(&mut driver, 0, None)?;
    }
    driver.begin_activation("Vazi, Keen Negotiator")?;
    settle(&mut driver, 0, Some(PlayerId(1)))?;
    assert_eq!(treasures(driver.state(), 1), made);
    assert_eq!(treasures(driver.state(), 0), made);
    Ok(())
}

/// Tree of Redemption (0/13) with `counters` +1/+1 counters exchanges seat 0's 20 life with its toughness: the life becomes
/// the toughness it had, its base toughness becomes 20 and the counters still count.
#[rstest]
fn tree_of_redemption_exchanges_life_with_its_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] counters: u32,
) -> Result<(), ScriptError> {
    let tree = with_counters(
        ready(card("Tree of Redemption").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let scenario = table(seats, vec![tree]).life(0, 20);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Tree of Redemption")?;
    settle(&mut driver, 0, None)?;
    let state = driver.state();
    let tree = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.card().name == "Tree of Redemption")
        })
        .unwrap();
    let toughness = mtg_engine::characteristics::characteristics(state, tree)
        .and_then(|chars| chars.toughness)
        .unwrap();
    let before = 13 + i32::try_from(counters).unwrap();
    assert_eq!(state.player(PlayerId(0)).unwrap().life(), before);
    assert_eq!(toughness, 20 + i32::try_from(counters).unwrap());
    Ok(())
}

/// Seasinger may take a creature only while that creature's controller controls an Island, and keeps it while it stays
/// tapped under its controller.
#[rstest]
fn seasinger_takes_a_creature_whose_controller_controls_an_island(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] island_there: bool,
) -> Result<(), ScriptError> {
    let mut theirs = vec![ready(mtg_testkit::cards::free_creature("Bear", 2, 2))];
    if island_there {
        theirs.push(ready(island()));
    }
    let scenario = table(
        seats,
        vec![ready(card("Seasinger").unwrap()), ready(island())],
    )
    .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Seasinger")?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("targets, not {:?}", decision.kind);
    };
    let bear = driver.target_named("Bear")?;
    assert_eq!(options.legal.contains(&bear), island_there);
    if island_there {
        driver.choose_targets(&[bear])?;
        settle(&mut driver, 0, None)?;
        let bear = driver.id_of("Bear")?;
        assert_eq!(
            mtg_engine::characteristics::controller_of(driver.state(), bear),
            Some(PlayerId(0))
        );
    }
    Ok(())
}
