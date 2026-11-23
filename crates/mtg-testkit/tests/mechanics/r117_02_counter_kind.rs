//! Goal: "Choose a counter on target enchantment you control. Put another counter of that kind on it." (`Effect::R117AnotherOfKind`) puts one more counter of the one kind there is without asking, asks the controller between two kinds and puts the one chosen, and puts nothing on a permanent without counters; at two to four seats, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, OptionChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn lens() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}],"effects":[{"R117AnotherOfKind":{"on":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Lens")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

fn shrine() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shrine")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[])),
    )
}

fn lore() -> CounterKind {
    CounterKind::Other("lore".to_owned())
}

#[rstest]
#[case::one_kind_two_seats(2, vec![(lore(), 2)], None, vec![(lore(), 3)])]
#[case::two_kinds_lore(3, vec![(lore(), 1), (CounterKind::PlusOnePlusOne, 1)], Some("lore"), vec![(lore(), 2), (CounterKind::PlusOnePlusOne, 1)])]
#[case::two_kinds_plus(4, vec![(lore(), 1), (CounterKind::PlusOnePlusOne, 1)], Some("+1/+1"), vec![(lore(), 1), (CounterKind::PlusOnePlusOne, 2)])]
#[case::no_counters(2, vec![], None, vec![])]
fn another_counter_of_a_chosen_kind(
    #[case] seats: usize,
    #[case] start: Vec<(CounterKind, u32)>,
    #[case] pick: Option<&str>,
    #[case] end: Vec<(CounterKind, u32)>,
) {
    let mut held = ready(shrine());
    for (kind, count) in start {
        held = with_counters(held, kind, count);
    }
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, vec![ready(lens()), held]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Lens").unwrap();
    let mut asked = false;
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => Answer::ChooseTargets(TargetChoice::Targets(vec![
                driver.target_named("Shrine").unwrap(),
            ])),
            DecisionKind::ChooseOption(options) => {
                asked = true;
                assert_eq!(decision.player, PlayerId(0));
                let wanted = pick.unwrap();
                let at = options
                    .options
                    .iter()
                    .position(|o| matches!(o, ChoiceOption::Label(l) if l.starts_with(wanted)))
                    .unwrap();
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(asked, pick.is_some(), "asked only between kinds");
    let shrine = driver.id_of("Shrine").unwrap();
    let permanent = driver.state().object(shrine).unwrap().permanent().unwrap();
    let held: Vec<(CounterKind, u32)> = permanent
        .counters()
        .map(|(kind, count)| (kind.clone(), count))
        .collect();
    let mut end = end;
    end.sort();
    assert_eq!(held, end);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}
