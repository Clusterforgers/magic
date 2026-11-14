//! Goal: when an ability that targets only Agrus Kos, Eternal Soldier targets it, its controller may pay {1}{R/W} to
//! copy that ability once for each other creature they control that it could target, each copy aimed at a different
//! one; a spell or an ability that also targets another creature does not trigger it, and declining copies nothing; at
//! two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const AGRUS: &str = "Agrus Kos, Eternal Soldier";

/// An artifact with "{0}: Target creature gets +1/+1 until end of turn." (one target) and "{0}: Two target creatures
/// each get +1/+1 until end of turn." (two targets).
fn r6104_rod() -> Arc<CardDefinition> {
    let pump = |places: usize| {
        let targets = vec![r#"{"what":{"Object":[{"Type":"Creature"}]}}"#; places].join(",");
        let effects = (0..places)
            .map(|at| {
                format!(
                    r#"{{"Continuous":{{"affected":{{"Target":{at}}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}],"duration":"UntilEndOfTurn"}}}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":"{{0}}","targets":[{targets}],"effects":[{effects}]}}}}"#
        ))
        .unwrap()
    };
    Arc::new(
        CardDefinition::new("Rod")
            .with_type(CardType::Artifact)
            .with_rules(pump(1))
            .with_rules(pump(2)),
    )
}

/// Seat 0 controls Agrus Kos, two Bears, the Rod and two Mountains; every other seat an Ogre.
fn r6104_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(AGRUS).unwrap()),
            ready(creature("Bear", 1, &[], 2, 2)),
            ready(creature("Bear", 1, &[], 2, 2)),
            ready(r6104_rod()),
            ready(mountain()),
            ready(mountain()),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(creature("Ogre", 2, &[], 3, 3))]);
    }
    scenario
}

/// The power of every creature on the battlefield, by name, sorted.
fn r6104_powers(driver: &Driver) -> Vec<(String, i32)> {
    let state = driver.state();
    let mut found: Vec<(String, i32)> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| {
            let seen = mtg_engine::characteristics::characteristics(state, *id)?;
            let power = seen.power?;
            Some((state.object(*id)?.card().name.clone(), power))
        })
        .collect();
    found.sort();
    found
}

/// Activates the Rod's ability `index` at `targets` (one per place), answers the payment of the trigger with `pay`,
/// and lets everything resolve.
fn r6104_pump(driver: &mut Driver, index: usize, targets: &[Target], pay: bool) {
    driver.begin_ability("Rod", index).unwrap();
    let mut places = targets.iter();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseTargets(_) => match places.next() {
                Some(at) => {
                    driver.choose_targets(&[*at]).unwrap();
                }
                None => {
                    driver.answer_passively().unwrap();
                }
            },
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(pay))).unwrap();
            }
            DecisionKind::PayMana(_) if !driver.state().stack().is_empty() && !pay => {
                driver.answer(Answer::PayMana(PayChoice::Cancel)).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not empty");
}

#[rstest]
fn r6104_an_ability_at_agrus_alone_is_copied_for_each_other_creature_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pay: bool,
) {
    let scenario = r6104_table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let agrus = driver.target_named(AGRUS).unwrap();
    r6104_pump(&mut driver, 0, &[agrus], pay);
    let bears = if pay { 3 } else { 2 };
    let mut expected = vec![
        ("Bear".to_owned(), bears),
        ("Bear".to_owned(), bears),
        (AGRUS.to_owned(), 4),
    ];
    expected.extend((1..seats).map(|_| ("Ogre".to_owned(), 3)));
    expected.sort();
    assert_eq!(r6104_powers(&driver), expected);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r6104_an_ability_that_also_targets_another_creature_is_not_copied(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = r6104_table(seats);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let agrus = driver.target_named(AGRUS).unwrap();
    let bear = driver.target_named("Bear").unwrap();
    r6104_pump(&mut driver, 1, &[agrus, bear], true);
    let mut expected = vec![
        ("Bear".to_owned(), 2),
        ("Bear".to_owned(), 3),
        (AGRUS.to_owned(), 4),
    ];
    expected.extend((1..seats).map(|_| ("Ogre".to_owned(), 3)));
    expected.sort();
    assert_eq!(r6104_powers(&driver), expected);
}
