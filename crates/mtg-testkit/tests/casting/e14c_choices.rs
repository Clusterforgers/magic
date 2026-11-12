//! Goal: choices the game or the table makes play at two, three and four seats: an opponent picked at random comes from the seeded source (the same one for the same seed, every opponent for some seed, never the caster) as one public event; a later mode of a spell whose modes must target different players is never offered a player an earlier mode took; a creature type other than Wall and a choice between named card types offer only those; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, ModeChoice, OptionChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::{Event, replay};
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// A free sorcery with these rules.
fn sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// What the translator writes for "choose an opponent at random. That player loses 3 life."
const AT_RANDOM: &str = r#"{"Spell":{"effects":[
    {"ChooseAtRandom":{"from":"Opponents","as":"player"}},
    {"LoseLife":{"who":{"Bound":"player"},"amount":3}}]}}"#;

/// Casts the spell at a table of `seats` with this seed and plays it out; returns the opponent who lost life.
fn picked(seats: usize, seed: u64) -> Result<(Driver, usize), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .hand(0, [sorcery("Pick", AT_RANDOM)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Pick")?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let losers: Vec<usize> = (0..seats)
        .filter(|seat| life(&driver, *seat) < 20)
        .collect();
    assert_eq!(losers.len(), 1, "one player lost life");
    Ok((driver, losers[0]))
}

#[rstest]
fn an_opponent_at_random_comes_from_the_seed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut seen = BTreeSet::new();
    for seed in 0..30 {
        let (driver, seat) = picked(seats, seed)?;
        assert_ne!(seat, 0, "the caster is no opponent");
        assert_eq!(life(&driver, seat), 17);
        assert_eq!(seat, picked(seats, seed)?.1, "seed {seed}");
        let events: Vec<&Vec<PlayerId>> = driver
            .engine()
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::PlayersChosenAtRandom { players } => Some(players),
                _ => None,
            })
            .collect();
        assert_eq!(events, [&vec![pid(seat)]]);
        replays(&driver);
        seen.insert(seat);
    }
    assert_eq!(seen, (1..seats).collect::<BTreeSet<_>>());
    Ok(())
}

/// "Choose one or both. Each mode must target a different player. • Target player loses 1 life. • Target player loses
/// 2 life."
const DIFFERENT: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":2},"different_players":true,"modes":[
    {"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}}]},
    {"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":2}}]}]}}]}}"#;

#[rstest]
fn a_later_mode_targets_another_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] first: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).hand(0, [sorcery("Split", DIFFERENT)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Split")?;
    let mut offered: Vec<Vec<Target>> = Vec::new();
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseMode(_) => {
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))?;
            }
            DecisionKind::ChooseTargets(options) => {
                offered.push(options.legal.clone());
                let pick = if offered.len() == 1 {
                    Target::Player(pid(first))
                } else {
                    options.legal[0]
                };
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![pick])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(offered.len(), 2);
    assert_eq!(offered[0].len(), seats);
    assert_eq!(offered[1].len(), seats - 1);
    assert!(!offered[1].contains(&Target::Player(pid(first))));
    let second = offered[1][0].player().unwrap().index();
    assert_eq!(life(&driver, first), 19);
    assert_eq!(life(&driver, second), 18);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::not_a_wall(r#"{"CreatureTypeExcept":["Wall"]}"#, None, Some("Wall"))]
#[case::creature_or_land(r#"{"CardTypeOf":["Creature","Land"]}"#, Some(vec!["Creature", "Land"]), None)]
fn a_type_choice_offers_only_its_kinds(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] exactly: Option<Vec<&str>>,
    #[case] never: Option<&str>,
) -> Result<(), ScriptError> {
    let rules =
        format!(r#"{{"Spell":{{"effects":[{{"ChooseValue":{{"kind":{kind},"as":"chosen"}}}}]}}}}"#);
    let scenario = Scenario::new(seats).hand(0, [sorcery("Choose", &rules)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Choose")?;
    let mut asked = None;
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseOption(options) => {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .filter_map(|option| match option {
                        ChoiceOption::Label(label) => Some(label.clone()),
                        _ => None,
                    })
                    .collect();
                asked = Some(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let asked = asked.expect("the controller chose");
    if let Some(exactly) = exactly {
        assert_eq!(asked, exactly);
    }
    if let Some(never) = never {
        assert!(asked.len() > 100 && !asked.iter().any(|label| label == never));
    }
    replays(&driver);
    Ok(())
}
