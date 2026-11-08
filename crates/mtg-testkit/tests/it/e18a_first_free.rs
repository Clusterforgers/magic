//! Goal: pack e18a's first ability of a kind each turn plays whole at two, three and four seats: with "you may pay {0}
//! rather than pay the [kind] cost of the first [kind] ability you activate each turn" the first equip, cycling or unearth
//! of the turn costs nothing and the second its printed cost, the tally starts again on the controller's next turn, a
//! `FreeActivation` without a condition makes every one free, the turn's tally of activations is kept by kind, an
//! `InstantActivation` of equip offers the equip abilities of its controller (only) while the stack holds a spell and on
//! another player's turn when it has no condition, and each game replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_engine::PresetPermanent;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::name_of;

/// The condition "the first [kind] ability you activate each turn": none of the kind activated by you this turn.
fn first(kind: &str) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Tally":{{"event":{{"KindActivated":"{kind}"}},"filter":[],"who":"You"}}}},"op":"Eq","right":0}}}}"#
    )
}

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// The artifact with the `FreeActivation` of the kind, gated by `condition` when there is one.
fn guide(kind: &str, condition: Option<&str>) -> Arc<CardDefinition> {
    let condition = condition.map_or_else(String::new, |held| format!(r#","condition":{held}"#));
    let json = format!(r#"{{"FreeActivation":{{"kind":"{kind}"{condition}}}}}"#);
    Arc::new(
        CardDefinition::new("Guide")
            .with_type(CardType::Artifact)
            .with_rules(rules(&json)),
    )
}

fn equipment(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_rules(rules(r#"{"Keyword":{"Equip":"{2}"}}"#)),
    )
}

fn cycler(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_rules(rules(r#"{"Keyword":{"Cycling":"{2}"}}"#)),
    )
}

fn unearther(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules(r#"{"Keyword":{"Unearth":"{2}"}}"#)),
    )
}

const NAMES: [&str; 3] = ["One", "Two", "Three"];

/// Seat 0 holds the guide, four Mountains, a creature and three sources of the kind (on the battlefield, in hand or in the
/// graveyard), at `seats` seats on turn 1.
fn scenario(seats: usize, kind: &str, guide: Arc<CardDefinition>) -> Scenario {
    scenario_with(seats, kind, guide, Vec::new())
}

/// [`scenario`] with more permanents for seat 0.
fn scenario_with(
    seats: usize,
    kind: &str,
    guide: Arc<CardDefinition>,
    more: Vec<PresetPermanent>,
) -> Scenario {
    let mut mine = vec![ready(guide), ready(mtg_testkit::cards::free_creature("Bear", 2, 2))];
    mine.extend(more);
    mine.extend((0..4).map(|_| ready(mountain())));
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    match kind {
        "Equip" => mine.extend(NAMES.iter().map(|name| ready(equipment(name)))),
        "Cycling" => scenario = scenario.hand(0, NAMES.map(cycler)),
        _ => scenario = scenario.graveyard(0, NAMES.map(unearther)),
    }
    (0..seats).fold(scenario.battlefield(0, mine), |scenario, seat| {
        scenario.library(seat, vec![forest(); 10])
    })
}

/// Activates the named source's ability and lets it resolve, with the passive answers and the automatic payment.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..60 {
        let decision = driver.decision()?;
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            // The equip ability targets the Bear.
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Bear")?;
                driver.choose_targets(&[bear])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the activation did not settle");
}

/// How many of seat 0's Mountains are tapped.
fn tapped_mountains(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == "Mountain")
        .filter(|id| state.object(**id).unwrap().permanent().unwrap().tapped())
        .count()
}

#[rstest]
fn the_first_ability_of_the_kind_each_turn_is_free(
    #[values("Equip", "Cycling", "Unearth")] kind: &str,
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = scenario(seats, kind, guide(kind, Some(&first(kind))));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    activate(&mut driver, "One").unwrap();
    assert_eq!(tapped_mountains(&driver), 0, "the first {kind} is free");
    activate(&mut driver, "Two").unwrap();
    assert_eq!(tapped_mountains(&driver), 2, "the second {kind} costs {{2}}");
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1).unwrap();
    activate(&mut driver, "Three").unwrap();
    assert_eq!(tapped_mountains(&driver), 0, "a new turn's first {kind} is free");
    replays(&driver, &setup);
}

#[rstest]
fn without_a_condition_every_ability_of_the_kind_is_free(#[values(2, 3, 4)] seats: usize) {
    let scenario = scenario(seats, "Cycling", guide("Cycling", None));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    activate(&mut driver, "One").unwrap();
    activate(&mut driver, "Two").unwrap();
    assert_eq!(tapped_mountains(&driver), 0);
    replays(&driver, &setup);
}

#[rstest]
#[case::another_kind("Equip", "Cycling")]
#[case::unearth_for_equip("Unearth", "Equip")]
fn a_guide_of_another_kind_frees_nothing(#[case] guided: &str, #[case] kind: &str) {
    let mut driver = Driver::from_scenario(scenario(2, kind, guide(guided, Some(&first(guided))))).unwrap();
    activate(&mut driver, "One").unwrap();
    assert_eq!(tapped_mountains(&driver), 2);
}

/// An artifact whose "{T}: You gain 3 life." may be activated only once `count` abilities of the kind were activated by you
/// this turn.
fn watcher(name: &str, kind: &str, count: u32) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"condition":{{"Compare":{{"left":{{"Tally":{{"event":{{"KindActivated":"{kind}"}},"filter":[],"who":"You"}}}},"op":"Ge","right":{count}}}}},"cost":["Tap"],"effects":[{{"GainLife":{{"amount":3}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(rules(&json)),
    )
}

#[rstest]
fn the_tally_of_the_turn_is_kept_by_kind(#[values(2, 3, 4)] seats: usize) {
    let scenario = scenario_with(
        seats,
        "Cycling",
        guide("Equip", None),
        vec![
            ready(watcher("Cycled Once", "Cycling", 1)),
            ready(watcher("Cycled Twice", "Cycling", 2)),
            ready(watcher("Equipped", "Equip", 1)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let offered = |driver: &Driver, name: &str| !driver.offered_abilities(name).unwrap().is_empty();
    assert!(!offered(&driver, "Cycled Once"));
    activate(&mut driver, "One").unwrap();
    assert!(offered(&driver, "Cycled Once"));
    assert!(!offered(&driver, "Cycled Twice"));
    assert!(!offered(&driver, "Equipped"));
    activate(&mut driver, "Two").unwrap();
    assert!(offered(&driver, "Cycled Twice"));
}

/// The guide with "you may activate equip abilities any time you could cast an instant", during your turn when `yours`;
/// `None` for an artifact without it.
fn quick(rule: Option<bool>) -> Arc<CardDefinition> {
    let json = match rule {
        None => r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#.to_owned(),
        Some(yours) => {
            let condition = if yours { r#","condition":"YourTurn""# } else { "" };
            format!(r#"{{"InstantActivation":{{"kind":"Equip"{condition}}}}}"#)
        }
    };
    Arc::new(
        CardDefinition::new("Guide")
            .with_type(CardType::Artifact)
            .with_rules(rules(&json)),
    )
}

/// Whether seat 0 is offered the equip ability of the Equipment named "One" at its first priority in `step` of seat
/// `active`'s turn.
fn equip_offered(seats: usize, rule: Option<bool>, active: usize, step: Step) -> bool {
    let scenario = scenario(seats, "Equip", quick(rule)).at(1, active, step);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..seats {
        if driver.decision().unwrap().player.0 == 0 {
            break;
        }
        driver.pass().unwrap();
    }
    assert_eq!(driver.decision().unwrap().player.0, 0);
    let offered = !driver.offered_abilities("One").unwrap().is_empty();
    replays(&driver, &setup);
    offered
}

#[rstest]
#[case::own_main_phase(None, 0, Step::Main1, true)]
#[case::own_combat_without(None, 0, Step::BeginCombat, false)]
#[case::own_combat(Some(false), 0, Step::BeginCombat, true)]
#[case::own_combat_your_turn(Some(true), 0, Step::BeginCombat, true)]
#[case::another_players_turn(Some(false), 1, Step::Main1, true)]
#[case::another_players_turn_your_turn(Some(true), 1, Step::Main1, false)]
#[case::another_players_turn_without(None, 1, Step::Main1, false)]
fn an_instant_activation_opens_equip_at_instant_speed(
    #[case] rule: Option<bool>,
    #[case] active: usize,
    #[case] step: Step,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    assert_eq!(equip_offered(seats, rule, active, step), offered);
}
