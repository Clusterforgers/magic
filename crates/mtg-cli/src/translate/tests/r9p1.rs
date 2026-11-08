//! Goal: round 9 piece 1 reads the long tail of single-card keys; a cost that discards the whole hand pays the hand size and plays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn artifact(line: &str) -> String {
    format!("Name:Subject\nTypes:Artifact\n{line}\n")
}

fn card(text: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Artifact)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    for value in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == name)
        .count()
}

#[rstest]
#[case::whole_hand_as_one("Discard<1/Hand>", 3)]
#[case::whole_hand_as_zero("Discard<0/Hand>", 3)]
#[case::empty_hand("Discard<1/Hand>", 0)]
fn a_cost_that_discards_the_hand_discards_all_of_it(#[case] cost: &str, #[case] hand: usize) {
    let text = artifact(&format!(
        "A:AB$ Draw | Cost$ {cost} | NumCards$ 2 | SpellDescription$ x"
    ));
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let scenario = Scenario::new(2)
        .at(3, 0, mtg_engine::turn::Step::Main1)
        .battlefield(0, [ready(card(&text))])
        .hand(0, (0..hand).map(|_| mountain()))
        .library(0, vec![forest(); 9])
        .library(1, vec![forest(); 9]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = Driver::passive_answer(&decision);
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        count(
            &driver,
            Zone::Graveyard(PlayerId::from_index(0)),
            "Mountain"
        ),
        hand
    );
    assert_eq!(
        count(&driver, Zone::Hand(PlayerId::from_index(0)), "Mountain"),
        0
    );
    assert_eq!(
        count(&driver, Zone::Hand(PlayerId::from_index(0)), "Forest"),
        2
    );
}

#[rstest]
#[case::accepted(true, 0, 2)]
#[case::declined(false, 3, 0)]
fn a_may_discard_the_hand_trigger_draws_only_when_paid(
    #[case] yes: bool,
    #[case] kept: usize,
    #[case] drawn: usize,
) {
    let text = artifact(
        "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:AB$ Draw | Cost$ Discard<0/Hand> | NumCards$ 2",
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let scenario = Scenario::new(2)
        .at(3, 0, mtg_engine::turn::Step::Main2)
        .battlefield(0, [ready(card(&text))])
        .hand(0, (0..3).map(|_| mountain()))
        .library(0, vec![forest(); 9])
        .library(1, vec![forest(); 9]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(mtg_engine::turn::Step::End).unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) if !o.only_no => {
                Answer::YesNo(mtg_engine::choose::YesNoChoice(yes))
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        count(&driver, Zone::Hand(PlayerId::from_index(0)), "Mountain"),
        kept
    );
    assert_eq!(
        count(&driver, Zone::Hand(PlayerId::from_index(0)), "Forest"),
        drawn
    );
}
