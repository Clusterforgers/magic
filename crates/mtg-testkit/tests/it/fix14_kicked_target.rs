//! Goal: a target only a kicked spell has ("If this spell was kicked, it deals 5 damage to target player", Unstable
//! Footing) is asked for only when the kicker is paid, at tables of two to four seats: unkicked, the spell asks for no
//! target and its rule still begins; kicked, the target is asked for and dealt the damage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CostsChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The spell as the translator writes Unstable Footing's.
const FOOTING: &str = r#"{"Spell":{"effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"DamageCantBePrevented":{}}}},{"If":{"condition":"Kicked","then":[{"DealDamage":{"amount":5,"to":{"Target":0},"unpreventable":true}}]}}],"targets":[{"bounds":{"max":{"When":{"condition":"Kicked","otherwise":0,"then":1}},"min":{"When":{"condition":"Kicked","otherwise":0,"then":1}}},"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}]}}"#;

fn footing() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(FOOTING).unwrap();
    Arc::new(
        CardDefinition::new("Footing")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_rules(Ability::Keyword(Keyword::Kicker(Cost::mana(
                "{3}{R}".parse().unwrap(),
            ))))
            .with_rules(spell),
    )
}

#[rstest]
fn the_target_is_asked_for_only_when_kicked(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] kicked: bool,
) -> Result<(), ScriptError> {
    let other = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [footing()])
            .battlefield(0, (0..5).map(|_| ready(mountain()))),
    )?;
    driver.begin_cast("Footing")?;
    let mut asked = false;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseCosts(_) => {
                Answer::ChooseCosts(CostsChoice::Costs(if kicked { vec![0] } else { vec![] }))
            }
            DecisionKind::ChooseTargets(_) => {
                asked = true;
                Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_player(other)]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(asked, kicked);
    let life = driver.state().player(pid(other)).unwrap().life();
    assert_eq!(life, if kicked { 15 } else { 20 });
    let rules = driver.state().rule_effects().len();
    assert!(rules >= 1, "the rule began");
    Ok(())
}
