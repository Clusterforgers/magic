//! Goal: the cost parts "put counters on this", "exert this", "untap tapped creatures", "remove counters from among your permanents" and a sacrifice counted by what the table holds are offered only while the player can pay them, pay with the events the engine logs for each, leave nothing changed when they cannot be paid, and replay to the state they were played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, choose::CardsChoice, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::pid;

/// A creature for nothing: "<cost>: You gain 1 life."
fn pitcher(cost: &str) -> Arc<CardDefinition> {
    let ability =
        format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Pitcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    )
}

fn relic(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

/// What the table holds besides the Pitcher, and what paid looks like.
#[derive(Debug, Clone, Copy)]
enum Table {
    Alone,
    TwoTappedWolves,
    OneTappedWolf,
    FourCountersAmongTwo,
    TwoCountersAmongTwo,
    ThreeRelicsTwoWolves,
    ThreeRelicsOneWolf,
}

impl Table {
    fn scenario(self, cost: &str) -> Scenario {
        let wolf = |name: &str| free_creature(name, 2, 2);
        let charged = |name: &str, n| with_counters(ready(wolf(name)), CounterKind::Charge, n);
        let pitcher = ready(pitcher(cost));
        let battlefield = match self {
            Table::Alone => vec![pitcher],
            Table::TwoTappedWolves => vec![pitcher, tapped(wolf("Wolf")), tapped(wolf("Ogre"))],
            Table::OneTappedWolf => vec![pitcher, tapped(wolf("Wolf")), ready(wolf("Ogre"))],
            Table::FourCountersAmongTwo => vec![pitcher, charged("Wolf", 3), charged("Ogre", 1)],
            Table::TwoCountersAmongTwo => vec![pitcher, charged("Wolf", 1), charged("Ogre", 1)],
            Table::ThreeRelicsTwoWolves => vec![
                pitcher,
                ready(relic("A")),
                ready(relic("B")),
                ready(relic("C")),
                ready(wolf("Wolf")),
                ready(wolf("Ogre")),
            ],
            Table::ThreeRelicsOneWolf => vec![
                pitcher,
                ready(relic("A")),
                ready(relic("B")),
                ready(relic("C")),
                ready(wolf("Wolf")),
            ],
        };
        Scenario::new(2).battlefield(0, battlefield)
    }
}

/// The cost and the table of each case; the first is paid in full, the second leaves the cost unpayable.
const PUT: &str = r#"[{"PutCounters":{"counter":"charge","count":2}}]"#;
const EXERT: &str = r#"["Exert"]"#;
const UNTAP: &str = r#"[{"UntapPermanents":{"filter":[{"Type":"Creature"}],"count":2}}]"#;
const AMONG: &str =
    r#"[{"RemoveCountersAmong":{"counter":"charge","count":3,"from":[{"Type":"Creature"}]}}]"#;
const AMONG_ANY: &str = r#"[{"RemoveCountersAmong":{"count":3,"from":[{"Type":"Creature"}]}}]"#;
const SACRIFICE_COUNTED: &str =
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":{"Count":[{"Type":"Artifact"}]}}}]"#;

/// Whether the cost was paid, read from the game after the ability resolved.
fn paid(driver: &Driver, cost: &str) -> bool {
    let state = driver.state();
    let counters = |name: &str| {
        let id = driver.id_of(name).unwrap();
        state
            .object(id)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| permanent.counters_of(&CounterKind::Charge))
    };
    let untapped = |name: &str| {
        let id = driver.id_of(name).unwrap();
        !state.object(id).unwrap().permanent().unwrap().tapped()
    };
    match cost {
        PUT => counters("Pitcher") == 2,
        EXERT => driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Exerted { .. })),
        UNTAP => untapped("Wolf") && untapped("Ogre"),
        AMONG | AMONG_ANY => counters("Wolf") + counters("Ogre") == 1,
        _ => state.zone(Zone::Graveyard(pid(0))).len() == 3,
    }
}

#[rstest]
#[case::counters_on_the_source(PUT, Table::Alone, true)]
#[case::exerting_the_source(EXERT, Table::Alone, true)]
#[case::untapping_two_tapped_creatures(UNTAP, Table::TwoTappedWolves, true)]
#[case::untapping_with_one_tapped_creature(UNTAP, Table::OneTappedWolf, false)]
#[case::untapping_with_nothing_tapped(UNTAP, Table::Alone, false)]
#[case::three_counters_among_four(AMONG, Table::FourCountersAmongTwo, true)]
#[case::three_counters_among_two(AMONG, Table::TwoCountersAmongTwo, false)]
#[case::three_counters_of_any_kind_among_four(AMONG_ANY, Table::FourCountersAmongTwo, true)]
#[case::three_counters_of_any_kind_among_two(AMONG_ANY, Table::TwoCountersAmongTwo, false)]
#[case::a_sacrifice_counted_by_the_artifacts(SACRIFICE_COUNTED, Table::ThreeRelicsTwoWolves, true)]
#[case::a_sacrifice_counted_beyond_the_creatures(
    SACRIFICE_COUNTED,
    Table::ThreeRelicsOneWolf,
    false
)]
fn a_cost_is_offered_only_while_it_can_be_paid_and_then_pays(
    #[case] cost: &str,
    #[case] table: Table,
    #[case] payable: bool,
) -> Result<(), ScriptError> {
    let scenario = table.scenario(cost);
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    assert_eq!(
        driver.offered_abilities("Pitcher")?.is_empty(),
        !payable,
        "the offer and the payment agree"
    );
    if !payable {
        assert!(driver.begin_activation("Pitcher").is_err());
        assert_eq!(
            before,
            (driver.engine().events().len(), driver.engine().state_hash())
        );
        return Ok(());
    }
    driver.begin_activation("Pitcher")?;
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let picks = options
            .from
            .iter()
            .take(usize::try_from(options.min).unwrap())
            .copied()
            .collect();
        driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
    }
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert!(driver.state().stack().is_empty(), "the ability resolved");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    assert!(paid(&driver, cost));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
